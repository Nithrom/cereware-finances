//! Consultas agregadas: panel, presupuestos, informes.

use rusqlite::{params, Connection};
use serde::Serialize;

/// Excluye transferencias internas de ingresos/gastos.
const NOT_TRANSFER: &str = "COALESCE(c.kind,'') <> 'transfer'";

#[derive(Serialize)]
pub struct Point {
    pub month: String,
    pub value_cents: i64,
}

#[derive(Serialize)]
pub struct CatSlice {
    pub id: String,
    pub name: String,
    pub color: String,
    pub icon: String,
    pub cents: i64,
}

#[derive(Serialize)]
pub struct MonthFlow {
    pub month: String,
    pub income_cents: i64,
    pub expense_cents: i64,
}

#[derive(Serialize)]
pub struct Dashboard {
    pub total_balance_cents: i64,
    pub accounts: i64,
    pub transactions: i64,
    pub uncategorized: i64,
    pub month: String,
    pub month_income_cents: i64,
    pub month_expense_cents: i64,
    pub avg_expense_cents: i64,
    pub avg_income_cents: i64,
    pub balance_history: Vec<Point>,
    pub flows: Vec<MonthFlow>,
    pub by_category: Vec<CatSlice>,
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn month_now(conn: &Connection) -> rusqlite::Result<String> {
    conn.query_row("SELECT strftime('%Y-%m','now','localtime')", [], |r| r.get(0))
}

/// Últimos `n` meses (incluido el actual), del más antiguo al más reciente.
pub fn last_months(conn: &Connection, n: i64) -> rusqlite::Result<Vec<String>> {
    (0..n)
        .rev()
        .map(|i| conn.query_row(&format!("SELECT strftime('%Y-%m','now','localtime','start of month','-{i} months')"), [], |r| r.get(0)))
        .collect()
}

pub fn flows(conn: &Connection, months: &[String]) -> rusqlite::Result<Vec<MonthFlow>> {
    let mut st = conn.prepare(&format!(
        "SELECT COALESCE(SUM(CASE WHEN t.amount_cents>0 THEN t.amount_cents END),0),
                COALESCE(SUM(CASE WHEN t.amount_cents<0 THEN -t.amount_cents END),0)
         FROM transactions t LEFT JOIN categories c ON c.id=t.category_id
         WHERE substr(t.booking_date,1,7)=? AND {NOT_TRANSFER}"
    ))?;
    months
        .iter()
        .map(|m| st.query_row([m], |r| Ok(MonthFlow { month: m.clone(), income_cents: r.get(0)?, expense_cents: r.get(1)? })))
        .collect()
}

pub fn by_category(conn: &Connection, from: &str, to: &str, kind: &str) -> rusqlite::Result<Vec<CatSlice>> {
    let sign = if kind == "income" { "t.amount_cents > 0" } else { "t.amount_cents < 0" };
    let mut st = conn.prepare(&format!(
        "SELECT COALESCE(p.id, c.id, '_none'), COALESCE(p.name, c.name, 'Sin categoría'), COALESCE(p.color, c.color, '#94A3B8'),
                COALESCE(p.icon, c.icon, 'tag'), SUM(abs(t.amount_cents)) s
         FROM transactions t LEFT JOIN categories c ON c.id=t.category_id LEFT JOIN categories p ON p.id=c.parent_id
         WHERE t.booking_date >= ? AND t.booking_date <= ? AND {sign} AND {NOT_TRANSFER}
         GROUP BY 1 ORDER BY s DESC"
    ))?;
    let rows = st.query_map([from, to], |r| Ok(CatSlice { id: r.get(0)?, name: r.get(1)?, color: r.get(2)?, icon: r.get(3)?, cents: r.get(4)? }))?;
    rows.collect()
}

pub fn dashboard(conn: &Connection) -> rusqlite::Result<Dashboard> {
    let total: i64 = conn.query_row("SELECT COALESCE(SUM(balance_cents),0) FROM accounts WHERE archived=0", [], |r| r.get(0))?;
    let accounts: i64 = conn.query_row("SELECT COUNT(*) FROM accounts WHERE archived=0", [], |r| r.get(0))?;
    let transactions: i64 = conn.query_row("SELECT COUNT(*) FROM transactions", [], |r| r.get(0))?;
    let uncategorized: i64 = conn.query_row("SELECT COUNT(*) FROM transactions WHERE category_id IS NULL", [], |r| r.get(0))?;
    let months = last_months(conn, 6)?;
    let month = months.last().cloned().unwrap_or_default();
    let fl = flows(conn, &months)?;
    let cur = fl.last();
    let prev: Vec<&MonthFlow> = fl.iter().rev().skip(1).take(3).filter(|f| f.expense_cents > 0 || f.income_cents > 0).collect();
    let avg = |f: &dyn Fn(&MonthFlow) -> i64| if prev.is_empty() { 0 } else { prev.iter().map(|m| f(m)).sum::<i64>() / prev.len() as i64 };
    // Saldo al final de cada mes = saldo actual − movimientos posteriores
    let mut st = conn.prepare(
        "SELECT COALESCE(SUM(t.amount_cents),0) FROM transactions t JOIN accounts a ON a.id=t.account_id
         WHERE a.archived=0 AND t.booking_date > date(?||'-01','+1 month','-1 day')",
    )?;
    let mut history = Vec::new();
    for m in &months {
        let after: i64 = st.query_row([m], |r| r.get(0))?;
        history.push(Point { month: m.clone(), value_cents: total - after });
    }
    let from = format!("{month}-01");
    let to = format!("{month}-31");
    Ok(Dashboard {
        total_balance_cents: total,
        accounts,
        transactions,
        uncategorized,
        month: month.clone(),
        month_income_cents: cur.map_or(0, |c| c.income_cents),
        month_expense_cents: cur.map_or(0, |c| c.expense_cents),
        avg_expense_cents: avg(&|m| m.expense_cents),
        avg_income_cents: avg(&|m| m.income_cents),
        balance_history: history,
        flows: fl,
        by_category: by_category(conn, &from, &to, "expense")?,
    })
}

// ---------------------------------------------------------------- presupuestos

#[derive(Serialize)]
pub struct BudgetRow {
    pub category_id: String,
    pub name: String,
    pub color: String,
    pub icon: String,
    pub budget_cents: Option<i64>,
    pub spent_cents: i64,
    pub avg_cents: i64,
}

/// Una fila por categoría principal de gasto: presupuesto (del mes o por defecto), gastado y media de 3 meses.
pub fn budgets(conn: &Connection, month: &str) -> rusqlite::Result<Vec<BudgetRow>> {
    let mut st = conn.prepare(&format!(
        "WITH spent AS (
           SELECT COALESCE(c.parent_id, c.id) cat, substr(t.booking_date,1,7) m, SUM(-t.amount_cents) s
           FROM transactions t JOIN categories c ON c.id=t.category_id
           WHERE t.amount_cents < 0 AND {NOT_TRANSFER} GROUP BY 1,2)
         SELECT p.id, p.name, p.color, p.icon,
           COALESCE((SELECT amount_cents FROM budgets WHERE category_id=p.id AND month=?1),
                    (SELECT amount_cents FROM budgets WHERE category_id=p.id AND month IS NULL)),
           COALESCE((SELECT s FROM spent WHERE cat=p.id AND m=?1),0),
           COALESCE((SELECT SUM(s) FROM spent WHERE cat=p.id AND m < ?1 AND m >= strftime('%Y-%m', ?1||'-01', '-3 months')),0)/3
         FROM categories p WHERE p.parent_id IS NULL AND p.kind='expense' ORDER BY p.sort"
    ))?;
    let rows = st.query_map([month], |r| {
        Ok(BudgetRow {
            category_id: r.get(0)?, name: r.get(1)?, color: r.get(2)?, icon: r.get(3)?,
            budget_cents: r.get(4)?, spent_cents: r.get(5)?, avg_cents: r.get(6)?,
        })
    })?;
    rows.collect()
}

pub fn budget_set(conn: &Connection, category_id: &str, month: Option<&str>, amount: Option<i64>) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM budgets WHERE category_id=? AND month IS ?", params![category_id, month])?;
    if let Some(a) = amount.filter(|a| *a > 0) {
        conn.execute(
            "INSERT INTO budgets(id,category_id,month,amount_cents) VALUES(?,?,?,?)",
            params![crate::ledger::uid(), category_id, month, a],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboard_and_budgets() {
        let c = Connection::open_in_memory().unwrap();
        crate::schema::migrate(&c).unwrap();
        c.execute("INSERT INTO accounts(id,name,institution,balance_cents) VALUES('a','C','B',100000)", []).unwrap();
        let m = month_now(&c).unwrap();
        for (d, amt, cat) in [("05", -3000, "supermercado"), ("06", -2000, "restaurantes"), ("07", 150000, "nomina"), ("08", -500, "transferencia")] {
            c.execute(
                "INSERT INTO transactions(id,account_id,booking_date,amount_cents,description,category_id) VALUES(?,?,?,?,?,?)",
                params![crate::ledger::uid(), "a", format!("{m}-{d}"), amt, "x", cat],
            ).unwrap();
        }
        let d = dashboard(&c).unwrap();
        assert_eq!(d.month_expense_cents, 5000);
        assert_eq!(d.month_income_cents, 150000);
        assert_eq!(d.by_category[0].id, "alimentacion");
        assert_eq!(d.by_category[0].cents, 5000);
        assert_eq!(d.balance_history.last().unwrap().value_cents, 100000);
        budget_set(&c, "alimentacion", None, Some(40000)).unwrap();
        let b = budgets(&c, &m).unwrap();
        let al = b.iter().find(|r| r.category_id == "alimentacion").unwrap();
        assert_eq!((al.budget_cents, al.spent_cents), (Some(40000), 5000));
    }
}
