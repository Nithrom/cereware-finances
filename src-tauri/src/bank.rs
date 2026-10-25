//! Conexión PSD2 de solo lectura vía Enable Banking (https://enablebanking.com).
//! Fase 1: cada usuario usa su propia aplicación de Enable Banking (ID + clave privada RSA).
//! Las credenciales se guardan dentro del almacén cifrado.

use crate::import::ParsedTx;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const API: &str = "https://api.enablebanking.com";

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct BankConfig {
    pub app_id: String,
    pub private_key: String,
    pub redirect_url: String,
}

pub fn load_config(conn: &rusqlite::Connection) -> Option<BankConfig> {
    conn.query_row("SELECT value FROM settings WHERE key='bank_config'", [], |r| r.get::<_, String>(0))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
}

#[derive(Debug, thiserror::Error)]
pub enum BankError {
    #[error("Configura primero tus credenciales de Enable Banking en Ajustes")]
    NotConfigured,
    #[error("La clave privada no es válida (debe ser RSA en formato PEM)")]
    BadKey,
    #[error("El banco ha rechazado la petición: {0}")]
    Api(String),
    #[error("Sin conexión con Enable Banking: {0}")]
    Net(String),
}

fn jwt(cfg: &BankConfig) -> Result<String, BankError> {
    if cfg.app_id.is_empty() || cfg.private_key.is_empty() {
        return Err(BankError::NotConfigured);
    }
    let now = chrono::Utc::now().timestamp();
    let mut h = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
    h.kid = Some(cfg.app_id.clone());
    h.typ = Some("JWT".into());
    let claims = json!({ "iss": "enablebanking.com", "aud": "api.enablebanking.com", "iat": now, "exp": now + 3600 });
    let key = jsonwebtoken::EncodingKey::from_rsa_pem(cfg.private_key.as_bytes()).map_err(|_| BankError::BadKey)?;
    jsonwebtoken::encode(&h, &claims, &key).map_err(|_| BankError::BadKey)
}

fn call(cfg: &BankConfig, method: &str, path: &str, body: Option<Value>) -> Result<Value, BankError> {
    let token = jwt(cfg)?;
    let req = ureq::request(method, &format!("{API}{path}")).set("Authorization", &format!("Bearer {token}")).timeout(Duration::from_secs(60));
    let res = match body {
        Some(b) => req.send_json(b),
        None => req.call(),
    };
    match res {
        Ok(r) => r.into_json::<Value>().map_err(|e| BankError::Net(e.to_string())),
        Err(ureq::Error::Status(code, r)) => {
            let txt = r.into_string().unwrap_or_default();
            let msg = serde_json::from_str::<Value>(&txt)
                .ok()
                .and_then(|v| v.get("message").or(v.get("detail")).and_then(|m| m.as_str()).map(String::from))
                .unwrap_or(txt);
            Err(BankError::Api(format!("{code} · {msg}")))
        }
        Err(e) => Err(BankError::Net(e.to_string())),
    }
}

#[derive(Serialize)]
pub struct Aspsp {
    pub name: String,
    pub country: String,
    pub logo: Option<String>,
    pub max_consent_days: Option<i64>,
}

pub fn aspsps(cfg: &BankConfig, country: &str) -> Result<Vec<Aspsp>, BankError> {
    let v = call(cfg, "GET", &format!("/aspsps?country={country}&psu_type=personal&service=AIS"), None)?;
    let list = v["aspsps"].as_array().cloned().unwrap_or_default();
    Ok(list
        .into_iter()
        .map(|a| Aspsp {
            name: a["name"].as_str().unwrap_or_default().to_string(),
            country: a["country"].as_str().unwrap_or(country).to_string(),
            logo: a["logo"].as_str().map(String::from),
            max_consent_days: a["maximum_consent_validity"].as_i64().map(|s| s / 86400),
        })
        .collect())
}

/// URLs de redirección registradas en la aplicación de Enable Banking.
pub fn allowed_redirects(cfg: &BankConfig) -> Result<Vec<String>, BankError> {
    let v = call(cfg, "GET", "/application", None)?;
    Ok(v["redirect_urls"].as_array().into_iter().flatten().filter_map(|u| u.as_str().map(String::from)).collect())
}

/// Elige la URL de redirección: la configurada si está registrada; si no, la primera registrada
/// (preferiendo localhost). Devuelve también si ha cambiado.
pub fn pick_redirect(configured: &str, allowed: &[String]) -> Option<String> {
    let norm = |u: &str| u.trim().trim_end_matches('/').to_lowercase();
    if allowed.is_empty() {
        return None;
    }
    if let Some(a) = allowed.iter().find(|a| norm(a) == norm(configured)) {
        return Some(a.clone());
    }
    allowed.iter().find(|a| a.contains("localhost") || a.contains("127.0.0.1")).or(allowed.first()).cloned()
}

pub fn start_auth(cfg: &BankConfig, name: &str, country: &str, days: i64, state: &str) -> Result<String, BankError> {
    let until = (chrono::Utc::now() + chrono::Duration::days(days.clamp(1, 180))).to_rfc3339();
    let v = call(
        cfg,
        "POST",
        "/auth",
        Some(json!({
            "access": { "valid_until": until },
            "aspsp": { "name": name, "country": country },
            "state": state,
            "redirect_url": cfg.redirect_url,
            "psu_type": "personal",
            "language": "es"
        })),
    )?;
    v["url"].as_str().map(String::from).ok_or_else(|| BankError::Api("respuesta sin URL de autorización".into()))
}

/// Extrae `code` de la URL de vuelta pegada por el usuario (o acepta el código directamente).
pub fn extract_code(input: &str) -> Option<String> {
    let s = input.trim();
    if !s.contains('=') && !s.contains('/') && !s.is_empty() {
        return Some(s.to_string());
    }
    let q = s.split_once('?').map(|x| x.1).unwrap_or(s);
    q.split('&').find_map(|kv| kv.strip_prefix("code=")).map(|c| c.split('#').next().unwrap_or(c).to_string())
}

#[derive(Serialize, Clone)]
pub struct LinkedAccount {
    pub uid: String,
    pub name: String,
    pub iban: Option<String>,
    pub currency: String,
}

pub struct Session {
    pub accounts: Vec<LinkedAccount>,
    pub valid_until: Option<String>,
    pub aspsp: String,
}

pub fn create_session(cfg: &BankConfig, code: &str) -> Result<Session, BankError> {
    let v = call(cfg, "POST", "/sessions", Some(json!({ "code": code })))?;
    let accounts = v["accounts"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|a| {
            Some(LinkedAccount {
                uid: a["uid"].as_str()?.to_string(),
                name: a["name"].as_str().or(a["product"].as_str()).unwrap_or("Cuenta").to_string(),
                iban: a["account_id"]["iban"].as_str().map(String::from),
                currency: a["currency"].as_str().unwrap_or("EUR").to_string(),
            })
        })
        .collect();
    Ok(Session {
        accounts,
        valid_until: v["access"]["valid_until"].as_str().map(String::from),
        aspsp: v["aspsp"]["name"].as_str().unwrap_or_default().to_string(),
    })
}

fn cents(v: &Value) -> Option<i64> {
    let s = v.as_str().map(String::from).or_else(|| v.as_f64().map(|f| f.to_string()))?;
    let f: f64 = s.parse().ok()?;
    Some((f * 100.0).round() as i64)
}

pub fn balance(cfg: &BankConfig, uid: &str) -> Result<Option<i64>, BankError> {
    let v = call(cfg, "GET", &format!("/accounts/{uid}/balances"), None)?;
    let list = v["balances"].as_array().cloned().unwrap_or_default();
    for pref in ["CLBD", "ITAV", "CLAV", "XPCD", "ITBD", "OPBD", "OTHR"] {
        if let Some(b) = list.iter().find(|b| b["balance_type"].as_str() == Some(pref)) {
            return Ok(cents(&b["balance_amount"]["amount"]));
        }
    }
    Ok(list.first().and_then(|b| cents(&b["balance_amount"]["amount"])))
}

pub fn map_tx(t: &Value) -> Option<ParsedTx> {
    if t["status"].as_str().is_some_and(|s| s != "BOOK") {
        return None;
    }
    let mut amt = cents(&t["transaction_amount"]["amount"])?.abs();
    if t["credit_debit_indicator"].as_str() == Some("DBIT") {
        amt = -amt;
    }
    let date = t["booking_date"].as_str().or(t["value_date"].as_str()).or(t["transaction_date"].as_str())?.chars().take(10).collect();
    let remit: Vec<String> = t["remittance_information"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
    let party = if amt < 0 { &t["creditor"]["name"] } else { &t["debtor"]["name"] };
    let mut desc = remit.join(" ").trim().to_string();
    if let Some(p) = party.as_str().filter(|p| !p.is_empty() && !desc.to_uppercase().contains(&p.to_uppercase())) {
        desc = if desc.is_empty() { p.to_string() } else { format!("{p} · {desc}") };
    }
    Some(ParsedTx {
        date,
        value_date: t["value_date"].as_str().map(|s| s.chars().take(10).collect()),
        description: if desc.is_empty() { "(sin concepto)".into() } else { desc },
        amount_cents: amt,
        balance_cents: None,
        currency: t["transaction_amount"]["currency"].as_str().unwrap_or("EUR").to_string(),
        external_id: t["entry_reference"].as_str().or(t["transaction_id"].as_str()).filter(|s| !s.is_empty()).map(String::from),
    })
}

pub fn transactions(cfg: &BankConfig, uid: &str, date_from: &str) -> Result<Vec<ParsedTx>, BankError> {
    let mut out = Vec::new();
    let mut cont: Option<String> = None;
    for _ in 0..50 {
        let mut path = format!("/accounts/{uid}/transactions?date_from={date_from}");
        if let Some(c) = &cont {
            path.push_str(&format!("&continuation_key={}", urlencode(c)));
        }
        let v = call(cfg, "GET", &path, None)?;
        out.extend(v["transactions"].as_array().into_iter().flatten().filter_map(map_tx));
        cont = v["continuation_key"].as_str().filter(|s| !s.is_empty()).map(String::from);
        if cont.is_none() {
            break;
        }
    }
    Ok(out)
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Si la URL de vuelta es http://localhost:PUERTO/..., escucha ahí para capturar el código sin copiar/pegar.
pub fn listen_for_code(redirect_url: &str, slot: Arc<Mutex<Option<String>>>) -> bool {
    let rest = redirect_url.strip_prefix("http://localhost:").or_else(|| redirect_url.strip_prefix("http://127.0.0.1:"));
    let Some(port) = rest.and_then(|r| r.split('/').next()).and_then(|p| p.parse::<u16>().ok()) else { return false };
    let Ok(listener) = TcpListener::bind(("127.0.0.1", port)) else { return false };
    listener.set_nonblocking(true).ok();
    std::thread::spawn(move || {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(600) {
            if let Ok((mut s, _)) = listener.accept() {
                s.set_nonblocking(false).ok();
                let mut buf = [0u8; 4096];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let path = req.split_whitespace().nth(1).unwrap_or("");
                let code = extract_code(path).filter(|c| !c.contains('/'));
                let body = if code.is_some() {
                    "<h2 style='font-family:sans-serif'>Banco conectado. Ya puedes volver a CereWare Finances.</h2>"
                } else {
                    "<h2 style='font-family:sans-serif'>No se recibió autorización. Vuelve a la app e inténtalo de nuevo.</h2>"
                };
                let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                if let Some(c) = code {
                    *slot.lock().unwrap() = Some(c);
                    return;
                }
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    });
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_code_and_tx() {
        assert_eq!(extract_code("https://x.com/cb?state=abc&code=XYZ-1#f").as_deref(), Some("XYZ-1"));
        assert_eq!(extract_code("  abc123 ").as_deref(), Some("abc123"));
        assert_eq!(extract_code("/callback?code=k9").as_deref(), Some("k9"));
        let t = json!({"transaction_amount":{"currency":"EUR","amount":"54.30"},"credit_debit_indicator":"DBIT","status":"BOOK",
            "booking_date":"2026-10-24","remittance_information":["COMPRA TARJETA"],"creditor":{"name":"MERCADONA"},"entry_reference":"E1"});
        let p = map_tx(&t).unwrap();
        assert_eq!(p.amount_cents, -5430);
        assert_eq!(p.description, "MERCADONA · COMPRA TARJETA");
        assert_eq!(p.external_id.as_deref(), Some("E1"));
        assert!(map_tx(&json!({"transaction_amount":{"amount":"1"},"status":"PDNG","booking_date":"2026-01-01"})).is_none());
        let allowed = vec!["https://localhost:8765/callback/".to_string(), "https://ejemplo.com/cb".to_string()];
        assert_eq!(pick_redirect("https://localhost:8765/callback", &allowed).as_deref(), Some("https://localhost:8765/callback/"));
        assert_eq!(pick_redirect("http://otra", &allowed).as_deref(), Some("https://localhost:8765/callback/"));
        assert_eq!(pick_redirect("x", &[]), None);
        let cfg = BankConfig { app_id: "x".into(), private_key: "nope".into(), redirect_url: String::new() };
        assert!(matches!(jwt(&cfg), Err(BankError::BadKey)));
    }
}
