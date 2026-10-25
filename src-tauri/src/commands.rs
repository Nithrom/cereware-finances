use crate::vault::{Vault, VaultError};
use crate::AppState;
use serde::Serialize;
use tauri::State;

type R<T> = Result<T, VaultError>;

fn with_vault<T>(state: &State<AppState>, f: impl FnOnce(&Vault) -> R<T>) -> R<T> {
    let guard = state.vault.lock().unwrap();
    let v = guard.as_ref().ok_or(VaultError::Locked)?;
    f(v)
}

#[derive(Serialize)]
pub struct VaultStatus {
    exists: bool,
    unlocked: bool,
}

#[tauri::command]
pub fn vault_status(state: State<AppState>) -> VaultStatus {
    VaultStatus { exists: Vault::exists(&state.path), unlocked: state.vault.lock().unwrap().is_some() }
}

// Argon2 es costoso a propósito: se ejecuta fuera del hilo principal.
#[tauri::command]
pub async fn vault_create(state: State<'_, AppState>, password: String) -> R<()> {
    let path = state.path.clone();
    let v = tauri::async_runtime::spawn_blocking(move || Vault::create(&path, &password)).await.map_err(|_| VaultError::Crypto)??;
    *state.vault.lock().unwrap() = Some(v);
    Ok(())
}

#[tauri::command]
pub async fn vault_unlock(state: State<'_, AppState>, password: String) -> R<()> {
    let path = state.path.clone();
    let v = tauri::async_runtime::spawn_blocking(move || Vault::open(&path, &password)).await.map_err(|_| VaultError::Crypto)??;
    *state.vault.lock().unwrap() = Some(v);
    Ok(())
}

#[tauri::command]
pub fn vault_lock(state: State<AppState>) -> R<()> {
    let mut g = state.vault.lock().unwrap();
    if let Some(v) = g.as_ref() {
        v.save()?;
    }
    *g = None;
    Ok(())
}

#[tauri::command]
pub fn settings_get(state: State<AppState>, key: String) -> R<Option<String>> {
    with_vault(&state, |v| {
        let r = v.conn.query_row("SELECT value FROM settings WHERE key=?", [&key], |r| r.get(0));
        match r {
            Ok(s) => Ok(Some(s)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    })
}

#[tauri::command]
pub fn settings_set(state: State<AppState>, key: String, value: String) -> R<()> {
    with_vault(&state, |v| {
        v.conn.execute(
            "INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [&key, &value],
        )?;
        v.save()
    })
}

#[derive(Serialize)]
pub struct Account {
    id: String,
    name: String,
    institution: String,
    kind: String,
    currency: String,
    balance_cents: i64,
    provider: String,
    consent_expires_at: Option<String>,
    is_business: bool,
    color: Option<String>,
}

#[tauri::command]
pub fn accounts_list(state: State<AppState>) -> R<Vec<Account>> {
    with_vault(&state, |v| {
        let mut st = v.conn.prepare(
            "SELECT id,name,institution,kind,currency,balance_cents,provider,consent_expires_at,is_business,color
             FROM accounts WHERE archived=0 ORDER BY institution,name",
        )?;
        let rows = st.query_map([], |r| {
            Ok(Account {
                id: r.get(0)?,
                name: r.get(1)?,
                institution: r.get(2)?,
                kind: r.get(3)?,
                currency: r.get(4)?,
                balance_cents: r.get(5)?,
                provider: r.get(6)?,
                consent_expires_at: r.get(7)?,
                is_business: r.get::<_, i64>(8)? != 0,
                color: r.get(9)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    })
}

#[derive(Serialize)]
pub struct Category {
    id: String,
    parent_id: Option<String>,
    name: String,
    kind: String,
    color: String,
    icon: String,
}

#[tauri::command]
pub fn categories_list(state: State<AppState>) -> R<Vec<Category>> {
    with_vault(&state, |v| {
        let mut st = v.conn.prepare("SELECT id,parent_id,name,kind,color,icon FROM categories ORDER BY sort")?;
        let rows = st.query_map([], |r| {
            Ok(Category { id: r.get(0)?, parent_id: r.get(1)?, name: r.get(2)?, kind: r.get(3)?, color: r.get(4)?, icon: r.get(5)? })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    })
}

#[derive(Serialize)]
pub struct DashboardSummary {
    total_balance_cents: i64,
    month_income_cents: i64,
    month_expense_cents: i64,
    accounts: i64,
    transactions: i64,
}

#[tauri::command]
pub fn dashboard_summary(state: State<AppState>) -> R<DashboardSummary> {
    with_vault(&state, |v| {
        let c = &v.conn;
        let total: i64 = c.query_row("SELECT COALESCE(SUM(balance_cents),0) FROM accounts WHERE archived=0", [], |r| r.get(0))?;
        let (inc, exp): (i64, i64) = c.query_row(
            "SELECT COALESCE(SUM(CASE WHEN t.amount_cents>0 THEN t.amount_cents END),0),
                    COALESCE(SUM(CASE WHEN t.amount_cents<0 THEN t.amount_cents END),0)
             FROM transactions t LEFT JOIN categories c ON c.id=t.category_id
             WHERE strftime('%Y-%m',t.booking_date)=strftime('%Y-%m','now') AND COALESCE(c.kind,'')<>'transfer'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let accounts: i64 = c.query_row("SELECT COUNT(*) FROM accounts WHERE archived=0", [], |r| r.get(0))?;
        let transactions: i64 = c.query_row("SELECT COUNT(*) FROM transactions", [], |r| r.get(0))?;
        Ok(DashboardSummary { total_balance_cents: total, month_income_cents: inc, month_expense_cents: exp, accounts, transactions })
    })
}
