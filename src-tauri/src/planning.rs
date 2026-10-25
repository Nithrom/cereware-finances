//! Recurrentes (suscripciones y recibos), huchas y previsión de saldo.

use crate::ledger::{learn_key, rule_norm, uid};
use chrono::{Datelike, Days, Months, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

fn d(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

pub fn advance(date: NaiveDate, interval: &str) -> NaiveDate {
    match interval {
        "weekly" => date + Days::new(7),
        "quarterly" => date.checked_add_months(Months::new(3)).unwrap_or(date),
        "yearly" => date.checked_add_months(Months::new(12)).unwrap_or(date),
        _ => date.checked_add_months(Months::new(1)).unwrap_or(date),
    }
}

fn classify(days: f64) -> Option<&'static str> {
    match days {
        x if (5.0..=9.0).contains(&x) => Some("weekly"),
        x if (25.0..=36.0).contains(&x) => Some("monthly"),
        x if (83.0..=98.0).contains(&x) => Some("quarterly"),
        x if (350.0..=380.0).contains(&x) => Some("yearly"),
        _ => None,
    }
}

fn median(v: &mut [i64]) -> i64 {
    v.sort();
    v[v.len() / 2]
}

fn title(s: &str) -> String {
    s.split(' ')
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase()).unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Detecta pagos/ingresos periódicos en los últimos 13 meses y los guarda como `source='detected'`.
/// No resucita los descartados por el usuario (active=0).
pub fn detect(conn: &Connection) -> rusqlite::Result<usize> {
    let rows: Vec<(String, String, i64, String, Option<String>)> = {
        let mut st = conn.prepare(
            "SELECT t.description, t.booking_date, t.amount_cents, t.account_id, t.category_id FROM transactions t
             LEFT JOIN categories c ON c.id=t.category_id
             WHERE t.booking_date >= date('now','-13 months') AND COALESCE(c.kind,'') <> 'transfer'
             ORDER BY t.booking_date",
        )?;
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?.collect::<Result<_, _>>()?
    };
    let mut groups: HashMap<(String, String, bool), Vec<(NaiveDate, i64, Option<String>)>> = HashMap::new();
    for (desc, date, amt, acc, cat) in rows {
        let (Some(key), Some(dt)) = (learn_key(&desc), d(&date)) else { continue };
        groups.entry((key, acc, amt > 0)).or_default().push((dt, amt, cat));
    }
    let subs_cats: Vec<String> = {
        let mut st = conn.prepare("SELECT id FROM categories WHERE id='suscripciones' OR parent_id='suscripciones'")?;
        st.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?
    };
    let today = chrono::Local::now().date_naive();
    let mut n = 0;
    for ((key, acc, _), mut items) in groups {
        items.dedup_by_key(|i| i.0);
        if items.len() < 2 {
            continue;
        }
        let gaps: Vec<i64> = items.windows(2).map(|w| (w[1].0 - w[0].0).num_days()).collect();
        let mut g = gaps.clone();
        let med = median(&mut g) as f64;
        let Some(interval) = classify(med) else { continue };
        let min_n = if interval == "yearly" { 2 } else { 3 };
        if items.len() < min_n || gaps.iter().any(|&x| (x as f64 - med).abs() > med * 0.35 + 3.0) {
            continue;
        }
        // El último cargo puede cambiar de precio (subida): la regularidad se mide en los anteriores
        let body = if items.len() >= 3 { &items[..items.len() - 1] } else { &items[..] };
        let mut amts: Vec<i64> = body.iter().map(|i| i.1.abs()).collect();
        let med_amt = median(&mut amts);
        if body.iter().any(|i| (i.1.abs() - med_amt).abs() as f64 > med_amt as f64 * 0.25) {
            continue;
        }
        let last = items.last().unwrap();
        let mut next = advance(last.0, interval);
        // Si ya debería haberse cobrado hace mucho, se considera inactivo
        if (today - next).num_days() > med as i64 {
            continue;
        }
        while next < today {
            next = advance(next, interval);
        }
        let cat = last.2.clone();
        let is_sub = cat.as_ref().is_some_and(|c| subs_cats.contains(c));
        let existing: Option<(String, i64)> = conn
            .query_row("SELECT id, active FROM recurring WHERE match_value=? AND account_id IS ?", params![key, acc], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        match existing {
            Some((id, _)) => {
                conn.execute(
                    "UPDATE recurring SET amount_cents=?, interval=?, next_date=?, category_id=COALESCE(category_id,?) WHERE id=? AND source='detected'",
                    params![last.1, interval, next.to_string(), cat, id],
                )?;
            }
            None => {
                conn.execute(
                    "INSERT INTO recurring(id,name,account_id,category_id,amount_cents,interval,next_date,match_value,source,is_subscription)
                     VALUES(?,?,?,?,?,?,?,?, 'detected', ?)",
                    params![uid(), title(&key), acc, cat, last.1, interval, next.to_string(), key, is_sub as i64],
                )?;
                n += 1;
            }
        }
    }
    Ok(n)
}

#[derive(Serialize)]
pub struct Recurring {
    pub id: String,
    pub name: String,
    pub account_id: Option<String>,
    pub account_name: Option<String>,
    pub category_id: Option<String>,
    pub category_color: Option<String>,
    pub category_icon: Option<String>,
    pub amount_cents: i64,
    pub prev_amount_cents: Option<i64>,
    pub interval: String,
    pub next_date: String,
    pub source: String,
    pub is_subscription: bool,
    pub active: bool,
    pub yearly_cents: i64,
}

fn per_year(interval: &str) -> i64 {
    match interval {
        "weekly" => 52,
        "quarterly" => 4,
        "yearly" => 1,
        _ => 12,
    }
}

pub fn list(conn: &Connection) -> rusqlite::Result<Vec<Recurring>> {
    let mut st = conn.prepare(
        "SELECT r.id,r.name,r.account_id,a.institution,r.category_id,c.color,c.icon,r.amount_cents,r.interval,r.next_date,r.source,r.is_subscription,r.active,r.match_value
         FROM recurring r LEFT JOIN accounts a ON a.id=r.account_id LEFT JOIN categories c ON c.id=r.category_id
         ORDER BY r.active DESC, r.next_date",
    )?;
    let mut prev_st = conn.prepare(
        "SELECT description, amount_cents FROM transactions WHERE account_id IS ? AND ((amount_cents<0) = (?<0)) ORDER BY booking_date DESC LIMIT 400",
    )?;
    let rows: Vec<(Recurring, Option<String>)> = st
        .query_map([], |r| {
            let interval: String = r.get(8)?;
            let amount: i64 = r.get(7)?;
            Ok((
                Recurring {
                    id: r.get(0)?, name: r.get(1)?, account_id: r.get(2)?, account_name: r.get(3)?, category_id: r.get(4)?,
                    category_color: r.get(5)?, category_icon: r.get(6)?, amount_cents: amount, prev_amount_cents: None,
                    yearly_cents: amount * per_year(&interval), interval, next_date: r.get(9)?, source: r.get(10)?,
                    is_subscription: r.get::<_, i64>(11)? != 0, active: r.get::<_, i64>(12)? != 0,
                },
                r.get(13)?,
            ))
        })?
        .collect::<Result<_, _>>()?;
    let mut out = Vec::new();
    for (mut rec, key) in rows {
        // Detección de cambio de precio: penúltimo cargo con la misma clave
        if let Some(key) = key {
            let hits: Vec<i64> = prev_st
                .query_map(params![rec.account_id, rec.amount_cents], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
                .filter_map(|x| x.ok())
                .filter(|(desc, _)| key.split(' ').all(|w| format!(" {} ", rule_norm(desc)).contains(&format!(" {w} "))))
                .map(|(_, a)| a)
                .take(2)
                .collect();
            if hits.len() == 2 && hits[0] != hits[1] {
                rec.prev_amount_cents = Some(hits[1]);
            }
        }
        out.push(rec);
    }
    Ok(out)
}

#[derive(Deserialize)]
pub struct RecurringInput {
    pub name: String,
    pub account_id: Option<String>,
    pub category_id: Option<String>,
    pub amount_cents: i64,
    pub interval: String,
    pub next_date: String,
    pub is_subscription: bool,
}

pub fn upsert(conn: &Connection, id: Option<&str>, i: &RecurringInput) -> rusqlite::Result<String> {
    match id {
        Some(id) => {
            conn.execute(
                "UPDATE recurring SET name=?,account_id=?,category_id=?,amount_cents=?,interval=?,next_date=?,is_subscription=?,source='manual',active=1 WHERE id=?",
                params![i.name, i.account_id, i.category_id, i.amount_cents, i.interval, i.next_date, i.is_subscription as i64, id],
            )?;
            Ok(id.to_string())
        }
        None => {
            let id = uid();
            conn.execute(
                "INSERT INTO recurring(id,name,account_id,category_id,amount_cents,interval,next_date,source,is_subscription) VALUES(?,?,?,?,?,?,?,'manual',?)",
                params![id, i.name, i.account_id, i.category_id, i.amount_cents, i.interval, i.next_date, i.is_subscription as i64],
            )?;
            Ok(id)
        }
    }
}

// ---------------------------------------------------------------- huchas

#[derive(Serialize)]
pub struct SubAlert {
    pub key: String,
    pub kind: &'static str, // new | up
    pub id: String,
    pub name: String,
    pub amount_cents: i64,
    pub prev_cents: Option<i64>,
}

const SEEN: &str = "sub_alerts_seen";

fn seen(conn: &Connection) -> rusqlite::Result<Option<Vec<String>>> {
    let v: Option<String> = conn.query_row("SELECT value FROM settings WHERE key=?", [SEEN], |r| r.get(0)).optional()?;
    Ok(v.map(|v| serde_json::from_str(&v).unwrap_or_default()))
}

/// Suscripciones nuevas y subidas de precio que aún no se han descartado.
/// La primera vez marca como vistas las existentes para no avisar de todo.
pub fn sub_alerts(conn: &Connection) -> rusqlite::Result<Vec<SubAlert>> {
    detect(conn)?;
    let all: Vec<SubAlert> = list(conn)?
        .into_iter()
        .filter(|r| r.active && r.is_subscription && r.amount_cents < 0)
        .map(|r| {
            let up = r.prev_amount_cents.filter(|p| r.amount_cents.abs() > p.abs());
            SubAlert {
                key: match up { Some(_) => format!("up:{}:{}", r.id, r.amount_cents), None => format!("new:{}", r.id) },
                kind: if up.is_some() { "up" } else { "new" },
                id: r.id, name: r.name, amount_cents: r.amount_cents, prev_cents: up,
            }
        })
        .collect();
    match seen(conn)? {
        None => {
            let keys: Vec<String> = all.iter().flat_map(|a| [format!("new:{}", a.id), a.key.clone()]).collect();
            dismiss_alerts(conn, &keys)?;
            Ok(vec![])
        }
        Some(s) => Ok(all.into_iter().filter(|a| !s.contains(&a.key) && (a.kind == "up" || !s.contains(&format!("new:{}", a.id)))).collect()),
    }
}

pub fn dismiss_alerts(conn: &Connection, keys: &[String]) -> rusqlite::Result<()> {
    let mut s = seen(conn)?.unwrap_or_default();
    for k in keys { if !s.contains(k) { s.push(k.clone()) } }
    if s.len() > 500 { s.drain(..s.len() - 500); }
    conn.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![SEEN, serde_json::to_string(&s).unwrap()])?;
    Ok(())
}

#[derive(Serialize)]
pub struct Goal {
    pub id: String,
    pub name: String,
    pub target_cents: i64,
    pub saved_cents: i64,
    pub target_date: Option<String>,
    pub color: String,
    pub monthly_needed_cents: Option<i64>,
}

pub fn goals(conn: &Connection) -> rusqlite::Result<Vec<Goal>> {
    let today = chrono::Local::now().date_naive();
    let mut st = conn.prepare("SELECT id,name,target_cents,saved_cents,target_date,color FROM goals WHERE archived=0 ORDER BY target_date IS NULL, target_date")?;
    let rows = st.query_map([], |r| {
        let target: i64 = r.get(2)?;
        let saved: i64 = r.get(3)?;
        let date: Option<String> = r.get(4)?;
        let monthly = date.as_deref().and_then(d).map(|td| {
            let months = ((td.year() - today.year()) * 12 + td.month() as i32 - today.month() as i32).max(1) as i64;
            ((target - saved).max(0) + months - 1) / months
        });
        Ok(Goal { id: r.get(0)?, name: r.get(1)?, target_cents: target, saved_cents: saved, target_date: date, color: r.get(5)?, monthly_needed_cents: monthly })
    })?;
    rows.collect()
}

// ---------------------------------------------------------------- previsión

#[derive(Serialize)]
pub struct ForecastEvent {
    pub date: String,
    pub name: String,
    pub amount_cents: i64,
    pub recurring_id: String,
}

#[derive(Serialize)]
pub struct Forecast {
    pub start_cents: i64,
    pub daily_variable_cents: i64,
    pub points: Vec<(String, i64)>,
    pub events: Vec<ForecastEvent>,
    pub min_cents: i64,
    pub min_date: String,
    pub end_cents: i64,
}

pub fn forecast(conn: &Connection, days: i64, include_variable: bool) -> rusqlite::Result<Forecast> {
    let start: i64 = conn.query_row("SELECT COALESCE(SUM(balance_cents),0) FROM accounts WHERE archived=0", [], |r| r.get(0))?;
    let today = chrono::Local::now().date_naive();
    let end = today + Days::new(days.clamp(7, 365) as u64);
    let recs: Vec<(String, String, i64, String, String)> = {
        let mut st = conn.prepare("SELECT id,name,amount_cents,interval,next_date FROM recurring WHERE active=1")?;
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?.collect::<Result<_, _>>()?
    };
    let mut events = Vec::new();
    for (id, name, amt, interval, next) in &recs {
        let Some(mut dt) = d(next) else { continue };
        while dt < today {
            dt = advance(dt, interval);
        }
        while dt <= end {
            events.push(ForecastEvent { date: dt.to_string(), name: name.clone(), amount_cents: *amt, recurring_id: id.clone() });
            dt = advance(dt, interval);
        }
    }
    events.sort_by(|a, b| a.date.cmp(&b.date));
    // Gasto variable medio diario (90 días) excluyendo transferencias y lo ya recurrente
    let daily_variable = if include_variable {
        let total_exp: i64 = conn.query_row(
            "SELECT COALESCE(SUM(-t.amount_cents),0) FROM tx_lines t LEFT JOIN categories c ON c.id=t.category_id
             WHERE t.amount_cents<0 AND t.booking_date >= date('now','-90 days') AND COALESCE(c.kind,'')<>'transfer'",
            [],
            |r| r.get(0),
        )?;
        let recurring_month: i64 = recs.iter().filter(|r| r.2 < 0).map(|r| -r.2 * per_year(&r.3) / 12).sum();
        ((total_exp / 3 - recurring_month).max(0)) / 30
    } else {
        0
    };
    let mut bal = start;
    let (mut min, mut min_date) = (start, today.to_string());
    let mut points = vec![(today.to_string(), start)];
    let mut day = today;
    let mut ei = 0;
    while day < end {
        day = day + Days::new(1);
        bal -= daily_variable;
        while ei < events.len() && events[ei].date <= day.to_string() {
            bal += events[ei].amount_cents;
            ei += 1;
        }
        if bal < min {
            min = bal;
            min_date = day.to_string();
        }
        points.push((day.to_string(), bal));
    }
    Ok(Forecast { start_cents: start, daily_variable_cents: daily_variable, points, events, min_cents: min, min_date, end_cents: bal })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_monthly_subscription_and_forecasts() {
        let c = Connection::open_in_memory().unwrap();
        crate::schema::migrate(&c).unwrap();
        c.execute("INSERT INTO accounts(id,name,institution,balance_cents) VALUES('a','C','B',50000)", []).unwrap();
        let today = chrono::Local::now().date_naive();
        for i in 1..=4 {
            let dt = today.checked_sub_months(Months::new(i)).unwrap();
            let amt = if i == 1 { -1299 } else { -1199 };
            c.execute(
                "INSERT INTO transactions(id,account_id,booking_date,amount_cents,description,category_id) VALUES(?,?,?,?,?, 'suscripciones')",
                params![uid(), "a", dt.to_string(), amt, format!("SPOTIFY P{i}ABC")],
            ).unwrap();
        }
        assert_eq!(detect(&c).unwrap(), 1);
        assert_eq!(detect(&c).unwrap(), 0, "idempotente");
        let l = list(&c).unwrap();
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].interval, "monthly");
        assert!(l[0].is_subscription);
        assert_eq!(l[0].amount_cents, -1299);
        assert_eq!(l[0].prev_amount_cents, Some(-1199));
        let f = forecast(&c, 90, false).unwrap();
        assert!(f.events.len() >= 2);
        assert_eq!(f.end_cents, 50000 - 1299 * f.events.len() as i64);
    }

    #[test]
    fn sub_alerts_flow() {
        let c = Connection::open_in_memory().unwrap();
        crate::schema::migrate(&c).unwrap();
        c.execute("INSERT INTO accounts(id,name,institution,balance_cents) VALUES('a','C','B',50000)", []).unwrap();
        let today = chrono::Local::now().date_naive();
        let add = |i: u32, amt: i64, d: &str| {
            let dt = today.checked_sub_months(Months::new(i)).unwrap();
            c.execute("INSERT INTO transactions(id,account_id,booking_date,amount_cents,description,category_id) VALUES(?,?,?,?,?, 'suscripciones')",
                params![uid(), "a", dt.to_string(), amt, d]).unwrap();
        };
        for i in 2..=4 { add(i, -1399, "NETFLIX.COM") }
        assert!(sub_alerts(&c).unwrap().is_empty(), "la primera vez no avisa de lo existente");
        add(1, -1799, "NETFLIX.COM"); // subida del 28 %
        for i in 1..=3 { add(i, -999, "DISNEY PLUS") }
        let a = sub_alerts(&c).unwrap();
        assert!(a.iter().any(|x| x.kind == "up" && x.prev_cents == Some(-1399)), "{:?}", a.iter().map(|x| &x.key).collect::<Vec<_>>());
        assert!(a.iter().any(|x| x.kind == "new" && x.name.contains("Disney")));
        dismiss_alerts(&c, &a.iter().map(|x| x.key.clone()).collect::<Vec<_>>()).unwrap();
        assert!(sub_alerts(&c).unwrap().is_empty());
    }
}