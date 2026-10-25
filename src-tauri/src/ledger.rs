//! Lógica de movimientos: inserción con deduplicado, reglas de categorización,
//! aprendizaje, transferencias internas y consultas.

use crate::import::ParsedTx;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

pub fn uid() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Mayúsculas, sin acentos, solo letras/números separados por un espacio.
pub fn rule_norm(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.to_uppercase().chars() {
        let c = match c {
            'Á' | 'À' | 'Ä' | 'Â' => 'A',
            'É' | 'È' | 'Ë' | 'Ê' => 'E',
            'Í' | 'Ì' | 'Ï' | 'Î' => 'I',
            'Ó' | 'Ò' | 'Ö' | 'Ô' => 'O',
            'Ú' | 'Ù' | 'Ü' | 'Û' => 'U',
            'Ñ' => 'N',
            c if c.is_alphanumeric() => c,
            _ => ' ',
        };
        out.push(c);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn word_contains(hay: &str, needle: &str) -> bool {
    !needle.is_empty() && format!(" {hay} ").contains(&format!(" {needle} "))
}

const STOP: &[&str] = &[
    "COMPRA", "COMPRAS", "EN", "TARJ", "TARJETA", "PAGO", "PAGOS", "RECIBO", "TRANSFERENCIA", "TRANSF", "A", "FAVOR", "DE", "DEL", "LA",
    "EL", "LOS", "CARD", "PAYMENT", "TO", "FROM", "CON", "ADEUDO", "CARGO", "SEPA", "DOMICILIADO", "DOMICILIACION", "INMEDIATA", "RECIBIDA",
    "EMITIDA", "OPERACION", "APPLE", "PAY", "GOOGLE", "CONTACTLESS", "NUM", "REF", "SL", "SA", "SLU", "ES", "WWW", "COM", "Y", "POR",
];

/// Clave para aprender reglas: dos primeras palabras significativas del concepto.
pub fn learn_key(desc: &str) -> Option<String> {
    let n = rule_norm(desc);
    let toks: Vec<&str> = n
        .split(' ')
        .filter(|t| t.len() >= 2 && !t.chars().any(|c| c.is_ascii_digit()) && !STOP.contains(t))
        .take(2)
        .collect();
    if toks.is_empty() { None } else { Some(toks.join(" ")) }
}

#[derive(Clone)]
struct Rule {
    match_field: String,
    match_op: String,
    match_value: String,
    amount_min: Option<i64>,
    amount_max: Option<i64>,
    account_id: Option<String>,
    category_id: Option<String>,
    business: Option<bool>,
}

pub struct Categorizer {
    rules: Vec<Rule>,
    kinds: HashMap<String, String>,
}

impl Categorizer {
    pub fn load(conn: &Connection) -> rusqlite::Result<Self> {
        let mut st = conn.prepare(
            "SELECT match_field,match_op,match_value,amount_min_cents,amount_max_cents,account_id,set_category_id,set_business
             FROM rules WHERE enabled=1 ORDER BY priority, length(match_value) DESC",
        )?;
        let rules = st
            .query_map([], |r| {
                Ok(Rule {
                    match_field: r.get(0)?,
                    match_op: r.get(1)?,
                    match_value: rule_norm(&r.get::<_, String>(2)?),
                    amount_min: r.get(3)?,
                    amount_max: r.get(4)?,
                    account_id: r.get(5)?,
                    category_id: r.get(6)?,
                    business: r.get::<_, Option<i64>>(7)?.map(|b| b != 0),
                })
            })?
            .collect::<Result<_, _>>()?;
        let mut kinds = HashMap::new();
        let mut st = conn.prepare("SELECT id,kind FROM categories")?;
        for row in st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
            let (id, k) = row?;
            kinds.insert(id, k);
        }
        Ok(Self { rules, kinds })
    }

    pub fn categorize(&self, desc: &str, counterparty: Option<&str>, amount: i64, account: &str) -> (Option<String>, Option<bool>) {
        let d = rule_norm(desc);
        let cp = counterparty.map(rule_norm).unwrap_or_default();
        for r in &self.rules {
            let hay = if r.match_field == "counterparty" { &cp } else { &d };
            let ok = match r.match_op.as_str() {
                "words" => r.match_value.split(' ').all(|w| word_contains(hay, w)),
                "equals" => hay == &r.match_value,
                "starts" => hay.starts_with(&r.match_value),
                _ => word_contains(hay, &r.match_value),
            };
            if !ok {
                continue;
            }
            if r.account_id.as_deref().is_some_and(|a| a != account) {
                continue;
            }
            if r.amount_min.is_some_and(|m| amount < m) || r.amount_max.is_some_and(|m| amount > m) {
                continue;
            }
            if let Some(c) = &r.category_id {
                match self.kinds.get(c).map(|s| s.as_str()) {
                    Some("income") if amount < 0 => continue,
                    Some("expense") if amount > 0 => continue,
                    _ => {}
                }
            }
            return (r.category_id.clone(), r.business);
        }
        (None, None)
    }
}

#[derive(Serialize, Default, Debug)]
pub struct ImportResult {
    pub inserted: usize,
    pub duplicates: usize,
    pub categorized: usize,
    pub transfers: usize,
}

fn tx_hash(account: &str, t: &ParsedTx, n: usize) -> String {
    let mut h = Sha256::new();
    match &t.external_id {
        Some(id) => h.update(format!("{account}|ext|{id}")),
        None => h.update(format!("{account}|{}|{}|{}|{n}", t.date, t.amount_cents, rule_norm(&t.description))),
    }
    format!("{:x}", h.finalize())
}

pub fn insert_parsed(conn: &Connection, account_id: &str, txs: &[ParsedTx], set_balance: Option<i64>) -> rusqlite::Result<ImportResult> {
    let cat = Categorizer::load(conn)?;
    let business: bool = conn.query_row("SELECT is_business FROM accounts WHERE id=?", [account_id], |r| r.get::<_, i64>(0))? != 0;
    let tx = conn.unchecked_transaction()?;
    let mut res = ImportResult::default();
    let mut seen: HashMap<(String, i64, String), usize> = HashMap::new();
    let mut sum = 0i64;
    {
        let mut st = tx.prepare(
            "INSERT OR IGNORE INTO transactions(id,account_id,booking_date,value_date,amount_cents,currency,description,category_id,category_source,is_business,external_id,import_hash)
             VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",
        )?;
        for t in txs {
            let k = (t.date.clone(), t.amount_cents, rule_norm(&t.description));
            let n = seen.entry(k).or_insert(0);
            *n += 1;
            let hash = tx_hash(account_id, t, *n);
            let (c, b) = cat.categorize(&t.description, None, t.amount_cents, account_id);
            let src = c.as_ref().map(|_| "rule");
            let changed = st.execute(params![
                uid(), account_id, t.date, t.value_date, t.amount_cents, t.currency, t.description, c, src,
                b.unwrap_or(business) as i64, t.external_id, hash
            ])?;
            if changed == 1 {
                res.inserted += 1;
                sum += t.amount_cents;
                if c.is_some() { res.categorized += 1 }
            } else {
                res.duplicates += 1;
            }
        }
    }
    match set_balance {
        Some(b) => tx.execute("UPDATE accounts SET balance_cents=?, updated_at=datetime('now') WHERE id=?", params![b, account_id])?,
        None => tx.execute("UPDATE accounts SET balance_cents=balance_cents+?, updated_at=datetime('now') WHERE id=?", params![sum, account_id])?,
    };
    tx.commit()?;
    res.transfers = detect_transfers(conn)?;
    Ok(res)
}

/// Empareja movimientos de igual importe y signo contrario entre cuentas propias (±3 días).
pub fn detect_transfers(conn: &Connection) -> rusqlite::Result<usize> {
    let pairs: Vec<(String, String)> = {
        let mut st = conn.prepare(
            "SELECT a.id, b.id FROM transactions a JOIN transactions b
               ON a.amount_cents = -b.amount_cents AND a.amount_cents < 0 AND a.account_id <> b.account_id
              AND abs(julianday(a.booking_date) - julianday(b.booking_date)) <= 3
             WHERE a.transfer_group IS NULL AND b.transfer_group IS NULL
               AND COALESCE(a.category_source,'') <> 'user' AND COALESCE(b.category_source,'') <> 'user'
             ORDER BY abs(julianday(a.booking_date) - julianday(b.booking_date))",
        )?;
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?
    };
    let mut used = std::collections::HashSet::new();
    let mut n = 0;
    for (a, b) in pairs {
        if used.contains(&a) || used.contains(&b) {
            continue;
        }
        used.insert(a.clone());
        used.insert(b.clone());
        let g = uid();
        conn.execute(
            "UPDATE transactions SET transfer_group=?1, category_id='transferencia', category_source='auto' WHERE id IN (?2,?3)",
            params![g, a, b],
        )?;
        n += 1;
    }
    Ok(n)
}

/// Re-aplica las reglas a los movimientos no categorizados por el usuario.
pub fn recategorize(conn: &Connection) -> rusqlite::Result<usize> {
    let cat = Categorizer::load(conn)?;
    let rows: Vec<(String, String, Option<String>, i64, String, Option<String>)> = {
        let mut st = conn.prepare(
            "SELECT id,description,counterparty,amount_cents,account_id,category_id FROM transactions
             WHERE COALESCE(category_source,'') NOT IN ('user','auto')",
        )?;
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?.collect::<Result<_, _>>()?
    };
    let mut n = 0;
    for (id, d, cp, a, acc, old) in rows {
        let (c, _) = cat.categorize(&d, cp.as_deref(), a, &acc);
        if c.is_some() && c != old {
            conn.execute("UPDATE transactions SET category_id=?, category_source='rule' WHERE id=?", params![c, id])?;
            n += 1;
        }
    }
    Ok(n)
}

/// Crea (o actualiza) una regla aprendida a partir de un movimiento y la aplica a los similares.
pub fn learn(conn: &Connection, desc: &str, category_id: &str) -> rusqlite::Result<usize> {
    let Some(key) = learn_key(desc) else { return Ok(0) };
    let existing: Option<String> = conn
        .query_row("SELECT id FROM rules WHERE learned=1 AND match_value=?", [&key], |r| r.get(0))
        .optional()?;
    match existing {
        Some(id) => conn.execute("UPDATE rules SET set_category_id=? WHERE id=?", params![category_id, id])?,
        None => conn.execute(
            "INSERT INTO rules(id,name,priority,match_field,match_op,match_value,set_category_id,learned) VALUES(?,?,10,'description','words',?,?,1)",
            params![uid(), key, key, category_id],
        )?,
    };
    recategorize(conn)
}

#[derive(Serialize)]
pub struct UncatGroup {
    pub key: String,
    pub sample: String,
    pub count: i64,
    pub total_cents: i64,
    pub income: bool,
}

/// Movimientos sin categoría agrupados por comercio (clave aprendible) y signo.
pub fn uncategorized_groups(conn: &Connection) -> rusqlite::Result<Vec<UncatGroup>> {
    let mut st = conn.prepare("SELECT description, amount_cents FROM transactions WHERE category_id IS NULL")?;
    let rows: Vec<(String, i64)> = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
    let mut map: HashMap<(String, bool), UncatGroup> = HashMap::new();
    for (d, a) in rows {
        let key = learn_key(&d).unwrap_or_else(|| "(otros)".into());
        let g = map.entry((key.clone(), a > 0)).or_insert(UncatGroup { key, sample: d.clone(), count: 0, total_cents: 0, income: a > 0 });
        g.count += 1;
        g.total_cents += a;
    }
    let mut v: Vec<UncatGroup> = map.into_values().collect();
    v.sort_by(|a, b| b.count.cmp(&a.count).then(b.total_cents.abs().cmp(&a.total_cents.abs())));
    Ok(v)
}

/// Categoriza todo un grupo: crea la regla aprendida y la aplica. Devuelve cuántos cambiaron.
pub fn categorize_group(conn: &Connection, key: &str, income: bool, category_id: &str) -> rusqlite::Result<usize> {
    if key == "(otros)" {
        return Ok(0);
    }
    let existing: Option<String> = conn
        .query_row("SELECT id FROM rules WHERE learned=1 AND match_value=? AND set_category_id IN (SELECT id FROM categories WHERE (kind='income')=?)",
            params![key, income], |r| r.get(0))
        .optional()?;
    match existing {
        Some(id) => conn.execute("UPDATE rules SET set_category_id=? WHERE id=?", params![category_id, id])?,
        None => conn.execute(
            "INSERT INTO rules(id,name,priority,match_field,match_op,match_value,set_category_id,learned) VALUES(?,?,10,'description','words',?,?,1)",
            params![uid(), key, key, category_id],
        )?,
    };
    recategorize(conn)
}

// ---------------------------------------------------------------- consultas

#[derive(Deserialize, Default, Clone)]
pub struct TxFilter {
    pub search: Option<String>,
    pub account_id: Option<String>,
    pub category_id: Option<String>,
    pub kind: Option<String>, // income | expense | uncategorized | business | transfer
    pub from: Option<String>,
    pub to: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Serialize)]
pub struct TxRow {
    pub id: String,
    pub account_id: String,
    pub account_name: String,
    pub institution: String,
    pub date: String,
    pub amount_cents: i64,
    pub description: String,
    pub notes: Option<String>,
    pub category_id: Option<String>,
    pub category_name: Option<String>,
    pub category_color: Option<String>,
    pub category_icon: Option<String>,
    pub category_source: Option<String>,
    pub is_business: bool,
    pub vat_rate: Option<f64>,
    pub is_transfer: bool,
}

#[derive(Serialize)]
pub struct TxPage {
    pub items: Vec<TxRow>,
    pub total: i64,
    pub income_cents: i64,
    pub expense_cents: i64,
}

pub fn list(conn: &Connection, f: &TxFilter) -> rusqlite::Result<TxPage> {
    let mut wh = vec!["1=1".to_string()];
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = vec![];
    if let Some(s) = f.search.as_ref().filter(|s| !s.trim().is_empty()) {
        wh.push("(t.description LIKE ? OR COALESCE(t.notes,'') LIKE ? OR COALESCE(c.name,'') LIKE ?)".into());
        let p = format!("%{}%", s.trim());
        for _ in 0..3 { args.push(Box::new(p.clone())) }
    }
    if let Some(a) = &f.account_id {
        wh.push("t.account_id = ?".into());
        args.push(Box::new(a.clone()));
    }
    if let Some(c) = &f.category_id {
        wh.push("(t.category_id = ? OR c.parent_id = ?)".into());
        args.push(Box::new(c.clone()));
        args.push(Box::new(c.clone()));
    }
    match f.kind.as_deref() {
        Some("income") => wh.push("t.amount_cents > 0 AND COALESCE(c.kind,'') <> 'transfer'".into()),
        Some("expense") => wh.push("t.amount_cents < 0 AND COALESCE(c.kind,'') <> 'transfer'".into()),
        Some("uncategorized") => wh.push("t.category_id IS NULL".into()),
        Some("business") => wh.push("t.is_business = 1".into()),
        Some("transfer") => wh.push("c.kind = 'transfer'".into()),
        _ => {}
    }
    if let Some(d) = &f.from {
        wh.push("t.booking_date >= ?".into());
        args.push(Box::new(d.clone()));
    }
    if let Some(d) = &f.to {
        wh.push("t.booking_date <= ?".into());
        args.push(Box::new(d.clone()));
    }
    let from = "FROM transactions t JOIN accounts a ON a.id=t.account_id LEFT JOIN categories c ON c.id=t.category_id";
    let w = wh.join(" AND ");
    let refs: Vec<&dyn rusqlite::ToSql> = args.iter().map(|b| b.as_ref()).collect();
    let (total, inc, exp): (i64, i64, i64) = conn.query_row(
        &format!(
            "SELECT COUNT(*), COALESCE(SUM(CASE WHEN t.amount_cents>0 AND COALESCE(c.kind,'')<>'transfer' THEN t.amount_cents END),0),
                    COALESCE(SUM(CASE WHEN t.amount_cents<0 AND COALESCE(c.kind,'')<>'transfer' THEN t.amount_cents END),0) {from} WHERE {w}"
        ),
        refs.as_slice(),
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let sql = format!(
        "SELECT t.id,t.account_id,a.name,a.institution,t.booking_date,t.amount_cents,t.description,t.notes,t.category_id,c.name,c.color,c.icon,
                t.category_source,t.is_business,t.vat_rate,COALESCE(c.kind,'')='transfer'
         {from} WHERE {w} ORDER BY t.booking_date DESC, t.created_at DESC LIMIT {} OFFSET {}",
        f.limit.unwrap_or(200).clamp(1, 2000),
        f.offset.unwrap_or(0).max(0)
    );
    let mut st = conn.prepare(&sql)?;
    let items = st
        .query_map(refs.as_slice(), |r| {
            Ok(TxRow {
                id: r.get(0)?,
                account_id: r.get(1)?,
                account_name: r.get(2)?,
                institution: r.get(3)?,
                date: r.get(4)?,
                amount_cents: r.get(5)?,
                description: r.get(6)?,
                notes: r.get(7)?,
                category_id: r.get(8)?,
                category_name: r.get(9)?,
                category_color: r.get(10)?,
                category_icon: r.get(11)?,
                category_source: r.get(12)?,
                is_business: r.get::<_, i64>(13)? != 0,
                vat_rate: r.get(14)?,
                is_transfer: r.get(15)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(TxPage { items, total, income_cents: inc, expense_cents: exp })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        crate::schema::migrate(&c).unwrap();
        c.execute("INSERT INTO accounts(id,name,institution) VALUES('a','Cuenta','Openbank'),('b','Ahorro','Revolut')", []).unwrap();
        c
    }

    fn ptx(date: &str, desc: &str, amt: i64) -> ParsedTx {
        ParsedTx { date: date.into(), value_date: None, description: desc.into(), amount_cents: amt, balance_cents: None, currency: "EUR".into(), external_id: None }
    }

    #[test]
    fn import_dedupe_rules_transfers_learning() {
        let c = db();
        let txs = vec![
            ptx("2026-10-01", "COMPRA EN MERCADONA CARTAGENA", -5430),
            ptx("2026-10-01", "Café bar", -150),
            ptx("2026-10-01", "Café bar", -150),
            ptx("2026-10-02", "TRANSFERENCIA A REVOLUT", -10000),
            ptx("2026-10-03", "NOMINA OCTUBRE", 150000),
            ptx("2026-10-04", "Pago en LA PELUQUERIA DE LOLA", -2300),
        ];
        let r = insert_parsed(&c, "a", &txs, None).unwrap();
        assert_eq!(r.inserted, 6);
        assert_eq!(r.categorized, 4); // mercadona, 2x bar, nomina
        let r2 = insert_parsed(&c, "a", &txs, None).unwrap();
        assert_eq!((r2.inserted, r2.duplicates), (0, 6));
        let bal: i64 = c.query_row("SELECT balance_cents FROM accounts WHERE id='a'", [], |r| r.get(0)).unwrap();
        assert_eq!(bal, -5430 - 300 - 10000 + 150000 - 2300);

        let r3 = insert_parsed(&c, "b", &[ptx("2026-10-03", "Top-up from Openbank", 10000)], Some(25000)).unwrap();
        assert_eq!(r3.transfers, 1);
        let bal_b: i64 = c.query_row("SELECT balance_cents FROM accounts WHERE id='b'", [], |r| r.get(0)).unwrap();
        assert_eq!(bal_b, 25000);

        assert_eq!(learn_key("Pago en LA PELUQUERIA DE LOLA").as_deref(), Some("PELUQUERIA LOLA"));
        insert_parsed(&c, "a", &[ptx("2026-10-09", "PAGO EN PELUQUERIA LOLA CARTAGENA", -1800)], None).unwrap();
        let n = learn(&c, "Pago en LA PELUQUERIA DE LOLA", "salud").unwrap();
        assert_eq!(n, 2);

        insert_parsed(&c, "a", &[ptx("2026-10-10", "BIZUM DE ANA CENA", 1500), ptx("2026-10-11", "BIZUM A PEPE", -900),
            ptx("2026-10-12", "PANADERIA LA ESPIGA", -150), ptx("2026-10-13", "PANADERIA LA ESPIGA", -180)], None).unwrap();
        let cat_of = |d: &str| -> Option<String> { c.query_row("SELECT category_id FROM transactions WHERE description=?", [d], |r| r.get(0)).unwrap() };
        assert_eq!(cat_of("BIZUM DE ANA CENA").as_deref(), Some("bizum-recibido"));
        assert_eq!(cat_of("BIZUM A PEPE").as_deref(), Some("bizum-enviado"));
        let g = uncategorized_groups(&c).unwrap();
        let pan = g.iter().find(|x| x.key == "PANADERIA ESPIGA").unwrap();
        assert_eq!(pan.count, 2);
        assert_eq!(categorize_group(&c, "PANADERIA ESPIGA", false, "supermercado").unwrap(), 2);

        let page = list(&c, &TxFilter { kind: Some("expense".into()), ..Default::default() }).unwrap();
        assert!(page.items.iter().all(|t| t.amount_cents < 0 && !t.is_transfer));
        let page = list(&c, &TxFilter { search: Some("mercadona".into()), ..Default::default() }).unwrap();
        assert_eq!(page.total, 1);
    }
}
