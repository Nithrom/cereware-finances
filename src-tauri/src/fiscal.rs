//! Estimación trimestral para autónomos en estimación directa: modelo 303 (IVA) y 130 (IRPF).
//! Es una estimación orientativa a partir de los movimientos marcados como de autónomo.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone)]
pub struct FiscalConfig {
    /// IVA por defecto de las facturas emitidas (%)
    pub income_vat: f64,
    /// Retención de IRPF que aplican tus clientes (%) — 0, 7 o 15
    pub income_retention: f64,
    /// IVA por defecto de los gastos sin tipo indicado (%)
    pub expense_vat: f64,
}

impl Default for FiscalConfig {
    fn default() -> Self {
        Self { income_vat: 21.0, income_retention: 0.0, expense_vat: 0.0 }
    }
}

pub fn load_config(conn: &Connection) -> FiscalConfig {
    conn.query_row("SELECT value FROM settings WHERE key='fiscal'", [], |r| r.get::<_, String>(0))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

#[derive(Serialize, Default, Clone)]
pub struct Quarter {
    pub year: i32,
    pub quarter: u32,
    pub deadline: String,
    pub income_count: i64,
    pub expense_count: i64,
    pub income_base_cents: i64,
    pub vat_output_cents: i64,
    pub retention_cents: i64,
    pub expense_base_cents: i64,
    pub vat_input_cents: i64,
    pub non_vat_expenses_cents: i64,
    /// Modelo 303: IVA repercutido − soportado
    pub m303_cents: i64,
    /// Rendimiento neto acumulado en el año hasta este trimestre
    pub ytd_profit_cents: i64,
    /// Modelo 130: 20 % del rendimiento acumulado − retenciones acumuladas − pagos previos
    pub m130_cents: i64,
}

fn deadline(year: i32, q: u32) -> String {
    match q {
        1 => format!("{year}-04-20"),
        2 => format!("{year}-07-20"),
        3 => format!("{year}-10-20"),
        _ => format!("{}-01-30", year + 1),
    }
}

fn quarter_raw(conn: &Connection, year: i32, q: u32, cfg: &FiscalConfig) -> rusqlite::Result<Quarter> {
    let from = format!("{year}-{:02}-01", (q - 1) * 3 + 1);
    let to = format!("{year}-{:02}-31", q * 3);
    let mut st = conn.prepare(
        "SELECT t.amount_cents, t.vat_rate, COALESCE(t.category_id,'') FROM transactions t
         LEFT JOIN categories c ON c.id=t.category_id
         WHERE t.is_business=1 AND t.booking_date BETWEEN ? AND ? AND COALESCE(c.kind,'') <> 'transfer'",
    )?;
    let rows: Vec<(i64, Option<f64>, String)> = st.query_map(params![from, to], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<Result<_, _>>()?;
    let mut out = Quarter { year, quarter: q, deadline: deadline(year, q), ..Default::default() };
    for (amt, vat, cat) in rows {
        let a = amt.abs() as f64;
        if amt > 0 {
            let v = vat.unwrap_or(cfg.income_vat) / 100.0;
            let ret = cfg.income_retention / 100.0;
            let base = a / (1.0 + v - ret);
            out.income_count += 1;
            out.income_base_cents += base.round() as i64;
            out.vat_output_cents += (base * v).round() as i64;
            out.retention_cents += (base * ret).round() as i64;
        } else if cat == "impuestos" {
            // Pagos a Hacienda: ni gasto deducible ni IVA soportado
        } else if cat == "cuota-reta" {
            out.expense_count += 1;
            out.non_vat_expenses_cents += a.round() as i64;
        } else {
            let v = vat.unwrap_or(cfg.expense_vat) / 100.0;
            let base = a / (1.0 + v);
            out.expense_count += 1;
            out.expense_base_cents += base.round() as i64;
            out.vat_input_cents += (a - base).round() as i64;
        }
    }
    out.m303_cents = out.vat_output_cents - out.vat_input_cents;
    Ok(out)
}

/// Los cuatro trimestres del año con el 130 acumulado.
pub fn year(conn: &Connection, year: i32) -> rusqlite::Result<Vec<Quarter>> {
    let cfg = load_config(conn);
    let mut out: Vec<Quarter> = Vec::new();
    let (mut ytd_profit, mut ytd_ret, mut paid) = (0i64, 0i64, 0i64);
    for q in 1..=4 {
        let mut r = quarter_raw(conn, year, q, &cfg)?;
        ytd_profit += r.income_base_cents - r.expense_base_cents - r.non_vat_expenses_cents;
        ytd_ret += r.retention_cents;
        r.ytd_profit_cents = ytd_profit;
        let due = ((ytd_profit as f64 * 0.20).round() as i64 - ytd_ret - paid).max(0);
        r.m130_cents = due;
        paid += due;
        out.push(r);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quarter_vat_and_irpf() {
        let c = Connection::open_in_memory().unwrap();
        crate::schema::migrate(&c).unwrap();
        c.execute("INSERT INTO accounts(id,name,institution,is_business) VALUES('a','C','B',1)", []).unwrap();
        let ins = |date: &str, amt: i64, cat: Option<&str>, vat: Option<f64>| {
            c.execute(
                "INSERT INTO transactions(id,account_id,booking_date,amount_cents,description,category_id,is_business,vat_rate) VALUES(?,?,?,?,?,?,1,?)",
                params![crate::ledger::uid(), "a", date, amt, "x", cat, vat],
            ).unwrap();
        };
        ins("2026-01-15", 121000, Some("facturacion"), None); // base 1000, IVA 210
        ins("2026-02-10", -24200, Some("tecnologia"), Some(21.0)); // base 200, IVA 42
        ins("2026-02-28", -30000, Some("cuota-reta"), None);
        ins("2026-03-20", -50000, Some("impuestos"), None);
        let y = year(&c, 2026).unwrap();
        let q1 = &y[0];
        assert_eq!(q1.income_base_cents, 100000);
        assert_eq!(q1.vat_output_cents, 21000);
        assert_eq!(q1.vat_input_cents, 4200);
        assert_eq!(q1.m303_cents, 16800);
        assert_eq!(q1.ytd_profit_cents, 100000 - 20000 - 30000);
        assert_eq!(q1.m130_cents, 10000);
        assert_eq!(y[1].m130_cents, 0);
        assert_eq!(q1.deadline, "2026-04-20");
    }
}
