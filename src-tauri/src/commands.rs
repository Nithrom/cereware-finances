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

// ---------------------------------------------------------------- escritura (guarda tras cada cambio)

fn with_vault_save<T>(state: &State<AppState>, f: impl FnOnce(&Vault) -> R<T>) -> R<T> {
    with_vault(state, |v| {
        let r = f(v)?;
        v.save()?;
        Ok(r)
    })
}

fn err(msg: impl Into<String>) -> VaultError {
    VaultError::Msg(msg.into())
}

#[derive(serde::Deserialize)]
pub struct AccountInput {
    name: String,
    institution: String,
    kind: Option<String>,
    balance_cents: Option<i64>,
    is_business: Option<bool>,
    color: Option<String>,
}

#[tauri::command]
pub fn account_create(state: State<AppState>, input: AccountInput) -> R<String> {
    if input.name.trim().is_empty() || input.institution.trim().is_empty() {
        return Err(err("El nombre y el banco son obligatorios"));
    }
    with_vault_save(&state, |v| {
        let id = crate::ledger::uid();
        v.conn.execute(
            "INSERT INTO accounts(id,name,institution,kind,balance_cents,is_business,color) VALUES(?,?,?,?,?,?,?)",
            rusqlite::params![id, input.name.trim(), input.institution.trim(), input.kind.unwrap_or("checking".into()),
                input.balance_cents.unwrap_or(0), input.is_business.unwrap_or(false) as i64, input.color],
        )?;
        Ok(id)
    })
}

#[tauri::command]
pub fn account_update(state: State<AppState>, id: String, input: AccountInput) -> R<()> {
    with_vault_save(&state, |v| {
        v.conn.execute(
            "UPDATE accounts SET name=?,institution=?,kind=COALESCE(?,kind),balance_cents=COALESCE(?,balance_cents),
             is_business=COALESCE(?,is_business),color=?,updated_at=datetime('now') WHERE id=?",
            rusqlite::params![input.name.trim(), input.institution.trim(), input.kind, input.balance_cents,
                input.is_business.map(|b| b as i64), input.color, id],
        )?;
        Ok(())
    })
}

#[tauri::command]
pub fn account_delete(state: State<AppState>, id: String) -> R<()> {
    with_vault_save(&state, |v| {
        v.conn.execute("DELETE FROM accounts WHERE id=?", [id])?;
        Ok(())
    })
}

#[tauri::command]
pub async fn import_preview(path: String, mapping: Option<crate::import::Mapping>) -> R<crate::import::Preview> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::import::parse_file(std::path::Path::new(&path), mapping).map(|p| p.preview(40)).map_err(|e| err(e.to_string()))
    })
    .await
    .map_err(|_| err("Error interno"))?
}

#[tauri::command]
pub fn import_commit(
    state: State<AppState>,
    path: String,
    account_id: String,
    mapping: Option<crate::import::Mapping>,
    use_balance: bool,
) -> R<crate::ledger::ImportResult> {
    let parsed = crate::import::parse_file(std::path::Path::new(&path), mapping).map_err(|e| err(e.to_string()))?;
    if parsed.txs.is_empty() {
        return Err(err("El fichero no contiene movimientos válidos"));
    }
    let bal = if use_balance { crate::import::latest_balance(&parsed.txs) } else { None };
    with_vault_save(&state, |v| {
        let r = crate::ledger::insert_parsed(&v.conn, &account_id, &parsed.txs, bal)?;
        v.conn.execute("UPDATE accounts SET provider=CASE WHEN provider='manual' THEN 'import' ELSE provider END WHERE id=?", [&account_id])?;
        Ok(r)
    })
}

#[tauri::command]
pub fn tx_list(state: State<AppState>, filter: crate::ledger::TxFilter) -> R<crate::ledger::TxPage> {
    with_vault(&state, |v| Ok(crate::ledger::list(&v.conn, &filter)?))
}

#[derive(serde::Deserialize)]
pub struct TxPatch {
    /// Some(None) = quitar categoría
    #[serde(default, deserialize_with = "double_option")]
    category_id: Option<Option<String>>,
    notes: Option<String>,
    is_business: Option<bool>,
    vat_rate: Option<f64>,
    #[serde(default)]
    learn: bool,
}

#[derive(Serialize)]
pub struct TxUpdateResult {
    learned_applied: usize,
}

#[tauri::command]
pub fn tx_update(state: State<AppState>, id: String, patch: TxPatch) -> R<TxUpdateResult> {
    with_vault_save(&state, |v| {
        let c = &v.conn;
        let mut learned_applied = 0;
        if let Some(cat) = &patch.category_id {
            c.execute(
                "UPDATE transactions SET category_id=?, category_source=CASE WHEN ? IS NULL THEN NULL ELSE 'user' END, updated_at=datetime('now') WHERE id=?",
                rusqlite::params![cat, cat, id],
            )?;
            if let (Some(cat), true) = (cat, patch.learn) {
                let desc: String = c.query_row("SELECT description FROM transactions WHERE id=?", [&id], |r| r.get(0))?;
                learned_applied = crate::ledger::learn(c, &desc, cat)?;
            }
        }
        if let Some(n) = &patch.notes {
            c.execute("UPDATE transactions SET notes=NULLIF(?, ''), updated_at=datetime('now') WHERE id=?", rusqlite::params![n, id])?;
        }
        if let Some(b) = patch.is_business {
            c.execute("UPDATE transactions SET is_business=?, updated_at=datetime('now') WHERE id=?", rusqlite::params![b as i64, id])?;
        }
        if let Some(r) = patch.vat_rate {
            c.execute("UPDATE transactions SET vat_rate=NULLIF(?, -1), updated_at=datetime('now') WHERE id=?", rusqlite::params![r, id])?;
        }
        Ok(TxUpdateResult { learned_applied })
    })
}

#[derive(serde::Deserialize)]
pub struct TxInput {
    account_id: String,
    date: String,
    amount_cents: i64,
    description: String,
    category_id: Option<String>,
    notes: Option<String>,
}

#[tauri::command]
pub fn tx_create(state: State<AppState>, input: TxInput) -> R<String> {
    if crate::import::parse_date(&input.date).is_none() || input.amount_cents == 0 || input.description.trim().is_empty() {
        return Err(err("Revisa la fecha, el importe y el concepto"));
    }
    with_vault_save(&state, |v| {
        let id = crate::ledger::uid();
        let tx = v.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO transactions(id,account_id,booking_date,amount_cents,description,category_id,category_source,notes,is_business)
             SELECT ?,?,?,?,?,?,CASE WHEN ? IS NULL THEN NULL ELSE 'user' END,?,is_business FROM accounts WHERE id=?",
            rusqlite::params![id, input.account_id, input.date, input.amount_cents, input.description.trim(), input.category_id,
                input.category_id, input.notes, input.account_id],
        )?;
        tx.execute("UPDATE accounts SET balance_cents=balance_cents+? WHERE id=?", rusqlite::params![input.amount_cents, input.account_id])?;
        tx.commit()?;
        Ok(id)
    })
}

#[tauri::command]
pub fn tx_delete(state: State<AppState>, id: String, adjust_balance: bool) -> R<()> {
    with_vault_save(&state, |v| {
        let tx = v.conn.unchecked_transaction()?;
        if adjust_balance {
            tx.execute(
                "UPDATE accounts SET balance_cents=balance_cents-(SELECT amount_cents FROM transactions WHERE id=?1)
                 WHERE id=(SELECT account_id FROM transactions WHERE id=?1)",
                [&id],
            )?;
        }
        tx.execute("DELETE FROM transactions WHERE id=?", [&id])?;
        tx.commit()?;
        Ok(())
    })
}

#[derive(Serialize)]
pub struct RuleRow {
    id: String,
    match_value: String,
    match_op: String,
    category_id: Option<String>,
    category_name: Option<String>,
    learned: bool,
    enabled: bool,
}

#[tauri::command]
pub fn rules_list(state: State<AppState>) -> R<Vec<RuleRow>> {
    with_vault(&state, |v| {
        let mut st = v.conn.prepare(
            "SELECT r.id,r.match_value,r.match_op,r.set_category_id,c.name,r.learned,r.enabled FROM rules r
             LEFT JOIN categories c ON c.id=r.set_category_id ORDER BY r.learned DESC, r.priority, r.match_value",
        )?;
        let rows = st.query_map([], |r| {
            Ok(RuleRow {
                id: r.get(0)?, match_value: r.get(1)?, match_op: r.get(2)?, category_id: r.get(3)?, category_name: r.get(4)?,
                learned: r.get::<_, i64>(5)? != 0, enabled: r.get::<_, i64>(6)? != 0,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    })
}

#[tauri::command]
pub fn rule_create(state: State<AppState>, match_value: String, category_id: String) -> R<usize> {
    if match_value.trim().len() < 2 {
        return Err(err("El texto de la regla es demasiado corto"));
    }
    with_vault_save(&state, |v| {
        v.conn.execute(
            "INSERT INTO rules(id,name,priority,match_field,match_op,match_value,set_category_id,learned) VALUES(?,?,5,'description','words',?,?,1)",
            rusqlite::params![crate::ledger::uid(), match_value.trim(), match_value.trim(), category_id],
        )?;
        Ok(crate::ledger::recategorize(&v.conn)?)
    })
}

#[tauri::command]
pub fn rule_delete(state: State<AppState>, id: String) -> R<()> {
    with_vault_save(&state, |v| {
        v.conn.execute("DELETE FROM rules WHERE id=?", [id])?;
        Ok(())
    })
}

fn double_option<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Option<String>>, D::Error> {
    Ok(Some(serde::Deserialize::deserialize(d)?))
}

#[tauri::command]
pub fn dashboard_full(state: State<AppState>) -> R<crate::analytics::Dashboard> {
    with_vault(&state, |v| Ok(crate::analytics::dashboard(&v.conn)?))
}

#[tauri::command]
pub fn budgets_list(state: State<AppState>, month: String) -> R<Vec<crate::analytics::BudgetRow>> {
    with_vault(&state, |v| Ok(crate::analytics::budgets(&v.conn, &month)?))
}

#[tauri::command]
pub fn budget_set(state: State<AppState>, category_id: String, month: Option<String>, amount_cents: Option<i64>) -> R<()> {
    with_vault_save(&state, |v| Ok(crate::analytics::budget_set(&v.conn, &category_id, month.as_deref(), amount_cents)?))
}

#[tauri::command]
pub fn report_categories(state: State<AppState>, from: String, to: String, kind: String) -> R<Vec<crate::analytics::CatSlice>> {
    with_vault(&state, |v| Ok(crate::analytics::by_category(&v.conn, &from, &to, &kind)?))
}

#[tauri::command]
pub fn report_flows(state: State<AppState>, months: i64) -> R<Vec<crate::analytics::MonthFlow>> {
    with_vault(&state, |v| {
        let ms = crate::analytics::last_months(&v.conn, months.clamp(1, 36))?;
        Ok(crate::analytics::flows(&v.conn, &ms)?)
    })
}

// ---------------------------------------------------------------- planificación

#[tauri::command]
pub fn recurring_list(state: State<AppState>) -> R<Vec<crate::planning::Recurring>> {
    with_vault_save(&state, |v| {
        crate::planning::detect(&v.conn)?;
        Ok(crate::planning::list(&v.conn)?)
    })
}

#[tauri::command]
pub fn recurring_upsert(state: State<AppState>, id: Option<String>, input: crate::planning::RecurringInput) -> R<String> {
    if input.name.trim().is_empty() || input.amount_cents == 0 || crate::import::parse_date(&input.next_date).is_none() {
        return Err(err("Revisa el nombre, el importe y la fecha"));
    }
    with_vault_save(&state, |v| Ok(crate::planning::upsert(&v.conn, id.as_deref(), &input)?))
}

#[tauri::command]
pub fn recurring_set_active(state: State<AppState>, id: String, active: bool) -> R<()> {
    with_vault_save(&state, |v| {
        v.conn.execute("UPDATE recurring SET active=? WHERE id=?", rusqlite::params![active as i64, id])?;
        Ok(())
    })
}

#[tauri::command]
pub fn recurring_delete(state: State<AppState>, id: String) -> R<()> {
    with_vault_save(&state, |v| {
        // Los detectados se marcan inactivos para que no reaparezcan
        v.conn.execute("UPDATE recurring SET active=0 WHERE id=? AND source='detected'", [&id])?;
        v.conn.execute("DELETE FROM recurring WHERE id=? AND source='manual'", [&id])?;
        Ok(())
    })
}

#[tauri::command]
pub fn goals_list(state: State<AppState>) -> R<Vec<crate::planning::Goal>> {
    with_vault(&state, |v| Ok(crate::planning::goals(&v.conn)?))
}

#[derive(serde::Deserialize)]
pub struct GoalInput {
    name: String,
    target_cents: i64,
    saved_cents: Option<i64>,
    target_date: Option<String>,
    color: Option<String>,
}

#[tauri::command]
pub fn goal_upsert(state: State<AppState>, id: Option<String>, input: GoalInput) -> R<()> {
    if input.name.trim().is_empty() || input.target_cents <= 0 {
        return Err(err("Indica un nombre y un objetivo mayor que cero"));
    }
    with_vault_save(&state, |v| {
        let date = input.target_date.filter(|d| !d.is_empty());
        match id {
            Some(id) => v.conn.execute(
                "UPDATE goals SET name=?,target_cents=?,saved_cents=COALESCE(?,saved_cents),target_date=?,color=COALESCE(?,color) WHERE id=?",
                rusqlite::params![input.name.trim(), input.target_cents, input.saved_cents, date, input.color, id],
            )?,
            None => v.conn.execute(
                "INSERT INTO goals(id,name,target_cents,saved_cents,target_date,color) VALUES(?,?,?,?,?,COALESCE(?,'#E5383B'))",
                rusqlite::params![crate::ledger::uid(), input.name.trim(), input.target_cents, input.saved_cents.unwrap_or(0), date, input.color],
            )?,
        };
        Ok(())
    })
}

#[tauri::command]
pub fn goal_contribute(state: State<AppState>, id: String, amount_cents: i64) -> R<()> {
    with_vault_save(&state, |v| {
        v.conn.execute("UPDATE goals SET saved_cents=max(0, saved_cents+?) WHERE id=?", rusqlite::params![amount_cents, id])?;
        Ok(())
    })
}

#[tauri::command]
pub fn goal_delete(state: State<AppState>, id: String) -> R<()> {
    with_vault_save(&state, |v| {
        v.conn.execute("DELETE FROM goals WHERE id=?", [id])?;
        Ok(())
    })
}

#[tauri::command]
pub fn forecast(state: State<AppState>, days: i64, include_variable: bool) -> R<crate::planning::Forecast> {
    with_vault_save(&state, |v| {
        crate::planning::detect(&v.conn)?;
        Ok(crate::planning::forecast(&v.conn, days, include_variable)?)
    })
}

// ---------------------------------------------------------------- autónomo (resumen del negocio)

#[tauri::command]
pub fn business_year(state: State<AppState>, year: i32) -> R<crate::analytics::BusinessYear> {
    with_vault(&state, |v| Ok(crate::analytics::business_year(&v.conn, year)?))
}

// ---------------------------------------------------------------- seguridad y datos

#[tauri::command]
pub async fn vault_change_password(state: State<'_, AppState>, current: String, new_password: String) -> R<()> {
    let mut g = state.vault.lock().unwrap();
    let v = g.as_mut().ok_or(VaultError::Locked)?;
    if !v.verify(&current) {
        return Err(VaultError::BadPassword);
    }
    v.rekey(&new_password)
}

/// Copia de seguridad: el mismo fichero cifrado, guardado donde elija el usuario.
#[tauri::command]
pub fn backup_export(state: State<AppState>, dest: String) -> R<()> {
    with_vault(&state, |v| {
        v.save()?;
        std::fs::copy(v.path(), &dest)?;
        Ok(())
    })
}

/// Restaura una copia: debe abrirse con la contraseña indicada; sustituye el almacén actual.
#[tauri::command]
pub async fn backup_restore(state: State<'_, AppState>, src: String, password: String) -> R<()> {
    let path = state.path.clone();
    let src_path = std::path::PathBuf::from(&src);
    let restored = tauri::async_runtime::spawn_blocking(move || -> R<Vault> {
        Vault::open(&src_path, &password)?;
        if path.exists() {
            std::fs::copy(&path, path.with_extension("cwf.bak"))?;
        }
        std::fs::copy(&src_path, &path)?;
        Vault::open(&path, &password)
    })
    .await
    .map_err(|_| err("Error interno"))??;
    *state.vault.lock().unwrap() = Some(restored);
    Ok(())
}

#[tauri::command]
pub fn export_csv(state: State<AppState>, dest: String, filter: crate::ledger::TxFilter) -> R<usize> {
    with_vault(&state, |v| {
        let f = crate::ledger::TxFilter { limit: Some(2000), ..filter };
        let mut all = Vec::new();
        let mut offset = 0;
        loop {
            let page = crate::ledger::list(&v.conn, &crate::ledger::TxFilter { offset: Some(offset), ..f.clone() })?;
            let n = page.items.len();
            all.extend(page.items);
            if n < 2000 { break }
            offset += 2000;
        }
        let mut w = csv::WriterBuilder::new().delimiter(b';').from_path(&dest).map_err(|e| err(e.to_string()))?;
        w.write_record(["Fecha", "Cuenta", "Banco", "Concepto", "Categoría", "Importe", "Notas", "Autónomo"]).map_err(|e| err(e.to_string()))?;
        for t in &all {
            w.write_record([
                t.date.clone(), t.account_name.clone(), t.institution.clone(), t.description.clone(),
                t.category_name.clone().unwrap_or_default(), format!("{:.2}", t.amount_cents as f64 / 100.0).replace('.', ","),
                t.notes.clone().unwrap_or_default(), if t.is_business { "Sí".into() } else { String::new() },
            ]).map_err(|e| err(e.to_string()))?;
        }
        w.flush()?;
        Ok(all.len())
    })
}

#[derive(serde::Deserialize)]
pub struct CategoryInput {
    name: String,
    parent_id: Option<String>,
    kind: String,
    color: String,
    icon: String,
}

#[tauri::command]
pub fn category_upsert(state: State<AppState>, id: Option<String>, input: CategoryInput) -> R<String> {
    if input.name.trim().is_empty() {
        return Err(err("El nombre es obligatorio"));
    }
    with_vault_save(&state, |v| {
        match id {
            Some(id) => {
                v.conn.execute(
                    "UPDATE categories SET name=?,color=?,icon=? WHERE id=?",
                    rusqlite::params![input.name.trim(), input.color, input.icon, id],
                )?;
                Ok(id)
            }
            None => {
                let id = crate::ledger::uid();
                v.conn.execute(
                    "INSERT INTO categories(id,parent_id,name,kind,color,icon,sort) VALUES(?,?,?,?,?,?,(SELECT COALESCE(MAX(sort),0)+1 FROM categories))",
                    rusqlite::params![id, input.parent_id, input.name.trim(), input.kind, input.color, input.icon],
                )?;
                Ok(id)
            }
        }
    })
}

#[tauri::command]
pub fn category_delete(state: State<AppState>, id: String) -> R<()> {
    with_vault_save(&state, |v| {
        v.conn.execute("UPDATE categories SET parent_id=NULL WHERE parent_id=?", [&id])?;
        v.conn.execute("DELETE FROM categories WHERE id=?", [&id])?;
        Ok(())
    })
}

// ---------------------------------------------------------------- conexión bancaria (Enable Banking)

fn bank_cfg(state: &State<AppState>) -> R<crate::bank::BankConfig> {
    with_vault(state, |v| crate::bank::load_config(&v.conn).ok_or_else(|| err(crate::bank::BankError::NotConfigured.to_string())))
}
fn berr(e: crate::bank::BankError) -> VaultError {
    err(e.to_string())
}

#[derive(Serialize)]
pub struct BankConfigView {
    app_id: String,
    has_key: bool,
    redirect_url: String,
}

#[tauri::command]
pub fn bank_config_get(state: State<AppState>) -> R<Option<BankConfigView>> {
    with_vault(&state, |v| {
        Ok(crate::bank::load_config(&v.conn).map(|c| BankConfigView { app_id: c.app_id, has_key: !c.private_key.is_empty(), redirect_url: c.redirect_url }))
    })
}

#[tauri::command]
pub fn bank_config_set(state: State<AppState>, app_id: String, key_path: Option<String>, redirect_url: String) -> R<()> {
    with_vault_save(&state, |v| {
        let mut cfg = crate::bank::load_config(&v.conn).unwrap_or_default();
        cfg.app_id = app_id.trim().to_string();
        cfg.redirect_url = redirect_url.trim().to_string();
        if let Some(p) = key_path {
            let pem = std::fs::read_to_string(&p).map_err(|_| err("No se pudo leer el fichero de la clave"))?;
            if !pem.contains("PRIVATE KEY") {
                return Err(err("El fichero no contiene una clave privada PEM"));
            }
            jsonwebtoken::EncodingKey::from_rsa_pem(pem.as_bytes()).map_err(|_| err("La clave privada no es RSA válida"))?;
            cfg.private_key = pem;
        }
        v.conn.execute(
            "INSERT INTO settings(key,value) VALUES('bank_config',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [serde_json::to_string(&cfg).unwrap_or_default()],
        )?;
        Ok(())
    })
}

#[tauri::command]
pub async fn bank_aspsps(state: State<'_, AppState>, country: String) -> R<Vec<crate::bank::Aspsp>> {
    let cfg = bank_cfg(&state)?;
    tauri::async_runtime::spawn_blocking(move || crate::bank::aspsps(&cfg, &country)).await.map_err(|_| err("Error interno"))?.map_err(berr)
}

#[derive(Serialize)]
pub struct AuthStart {
    url: String,
    state: String,
    auto_capture: bool,
    redirect_url: String,
}

#[tauri::command]
pub async fn bank_connect_start(state: State<'_, AppState>, name: String, country: String, days: i64) -> R<AuthStart> {
    let mut cfg = bank_cfg(&state)?;
    // Usa una URL de redirección realmente registrada en Enable Banking
    let allowed = tauri::async_runtime::spawn_blocking({
        let c = cfg.clone();
        move || crate::bank::allowed_redirects(&c)
    })
    .await
    .map_err(|_| err("Error interno"))?
    .unwrap_or_default();
    if let Some(r) = crate::bank::pick_redirect(&cfg.redirect_url, &allowed) {
        if r != cfg.redirect_url {
            cfg.redirect_url = r;
            let saved = cfg.clone();
            with_vault_save(&state, |v| {
                v.conn.execute(
                    "INSERT INTO settings(key,value) VALUES('bank_config',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                    [serde_json::to_string(&saved).unwrap_or_default()],
                )?;
                Ok(())
            })?;
        }
    }
    let st = crate::ledger::uid();
    let s2 = st.clone();
    let cfg2 = cfg.clone();
    let url = tauri::async_runtime::spawn_blocking(move || crate::bank::start_auth(&cfg2, &name, &country, days, &s2))
        .await
        .map_err(|_| err("Error interno"))?
        .map_err(|e| {
            let m = e.to_string();
            if m.contains("Redirect") && !allowed.is_empty() {
                err(format!("{m}. URLs registradas en Enable Banking: {}. La app usa: {}", allowed.join(", "), cfg.redirect_url))
            } else {
                berr(e)
            }
        })?;
    *state.bank_code.lock().unwrap() = None;
    let auto = crate::bank::listen_for_code(&cfg.redirect_url, state.bank_code.clone());
    Ok(AuthStart { url, state: st, auto_capture: auto, redirect_url: cfg.redirect_url })
}

#[tauri::command]
pub fn bank_connect_poll(state: State<AppState>) -> Option<String> {
    state.bank_code.lock().unwrap().clone()
}

#[derive(Serialize)]
pub struct ConnectResult {
    accounts: Vec<String>,
    imported: usize,
}

/// Crea la sesión con el código, vincula las cuentas y hace la primera sincronización.
#[tauri::command]
pub async fn bank_connect_finish(state: State<'_, AppState>, code_or_url: String, institution: String) -> R<ConnectResult> {
    let code = crate::bank::extract_code(&code_or_url).ok_or_else(|| err("No se encuentra el código de autorización en lo pegado"))?;
    let cfg = bank_cfg(&state)?;
    let session = tauri::async_runtime::spawn_blocking({
        let cfg = cfg.clone();
        move || crate::bank::create_session(&cfg, &code)
    })
    .await
    .map_err(|_| err("Error interno"))?
    .map_err(berr)?;
    if session.accounts.is_empty() {
        return Err(err("El banco no ha devuelto ninguna cuenta"));
    }
    let inst = if institution.trim().is_empty() { session.aspsp.clone() } else { institution };
    let mut names = Vec::new();
    with_vault_save(&state, |v| {
        for a in &session.accounts {
            let masked = a.iban.as_ref().map(|i| format!("···· {}", &i[i.len().saturating_sub(4)..]));
            let existing: Option<String> = v
                .conn
                .query_row("SELECT id FROM accounts WHERE provider='enablebanking' AND provider_ref=?", [&a.uid], |r| r.get(0))
                .ok();
            match existing {
                Some(id) => {
                    v.conn.execute("UPDATE accounts SET consent_expires_at=?, archived=0 WHERE id=?", rusqlite::params![session.valid_until, id])?;
                }
                None => {
                    v.conn.execute(
                        "INSERT INTO accounts(id,name,institution,currency,iban_masked,provider,provider_ref,consent_expires_at) VALUES(?,?,?,?,?,'enablebanking',?,?)",
                        rusqlite::params![crate::ledger::uid(), a.name, inst, a.currency, masked, a.uid, session.valid_until],
                    )?;
                }
            }
            names.push(format!("{inst} · {}", a.name));
        }
        Ok(())
    })?;
    let _ = cfg;
    Ok(ConnectResult { accounts: names, imported: 0 })
}

#[derive(Serialize)]
pub struct SyncResult {
    account: String,
    inserted: usize,
    error: Option<String>,
}

type LinkedRow = (String, String, String, Option<String>);

fn linked_accounts(state: &State<'_, AppState>, only: Option<&str>) -> R<Vec<LinkedRow>> {
    with_vault(state, |v| {
        let mut st = v.conn.prepare(
            "SELECT a.id, a.provider_ref, a.institution||' · '||a.name, (SELECT MAX(booking_date) FROM transactions WHERE account_id=a.id)
             FROM accounts a WHERE a.provider='enablebanking' AND a.archived=0 AND (?1 IS NULL OR a.id=?1)",
        )?;
        let rows = st.query_map([only], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    })
}

async fn sync_one(state: &State<'_, AppState>, cfg: &crate::bank::BankConfig, row: LinkedRow) -> R<SyncResult> {
    let (id, uid, label, last) = row;
    let from = last
        .and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok())
        .map(|d| d - chrono::Duration::days(7))
        .unwrap_or_else(|| chrono::Local::now().date_naive() - chrono::Duration::days(365))
        .to_string();
    let cfg2 = cfg.clone();
    let fetched = tauri::async_runtime::spawn_blocking(move || -> Result<(Vec<crate::import::ParsedTx>, Option<i64>), crate::bank::BankError> {
        let bal = crate::bank::balance(&cfg2, &uid)?;
        let txs = crate::bank::transactions(&cfg2, &uid, &from)?;
        Ok((txs, bal))
    })
    .await
    .map_err(|_| err("Error interno"))?;
    Ok(match fetched {
        Ok((txs, bal)) => {
            let r = with_vault_save(state, |v| {
                let r = crate::ledger::insert_parsed(&v.conn, &id, &txs, bal)?;
                v.conn.execute("UPDATE accounts SET updated_at=datetime('now') WHERE id=?", [&id])?;
                Ok(r)
            })?;
            SyncResult { account: label, inserted: r.inserted, error: None }
        }
        Err(e) => SyncResult { account: label, inserted: 0, error: Some(e.to_string()) },
    })
}

async fn sync_all(state: &State<'_, AppState>, cfg: &crate::bank::BankConfig) -> R<Vec<SyncResult>> {
    let mut out = Vec::new();
    for row in linked_accounts(state, None)? {
        out.push(sync_one(state, cfg, row).await?);
    }
    Ok(out)
}

#[derive(Serialize)]
pub struct LinkedView {
    id: String,
    label: String,
}

#[tauri::command]
pub fn bank_linked(state: State<AppState>) -> R<Vec<LinkedView>> {
    Ok(linked_accounts(&state, None)?.into_iter().map(|r| LinkedView { id: r.0, label: r.2 }).collect())
}

#[tauri::command]
pub async fn bank_sync_account(state: State<'_, AppState>, id: String) -> R<SyncResult> {
    let cfg = bank_cfg(&state)?;
    let row = linked_accounts(&state, Some(&id))?.into_iter().next().ok_or_else(|| err("Cuenta no vinculada"))?;
    sync_one(&state, &cfg, row).await
}

#[tauri::command]
pub async fn bank_sync(state: State<'_, AppState>) -> R<Vec<SyncResult>> {
    let cfg = bank_cfg(&state)?;
    sync_all(&state, &cfg).await
}

#[tauri::command]
pub fn uncategorized_groups(state: State<AppState>) -> R<Vec<crate::ledger::UncatGroup>> {
    with_vault(&state, |v| Ok(crate::ledger::uncategorized_groups(&v.conn)?))
}

#[tauri::command]
pub fn categorize_group(state: State<AppState>, key: String, income: bool, category_id: String) -> R<usize> {
    with_vault_save(&state, |v| Ok(crate::ledger::categorize_group(&v.conn, &key, income, &category_id)?))
}

/// Importa reglas desde un CSV "palabras;categoría" (categoría por id o por nombre). Cabecera opcional.
#[tauri::command]
pub fn rules_import(state: State<AppState>, path: String) -> R<(usize, usize, Vec<String>)> {
    let text = std::fs::read(&path).map_err(|_| err("No se pudo leer el fichero"))?;
    let text = match String::from_utf8(text.clone()) {
        Ok(s) => s,
        Err(_) => encoding_rs::WINDOWS_1252.decode(&text).0.into_owned(),
    };
    with_vault_save(&state, |v| {
        let cats: Vec<(String, String)> = {
            let mut st = v.conn.prepare("SELECT id, name FROM categories")?;
            let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
            rows.collect::<Result<_, _>>()?
        };
        let norm = |s: &str| crate::import::norm(s);
        let (mut added, mut unknown) = (0usize, Vec::new());
        for line in text.lines() {
            let line = line.trim().trim_start_matches('\u{feff}');
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let sep = if line.contains(';') { ';' } else if line.contains('\t') { '\t' } else { ',' };
            let parts: Vec<&str> = line.split(sep).map(|p| p.trim().trim_matches('"')).collect();
            if parts.len() < 2 {
                continue;
            }
            let (words, cat) = (parts[0], parts[1]);
            // 3ª columna opcional: "si"/"autonomo" marca los movimientos como de autónomo
            let business = parts.get(2).map(|b| matches!(norm(b).as_str(), "si" | "autonomo" | "1" | "true" | "x"));
            if norm(words) == "palabras" || norm(words) == "texto" {
                continue;
            }
            let Some(cid) = cats.iter().find(|(id, name)| id == cat || norm(name) == norm(cat)).map(|c| c.0.clone()) else {
                unknown.push(format!("{words} → {cat}"));
                continue;
            };
            let key = crate::ledger::rule_norm(words);
            if key.len() < 2 {
                continue;
            }
            v.conn.execute("DELETE FROM rules WHERE learned=1 AND match_value=? AND set_category_id=?", rusqlite::params![key, cid])?;
            v.conn.execute(
                "INSERT INTO rules(id,name,priority,match_field,match_op,match_value,set_category_id,set_business,learned) VALUES(?,?,8,'description','words',?,?,?,1)",
                rusqlite::params![crate::ledger::uid(), key, key, cid, business.filter(|b| *b).map(|_| 1i64)],
            )?;
            added += 1;
        }
        let changed = crate::ledger::recategorize(&v.conn)?;
        Ok((added, changed, unknown))
    })
}
