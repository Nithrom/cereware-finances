//! Sincronización cifrada de extremo a extremo entre dispositivos.
//!
//! - Cada cambio local se anota en `sync_log` mediante triggers (solo con `tracking=1`).
//! - Al sincronizar se envían paquetes cifrados (AES-256-GCM con la clave de sincronización,
//!   que nunca sale de los dispositivos) a un servidor que solo almacena blobs opacos.
//! - Conflictos: gana la última escritura por fila (marca de tiempo + id de dispositivo).
//! - Movimientos duplicados (mismo `import_hash` con distinto id): se conserva el id menor.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use rand::RngCore;
use rusqlite::{params, types::Value as SqlValue, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::time::Duration;

/// Tablas sincronizadas y su clave primaria.
pub const TABLES: &[(&str, &str)] = &[
    ("accounts", "id"),
    ("categories", "id"),
    ("transactions", "id"),
    ("transaction_splits", "id"),
    ("rules", "id"),
    ("budgets", "id"),
    ("recurring", "id"),
    ("goals", "id"),
    ("settings", "key"),
];

/// Claves de `settings` que son propias de cada dispositivo.
const LOCAL_SETTINGS: &str = "'seed_version'";

const HLC: &str = "printf('%013d', CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER)) || '|' || (SELECT value FROM sync_state WHERE key='device')";

pub const MIGRATION: &str = r#"
    CREATE TABLE sync_state (key TEXT PRIMARY KEY, value TEXT NOT NULL);
    INSERT INTO sync_state(key,value) VALUES ('tracking','0'), ('applying','0'), ('device', lower(hex(randomblob(8))));
    CREATE TABLE sync_log (seq INTEGER PRIMARY KEY AUTOINCREMENT, tbl TEXT NOT NULL, row_id TEXT NOT NULL, op TEXT NOT NULL, hlc TEXT NOT NULL);
    CREATE TABLE row_hlc (tbl TEXT NOT NULL, row_id TEXT NOT NULL, hlc TEXT NOT NULL, PRIMARY KEY(tbl,row_id));
"#;

/// Crea (si no existen) los triggers que anotan los cambios locales.
pub fn install_triggers(conn: &Connection) -> rusqlite::Result<()> {
    let active = "(SELECT value FROM sync_state WHERE key='tracking')='1' AND (SELECT value FROM sync_state WHERE key='applying')='0'";
    let mut sql = String::new();
    for (t, pk) in TABLES {
        let extra = |r: &str| if *t == "settings" { format!(" AND {r}.key NOT IN ({LOCAL_SETTINGS})") } else { String::new() };
        for (ev, short, rec, op) in [("INSERT", "ins", "NEW", "upsert"), ("UPDATE", "upd", "NEW", "upsert"), ("DELETE", "del", "OLD", "delete")] {
            sql.push_str(&format!(
                "CREATE TRIGGER IF NOT EXISTS trk_{t}_{short} AFTER {ev} ON {t} WHEN {active}{x} BEGIN
                   INSERT INTO sync_log(tbl,row_id,op,hlc) VALUES('{t}', {rec}.{pk}, '{op}', {HLC});
                   INSERT OR REPLACE INTO row_hlc(tbl,row_id,hlc) VALUES('{t}', {rec}.{pk}, {HLC});
                 END;\n",
                x = extra(rec)
            ));
        }
    }
    conn.execute_batch(&sql)
}

fn state(conn: &Connection, k: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT value FROM sync_state WHERE key=?", [k], |r| r.get(0)).optional()
}
fn set_state(conn: &Connection, k: &str, v: &str) -> rusqlite::Result<()> {
    conn.execute("INSERT INTO sync_state(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [k, v])?;
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("La sincronización no está configurada")]
    NotConfigured,
    #[error("Código de emparejamiento no válido")]
    BadCode,
    #[error("No se puede descifrar: la clave de sincronización no coincide")]
    BadKey,
    #[error("Servidor de sincronización: {0}")]
    Server(String),
    #[error("Base de datos: {0}")]
    Db(#[from] rusqlite::Error),
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    pub server: String,
    pub space: String,
    pub token: String,
    /// Clave AES-256 en hexadecimal
    pub key: String,
}

pub fn config(conn: &Connection) -> rusqlite::Result<Option<Config>> {
    Ok(state(conn, "config")?.and_then(|s| serde_json::from_str(&s).ok()))
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
fn unhex(s: &str) -> Option<Vec<u8>> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect()
}
fn b64url(data: &[u8]) -> String {
    const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..(c.len() + 1) {
            out.push(A[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    out
}
fn unb64url(s: &str) -> Option<Vec<u8>> {
    let val = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            _ => return None,
        } as u32)
    };
    let bytes: Vec<u8> = s.trim().trim_end_matches('=').bytes().collect();
    let mut out = Vec::new();
    for c in bytes.chunks(4) {
        let mut n = 0u32;
        for (i, b) in c.iter().enumerate() {
            n |= val(*b)? << (18 - 6 * i);
        }
        for i in 0..c.len().saturating_sub(1) {
            out.push((n >> (16 - 8 * i) & 255) as u8);
        }
    }
    Some(out)
}

fn rand_hex(n: usize) -> String {
    let mut b = vec![0u8; n];
    rand::thread_rng().fill_bytes(&mut b);
    hex(&b)
}

/// Código de emparejamiento (se muestra como QR): "CWF1." + base64url(json)
pub fn pairing_code(c: &Config) -> String {
    format!("CWF1.{}", b64url(json!({"u": c.server, "s": c.space, "t": c.token, "k": c.key}).to_string().as_bytes()))
}

pub fn parse_code(code: &str) -> Result<Config, SyncError> {
    let body = code.trim().strip_prefix("CWF1.").ok_or(SyncError::BadCode)?;
    let v: Value = serde_json::from_slice(&unb64url(body).ok_or(SyncError::BadCode)?).map_err(|_| SyncError::BadCode)?;
    let g = |k: &str| v[k].as_str().map(String::from).ok_or(SyncError::BadCode);
    let c = Config { server: g("u")?, space: g("s")?, token: g("t")?, key: g("k")? };
    if unhex(&c.key).map(|k| k.len()) != Some(32) {
        return Err(SyncError::BadCode);
    }
    Ok(c)
}

fn encrypt(key_hex: &str, plain: &[u8]) -> Result<String, SyncError> {
    let key = unhex(key_hex).ok_or(SyncError::BadKey)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| SyncError::BadKey)?;
    let mut nonce = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce);
    let ct = cipher.encrypt(Nonce::from_slice(&nonce), plain).map_err(|_| SyncError::BadKey)?;
    let mut out = nonce.to_vec();
    out.extend(ct);
    Ok(b64url(&out))
}

fn decrypt(key_hex: &str, blob: &str) -> Result<Vec<u8>, SyncError> {
    let key = unhex(key_hex).ok_or(SyncError::BadKey)?;
    let raw = unb64url(blob).ok_or(SyncError::BadKey)?;
    if raw.len() < 13 {
        return Err(SyncError::BadKey);
    }
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| SyncError::BadKey)?;
    cipher.decrypt(Nonce::from_slice(&raw[..12]), &raw[12..]).map_err(|_| SyncError::BadKey)
}

// ---------------------------------------------------------------- operaciones

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Op {
    pub t: String,
    pub id: String,
    pub op: String,
    pub hlc: String,
    #[serde(default)]
    pub data: Option<Map<String, Value>>,
}

fn pk_of(t: &str) -> Option<&'static str> {
    TABLES.iter().find(|x| x.0 == t).map(|x| x.1)
}

fn row_json(conn: &Connection, t: &str, id: &str) -> rusqlite::Result<Option<Map<String, Value>>> {
    let pk = pk_of(t).unwrap_or("id");
    let mut st = conn.prepare(&format!("SELECT * FROM {t} WHERE {pk}=?"))?;
    let cols: Vec<String> = st.column_names().iter().map(|s| s.to_string()).collect();
    st.query_row([id], |r| {
        let mut m = Map::new();
        for (i, c) in cols.iter().enumerate() {
            let v = match r.get::<_, SqlValue>(i)? {
                SqlValue::Null => Value::Null,
                SqlValue::Integer(n) => json!(n),
                SqlValue::Real(f) => json!(f),
                SqlValue::Text(s) => json!(s),
                SqlValue::Blob(b) => json!(hex(&b)),
            };
            m.insert(c.clone(), v);
        }
        Ok(m)
    })
    .optional()
}

/// Cambios locales pendientes (última versión de cada fila).
pub fn pending(conn: &Connection) -> rusqlite::Result<(Vec<Op>, i64)> {
    let max: i64 = conn.query_row("SELECT COALESCE(MAX(seq),0) FROM sync_log", [], |r| r.get(0))?;
    let mut st = conn.prepare(
        "SELECT tbl, row_id, op, hlc FROM sync_log WHERE seq IN (SELECT MAX(seq) FROM sync_log GROUP BY tbl,row_id) ORDER BY seq",
    )?;
    let rows: Vec<(String, String, String, String)> = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?.collect::<Result<_, _>>()?;
    let mut ops = Vec::new();
    for (t, id, op, hlc) in rows {
        let data = if op == "upsert" { row_json(conn, &t, &id)? } else { None };
        // Si la fila ya no existe, se envía como borrado
        let op = if op == "upsert" && data.is_none() { "delete".to_string() } else { op };
        ops.push(Op { t, id, op, hlc, data });
    }
    Ok((ops, max))
}

pub fn pending_count(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row("SELECT COUNT(DISTINCT tbl||'/'||row_id) FROM sync_log", [], |r| r.get(0))
}

/// Marca todas las filas existentes como cambiadas (primer dispositivo al activar la sincronización).
pub fn snapshot(conn: &Connection) -> rusqlite::Result<()> {
    for (t, pk) in TABLES {
        let filter = if *t == "settings" { format!(" WHERE key NOT IN ({LOCAL_SETTINGS})") } else { String::new() };
        conn.execute_batch(&format!(
            "INSERT INTO sync_log(tbl,row_id,op,hlc) SELECT '{t}', {pk}, 'upsert', {HLC} FROM {t}{filter};
             INSERT OR REPLACE INTO row_hlc(tbl,row_id,hlc) SELECT '{t}', {pk}, {HLC} FROM {t}{filter};"
        ))?;
    }
    Ok(())
}

fn to_sql(v: &Value) -> SqlValue {
    match v {
        Value::Null => SqlValue::Null,
        Value::Bool(b) => SqlValue::Integer(*b as i64),
        Value::Number(n) => n.as_i64().map(SqlValue::Integer).unwrap_or_else(|| SqlValue::Real(n.as_f64().unwrap_or(0.0))),
        Value::String(s) => SqlValue::Text(s.clone()),
        other => SqlValue::Text(other.to_string()),
    }
}

fn upsert(conn: &Connection, t: &str, data: &Map<String, Value>) -> rusqlite::Result<()> {
    let pk = pk_of(t).unwrap_or("id");
    let cols: Vec<String> = {
        let mut st = conn.prepare(&format!("PRAGMA table_info({t})"))?;
        let names: Vec<String> = st.query_map([], |r| r.get::<_, String>(1))?.collect::<Result<_, _>>()?;
        names.into_iter().filter(|c| data.contains_key(c)).collect()
    };
    let placeholders = vec!["?"; cols.len()].join(",");
    let updates: Vec<String> = cols.iter().filter(|c| *c != pk).map(|c| format!("{c}=excluded.{c}")).collect();
    let sql = format!(
        "INSERT INTO {t}({}) VALUES({placeholders}) ON CONFLICT({pk}) DO {}",
        cols.join(","),
        if updates.is_empty() { "NOTHING".into() } else { format!("UPDATE SET {}", updates.join(",")) }
    );
    let vals: Vec<SqlValue> = cols.iter().map(|c| to_sql(&data[c])).collect();
    conn.execute(&sql, rusqlite::params_from_iter(vals))?;
    Ok(())
}

/// Registra un cambio generado al resolver un conflicto (para que el resto de dispositivos converja).
fn log_local(conn: &Connection, t: &str, id: &str, op: &str) -> rusqlite::Result<()> {
    conn.execute(&format!("INSERT INTO sync_log(tbl,row_id,op,hlc) VALUES(?1,?2,?3,{HLC})"), params![t, id, op])?;
    conn.execute(&format!("INSERT OR REPLACE INTO row_hlc(tbl,row_id,hlc) VALUES(?1,?2,{HLC})"), params![t, id])?;
    Ok(())
}

/// Aplica operaciones remotas con "gana la última escritura".
pub fn apply(conn: &Connection, ops: &[Op]) -> rusqlite::Result<usize> {
    conn.execute_batch("PRAGMA foreign_keys = OFF;")?;
    set_state(conn, "applying", "1")?;
    let tx = conn.unchecked_transaction()?;
    let mut n = 0;
    let res = (|| -> rusqlite::Result<()> {
        for o in ops {
            let Some(pk) = pk_of(&o.t) else { continue };
            let local: Option<String> = tx.query_row("SELECT hlc FROM row_hlc WHERE tbl=? AND row_id=?", [&o.t, &o.id], |r| r.get(0)).optional()?;
            if local.as_ref().is_some_and(|l| l >= &o.hlc) {
                continue;
            }
            match (o.op.as_str(), &o.data) {
                ("delete", _) => {
                    tx.execute(&format!("DELETE FROM {} WHERE {pk}=?", o.t), [&o.id])?;
                }
                ("upsert", Some(d)) => {
                    // Duplicado del mismo movimiento bancario importado en dos dispositivos
                    if o.t == "transactions" {
                        if let Some(h) = d.get("import_hash").and_then(|h| h.as_str()) {
                            let other: Option<String> = tx
                                .query_row("SELECT id FROM transactions WHERE import_hash=? AND id<>?", [h, &o.id], |r| r.get(0))
                                .optional()?;
                            if let Some(other) = other {
                                if o.id < other {
                                    tx.execute("DELETE FROM transactions WHERE id=?", [&other])?;
                                    log_local(&tx, "transactions", &other, "delete")?;
                                } else {
                                    log_local(&tx, "transactions", &o.id, "delete")?;
                                    continue;
                                }
                            }
                        }
                    }
                    upsert(&tx, &o.t, d)?;
                }
                _ => continue,
            }
            tx.execute("INSERT OR REPLACE INTO row_hlc(tbl,row_id,hlc) VALUES(?,?,?)", params![o.t, o.id, o.hlc])?;
            n += 1;
        }
        Ok(())
    })();
    match res {
        Ok(()) => tx.commit()?,
        Err(e) => {
            drop(tx);
            set_state(conn, "applying", "0")?;
            conn.execute_batch("PRAGMA foreign_keys = ON;")?;
            return Err(e);
        }
    }
    set_state(conn, "applying", "0")?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    Ok(n)
}

// ---------------------------------------------------------------- red

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout(Duration::from_secs(60)).build()
}

fn url(c: &Config, action: &str) -> String {
    let base = c.server.trim_end_matches('/');
    let base = if base.ends_with(".php") { base.to_string() } else { format!("{base}/sync.php") };
    format!("{base}?a={action}")
}

fn err(e: ureq::Error) -> SyncError {
    match e {
        ureq::Error::Status(code, r) => SyncError::Server(format!("{code} · {}", r.into_string().unwrap_or_default().chars().take(200).collect::<String>())),
        e => SyncError::Server(e.to_string()),
    }
}

pub fn push_blob(c: &Config, device: &str, ops: &[Op]) -> Result<i64, SyncError> {
    let blob = encrypt(&c.key, &serde_json::to_vec(ops).unwrap_or_default())?;
    let r: Value = agent()
        .post(&url(c, "push"))
        .set("X-Space", &c.space)
        .set("X-Token", &c.token)
        .send_json(json!({ "device": device, "blob": blob }))
        .map_err(err)?
        .into_json()
        .map_err(|e| SyncError::Server(e.to_string()))?;
    r["seq"].as_i64().ok_or_else(|| SyncError::Server("respuesta inesperada".into()))
}

pub struct Pulled {
    pub ops: Vec<Op>,
    pub last: i64,
    pub more: bool,
}

pub fn pull(c: &Config, device: &str, since: i64) -> Result<Pulled, SyncError> {
    let r: Value = agent()
        .get(&format!("{}&since={since}", url(c, "pull")))
        .set("X-Space", &c.space)
        .set("X-Token", &c.token)
        .call()
        .map_err(err)?
        .into_json()
        .map_err(|e| SyncError::Server(e.to_string()))?;
    let mut ops = Vec::new();
    let mut last = since;
    for it in r["items"].as_array().into_iter().flatten() {
        last = last.max(it["seq"].as_i64().unwrap_or(last));
        if it["device"].as_str() == Some(device) {
            continue;
        }
        let plain = decrypt(&c.key, it["blob"].as_str().unwrap_or_default())?;
        let batch: Vec<Op> = serde_json::from_slice(&plain).map_err(|_| SyncError::BadKey)?;
        ops.extend(batch);
    }
    Ok(Pulled { ops, last, more: r["more"].as_bool().unwrap_or(false) })
}

// ---------------------------------------------------------------- estado

#[derive(Serialize)]
pub struct Status {
    pub enabled: bool,
    pub server: Option<String>,
    pub device: String,
    pub pending: i64,
    pub last_sync: Option<String>,
}

pub fn status(conn: &Connection) -> rusqlite::Result<Status> {
    let c = config(conn)?;
    Ok(Status {
        enabled: c.is_some(),
        server: c.map(|c| c.server),
        device: state(conn, "device")?.unwrap_or_default(),
        pending: pending_count(conn)?,
        last_sync: state(conn, "last_sync")?,
    })
}

pub fn device(conn: &Connection) -> rusqlite::Result<String> {
    Ok(state(conn, "device")?.unwrap_or_default())
}
pub fn cursor(conn: &Connection) -> rusqlite::Result<i64> {
    Ok(state(conn, "cursor")?.and_then(|s| s.parse().ok()).unwrap_or(0))
}
pub fn set_cursor(conn: &Connection, v: i64) -> rusqlite::Result<()> {
    set_state(conn, "cursor", &v.to_string())
}
pub fn clear_log(conn: &Connection, upto: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM sync_log WHERE seq<=?", [upto])?;
    Ok(())
}
pub fn mark_synced(conn: &Connection) -> rusqlite::Result<()> {
    set_state(conn, "last_sync", &chrono::Local::now().to_rfc3339())
}

/// Primer dispositivo: crea el espacio y la clave, y marca todo para subir.
pub fn enable_new(conn: &Connection, server: &str) -> rusqlite::Result<Config> {
    let c = Config { server: server.trim().to_string(), space: rand_hex(16), token: rand_hex(24), key: rand_hex(32) };
    set_state(conn, "config", &serde_json::to_string(&c).unwrap_or_default())?;
    set_state(conn, "cursor", "0")?;
    set_state(conn, "tracking", "1")?;
    snapshot(conn)?;
    Ok(c)
}

/// Dispositivo que se une: guarda la configuración; los datos locales se descartan en favor de los remotos.
pub fn join(conn: &Connection, c: &Config) -> rusqlite::Result<()> {
    set_state(conn, "config", &serde_json::to_string(c).unwrap_or_default())?;
    set_state(conn, "cursor", "0")?;
    set_state(conn, "tracking", "0")?;
    conn.execute("DELETE FROM sync_log", [])?;
    conn.execute("DELETE FROM row_hlc", [])?;
    Ok(())
}

pub fn start_tracking(conn: &Connection) -> rusqlite::Result<()> {
    set_state(conn, "tracking", "1")
}

pub fn disable(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM sync_state WHERE key IN ('config','cursor','last_sync')", [])?;
    set_state(conn, "tracking", "0")?;
    conn.execute("DELETE FROM sync_log", [])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        crate::schema::migrate(&c).unwrap();
        c
    }

    #[test]
    fn code_roundtrip_and_crypto() {
        let c = Config { server: "https://x.es/sync".into(), space: rand_hex(16), token: rand_hex(24), key: rand_hex(32) };
        let p = parse_code(&pairing_code(&c)).unwrap();
        assert_eq!(p.space, c.space);
        assert_eq!(p.key, c.key);
        let blob = encrypt(&c.key, b"hola").unwrap();
        assert_eq!(decrypt(&c.key, &blob).unwrap(), b"hola");
        assert!(decrypt(&rand_hex(32), &blob).is_err());
        assert!(parse_code("basura").is_err());
    }

    #[test]
    fn two_devices_converge() {
        let a = db();
        let b = db();
        a.execute("INSERT INTO accounts(id,name,institution) VALUES('acc','Nómina','Openbank')", []).unwrap();
        let cfg = enable_new(&a, "https://srv").unwrap();
        // A: cambios después de activar
        a.execute("INSERT INTO transactions(id,account_id,booking_date,amount_cents,description,import_hash) VALUES('t2','acc','2026-10-01',-500,'Pan','H1')", []).unwrap();
        let (ops_a, max_a) = pending(&a).unwrap();
        assert!(ops_a.iter().any(|o| o.t == "accounts"));
        assert!(ops_a.iter().any(|o| o.t == "categories"));
        assert!(!ops_a.iter().any(|o| o.t == "settings" && o.id == "seed_version"));
        clear_log(&a, max_a).unwrap();

        // B se une y aplica todo
        join(&b, &cfg).unwrap();
        apply(&b, &ops_a).unwrap();
        start_tracking(&b).unwrap();
        let n: i64 = b.query_row("SELECT COUNT(*) FROM transactions", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1);
        assert_eq!(pending_count(&b).unwrap(), 0, "aplicar no debe generar cambios locales");

        // B categoriza y además importa el mismo movimiento con otro id (H1) → se queda el id menor
        std::thread::sleep(std::time::Duration::from_millis(5));
        b.execute("UPDATE transactions SET category_id='supermercado' WHERE id='t2'", []).unwrap();
        b.execute("INSERT INTO transactions(id,account_id,booking_date,amount_cents,description,import_hash) VALUES('t9','acc','2026-10-02',-100,'Café','H2')", []).unwrap();
        let (ops_b, max_b) = pending(&b).unwrap();
        clear_log(&b, max_b).unwrap();
        apply(&a, &ops_b).unwrap();
        let cat: Option<String> = a.query_row("SELECT category_id FROM transactions WHERE id='t2'", [], |r| r.get(0)).unwrap();
        assert_eq!(cat.as_deref(), Some("supermercado"));
        let n: i64 = a.query_row("SELECT COUNT(*) FROM transactions", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 2);

        // Conflicto de duplicado: A tiene t2 (H1); llega 't1' con el mismo hash desde otro dispositivo
        let dup = Op { t: "transactions".into(), id: "t1".into(), op: "upsert".into(), hlc: "9999999999999|zz".into(),
            data: Some(serde_json::from_value(json!({"id":"t1","account_id":"acc","booking_date":"2026-10-01","amount_cents":-500,"description":"Pan","import_hash":"H1"})).unwrap()) };
        apply(&a, &[dup]).unwrap();
        let ids: Vec<String> = a.prepare("SELECT id FROM transactions WHERE import_hash='H1'").unwrap().query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect();
        assert_eq!(ids, vec!["t1".to_string()]);
        let (ops, _) = pending(&a).unwrap();
        assert!(ops.iter().any(|o| o.id == "t2" && o.op == "delete"), "el borrado del duplicado se propaga");

        // Borrado remoto
        let del = Op { t: "transactions".into(), id: "t9".into(), op: "delete".into(), hlc: "9999999999999|zz".into(), data: None };
        apply(&a, &[del]).unwrap();
        let n: i64 = a.query_row("SELECT COUNT(*) FROM transactions WHERE id='t9'", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0);
    }

    /// Prueba contra un servidor real: SYNC_URL=http://127.0.0.1:8787/sync.php cargo test -- --ignored
    #[test]
    #[ignore]
    fn end_to_end_with_server() {
        let url = std::env::var("SYNC_URL").expect("SYNC_URL");
        let (a, b) = (db(), db());
        a.execute("INSERT INTO accounts(id,name,institution,balance_cents) VALUES('acc','Nómina','Openbank',1000)", []).unwrap();
        let cfg = enable_new(&a, &url).unwrap();
        let (ops, max) = pending(&a).unwrap();
        push_blob(&cfg, &device(&a).unwrap(), &ops).unwrap();
        clear_log(&a, max).unwrap();
        let code = pairing_code(&cfg);
        let cb = parse_code(&code).unwrap();
        join(&b, &cb).unwrap();
        let p = pull(&cb, &device(&b).unwrap(), 0).unwrap();
        apply(&b, &p.ops).unwrap();
        set_cursor(&b, p.last).unwrap();
        start_tracking(&b).unwrap();
        let bal: i64 = b.query_row("SELECT balance_cents FROM accounts WHERE id='acc'", [], |r| r.get(0)).unwrap();
        assert_eq!(bal, 1000);
        std::thread::sleep(std::time::Duration::from_millis(5));
        b.execute("UPDATE accounts SET balance_cents=2500 WHERE id='acc'", []).unwrap();
        let (ops, max) = pending(&b).unwrap();
        push_blob(&cb, &device(&b).unwrap(), &ops).unwrap();
        clear_log(&b, max).unwrap();
        let p = pull(&cfg, &device(&a).unwrap(), cursor(&a).unwrap()).unwrap();
        apply(&a, &p.ops).unwrap();
        let bal: i64 = a.query_row("SELECT balance_cents FROM accounts WHERE id='acc'", [], |r| r.get(0)).unwrap();
        assert_eq!(bal, 2500);
    }
}
