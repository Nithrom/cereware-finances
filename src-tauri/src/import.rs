//! Importación de extractos bancarios: CSV, Excel (xls/xlsx/ods), OFX/QFX y Norma 43 (AEB/CSB43).
//! Todo se reduce a una lista de `ParsedTx`. Las tablas (CSV/Excel) se mapean por columnas
//! con detección automática de cabecera y formato numérico.

use calamine::{open_workbook_auto, Data, Reader};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Mapping {
    pub header_row: Option<usize>,
    pub date: Option<usize>,
    pub value_date: Option<usize>,
    pub description: Option<usize>,
    pub detail: Option<usize>,
    pub amount: Option<usize>,
    pub debit: Option<usize>,
    pub credit: Option<usize>,
    pub balance: Option<usize>,
    pub fee: Option<usize>,
    pub state: Option<usize>,
    pub currency: Option<usize>,
    /// Invierte el signo (bancos que exportan gastos en positivo)
    #[serde(default)]
    pub invert: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ParsedTx {
    pub date: String,
    pub value_date: Option<String>,
    pub description: String,
    pub amount_cents: i64,
    pub balance_cents: Option<i64>,
    pub currency: String,
    pub external_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Preview {
    pub format: String,
    pub bank_guess: Option<String>,
    pub columns: Vec<String>,
    pub mapping: Option<Mapping>,
    pub sample: Vec<ParsedTx>,
    pub total: usize,
    pub skipped: usize,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub last_balance_cents: Option<i64>,
    pub account_hint: Option<String>,
}

pub struct Parsed {
    pub format: String,
    pub bank_guess: Option<String>,
    pub columns: Vec<String>,
    pub mapping: Option<Mapping>,
    pub txs: Vec<ParsedTx>,
    pub skipped: usize,
    pub account_hint: Option<String>,
}

impl Parsed {
    pub fn preview(&self, n: usize) -> Preview {
        let mut dates: Vec<&String> = self.txs.iter().map(|t| &t.date).collect();
        dates.sort();
        Preview {
            format: self.format.clone(),
            bank_guess: self.bank_guess.clone(),
            columns: self.columns.clone(),
            mapping: self.mapping.clone(),
            sample: self.txs.iter().take(n).cloned().collect(),
            total: self.txs.len(),
            skipped: self.skipped,
            date_from: dates.first().map(|s| s.to_string()),
            date_to: dates.last().map(|s| s.to_string()),
            last_balance_cents: latest_balance(&self.txs),
            account_hint: self.account_hint.clone(),
        }
    }
}

/// Saldo del movimiento más reciente (respeta el orden del fichero dentro del mismo día).
pub fn latest_balance(txs: &[ParsedTx]) -> Option<i64> {
    let with: Vec<&ParsedTx> = txs.iter().filter(|t| t.balance_cents.is_some()).collect();
    if with.is_empty() {
        return None;
    }
    let max = with.iter().map(|t| &t.date).max()?;
    let desc_order = with.first()?.date >= with.last()?.date;
    let same: Vec<&&ParsedTx> = with.iter().filter(|t| &t.date == max).collect();
    let pick = if desc_order { same.first() } else { same.last() };
    pick.and_then(|t| t.balance_cents)
}

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("No se pudo leer el fichero: {0}")]
    Io(#[from] std::io::Error),
    #[error("No se pudo leer el Excel ({0}). Prueba a descargar el extracto en CSV desde tu banco.")]
    Excel(String),
    #[error("No se reconoce el formato del fichero")]
    Unknown,
    #[error("No se han encontrado las columnas de fecha e importe. Ajusta las columnas manualmente.")]
    NoColumns,
}

// ---------------------------------------------------------------- utilidades

pub fn norm(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.trim().to_lowercase().chars() {
        let c = match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            '.' | '_' | '(' | ')' | ':' => ' ',
            c => c,
        };
        out.push(c);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn parse_date(s: &str) -> Option<NaiveDate> {
    let s = s.trim();
    let tok = s.split(|c| c == ' ' || c == 'T').next().unwrap_or("");
    for f in ["%d/%m/%Y", "%d-%m-%Y", "%Y-%m-%d", "%d.%m.%Y", "%Y/%m/%d", "%d/%m/%y", "%d-%m-%y", "%Y%m%d"] {
        if let Ok(d) = NaiveDate::parse_from_str(tok, f) {
            if (1990..=2100).contains(&chrono::Datelike::year(&d)) {
                return Some(d);
            }
        }
    }
    None
}

/// Separador decimal: ',' o '.'
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Dec {
    Comma,
    Dot,
}

pub fn detect_decimal<'a>(values: impl Iterator<Item = &'a str>) -> Dec {
    let (mut comma, mut dot) = (0, 0);
    for v in values {
        let v = v.trim();
        if let Some(i) = v.rfind(|c| c == ',' || c == '.') {
            let tail = v[i + 1..].chars().filter(|c| c.is_ascii_digit()).count();
            let is_comma = v[i..].starts_with(',');
            if tail >= 1 && tail <= 2 {
                if is_comma { comma += 1 } else { dot += 1 }
            } else if tail == 3 && v.contains(',') && v.contains('.') {
                if is_comma { dot += 1 } else { comma += 1 }
            }
        }
    }
    if comma > dot { Dec::Comma } else { Dec::Dot }
}

pub fn parse_amount(s: &str, dec: Dec) -> Option<i64> {
    let mut t: String = s.trim().chars().filter(|c| !c.is_whitespace() && *c != '\u{a0}' && *c != '€').collect();
    t = t.replace("EUR", "").replace("eur", "");
    if t.is_empty() {
        return None;
    }
    let mut neg = false;
    if t.starts_with('(') && t.ends_with(')') {
        neg = true;
        t = t[1..t.len() - 1].to_string();
    }
    if t.ends_with('-') {
        neg = true;
        t.pop();
    }
    if t.starts_with('-') || t.starts_with('−') {
        neg = !neg;
        t = t.trim_start_matches(['-', '−']).to_string();
    }
    t = t.trim_start_matches('+').to_string();
    let t = match dec {
        Dec::Comma => t.replace('.', "").replace(',', "."),
        Dec::Dot => t.replace(',', ""),
    };
    let v: f64 = t.parse().ok()?;
    let c = (v * 100.0).round() as i64;
    Some(if neg { -c } else { c })
}

fn excel_serial_to_date(f: f64) -> Option<NaiveDate> {
    if !(20000.0..80000.0).contains(&f) {
        return None;
    }
    NaiveDate::from_ymd_opt(1899, 12, 30)?.checked_add_days(chrono::Days::new(f.floor() as u64))
}

fn decode(bytes: &[u8]) -> String {
    let b = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    match std::str::from_utf8(b) {
        Ok(s) => s.to_string(),
        Err(_) => encoding_rs::WINDOWS_1252.decode(b).0.into_owned(),
    }
}

// ---------------------------------------------------------------- lectura de tablas

fn read_csv(text: &str) -> Vec<Vec<String>> {
    let first: Vec<&str> = text.lines().take(30).collect();
    let delim = [b';', b',', b'\t', b'|']
        .into_iter()
        .max_by_key(|d| {
            let counts: Vec<usize> = first.iter().map(|l| l.matches(*d as char).count()).collect();
            let max = *counts.iter().max().unwrap_or(&0);
            counts.iter().filter(|c| **c == max && max > 0).count() * 10 + max
        })
        .unwrap_or(b';');
    let mut rdr = csv::ReaderBuilder::new().delimiter(delim).has_headers(false).flexible(true).from_reader(text.as_bytes());
    rdr.records().filter_map(|r| r.ok()).map(|r| r.iter().map(|c| c.trim().to_string()).collect()).collect()
}

/// Muchos bancos exportan ".xls" que en realidad son HTML, XML (SpreadsheetML) o texto.
fn read_excel(path: &Path) -> Result<Vec<Vec<String>>, ImportError> {
    match read_excel_real(path) {
        Ok(r) => Ok(r),
        Err(e) => {
            let bytes = std::fs::read(path)?;
            if bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0]) || bytes.starts_with(b"PK") {
                return Err(e);
            }
            let text = decode(&bytes);
            let low: String = text.chars().take(4000).collect::<String>().to_lowercase();
            if low.contains("urn:schemas-microsoft-com:office:spreadsheet") || low.contains("<workbook") {
                Ok(read_spreadsheet_xml(&text))
            } else if low.contains("<table") || low.contains("<html") {
                Ok(read_html_table(&text))
            } else {
                Ok(read_csv(&text))
            }
        }
    }
}

fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let Some(end) = tail.find(';').filter(|&e| e <= 10) else {
            out.push('&');
            rest = &tail[1..];
            continue;
        };
        let ent = &tail[1..end];
        let rep: Option<String> = if let Some(n) = ent.strip_prefix("#x").or_else(|| ent.strip_prefix("#X")) {
            u32::from_str_radix(n, 16).ok().and_then(char::from_u32).map(String::from)
        } else if let Some(n) = ent.strip_prefix('#') {
            n.parse::<u32>().ok().and_then(char::from_u32).map(String::from)
        } else {
            match ent {
                "nbsp" => Some(" "), "amp" => Some("&"), "lt" => Some("<"), "gt" => Some(">"), "quot" => Some("\""), "apos" => Some("'"),
                "aacute" => Some("á"), "eacute" => Some("é"), "iacute" => Some("í"), "oacute" => Some("ó"), "uacute" => Some("ú"),
                "Aacute" => Some("Á"), "Eacute" => Some("É"), "Iacute" => Some("Í"), "Oacute" => Some("Ó"), "Uacute" => Some("Ú"),
                "ntilde" => Some("ñ"), "Ntilde" => Some("Ñ"), "uuml" => Some("ü"), "Uuml" => Some("Ü"), "euro" => Some("€"),
                "ordf" => Some("ª"), "ordm" => Some("º"), "ccedil" => Some("ç"), "Ccedil" => Some("Ç"),
                _ => None,
            }
            .map(String::from)
        };
        match rep {
            Some(r) => { out.push_str(&r); rest = &tail[end + 1..]; }
            None => { out.push('&'); rest = &tail[1..]; }
        }
    }
    out.push_str(rest);
    out
}

fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => { in_tag = false; out.push(' '); }
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    decode_entities(&out).split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Divide `text` por etiquetas `tag` (sin distinguir mayúsculas) devolviendo el contenido interior.
fn split_tag<'a>(text: &'a str, tag: &str) -> Vec<&'a str> {
    let low = text.to_lowercase();
    let open = format!("<{tag}");
    let close = format!("</{tag}");
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(i) = low[pos..].find(&open) {
        let s = pos + i;
        let after = &low[s + open.len()..];
        // evita <td> casando con <tdx>
        if !after.starts_with(|c: char| c == '>' || c.is_whitespace() || c == '/') {
            pos = s + open.len();
            continue;
        }
        let Some(gt) = low[s..].find('>') else { break };
        let body_start = s + gt + 1;
        let next_open = low[body_start..].find(&open).map(|x| body_start + x);
        let end = low[body_start..].find(&close).map(|x| body_start + x);
        let body_end = match (end, next_open) {
            (Some(e), Some(n)) => e.min(n),
            (Some(e), None) => e,
            (None, Some(n)) => n,
            (None, None) => low.len(),
        };
        out.push(&text[body_start..body_end]);
        pos = body_end;
    }
    out
}

fn read_html_table(text: &str) -> Vec<Vec<String>> {
    split_tag(text, "tr")
        .into_iter()
        .map(|tr| {
            let low = tr.to_lowercase();
            let tag = if low.contains("<td") { "td" } else { "th" };
            let mut cells: Vec<String> = Vec::new();
            for c in split_tag(tr, tag) {
                cells.push(strip_tags(c));
            }
            if tag == "td" && low.contains("<th") {
                let mut h: Vec<String> = split_tag(tr, "th").into_iter().map(strip_tags).collect();
                h.extend(cells);
                cells = h;
            }
            cells
        })
        .filter(|r| r.iter().any(|c| !c.is_empty()))
        .collect()
}

fn read_spreadsheet_xml(text: &str) -> Vec<Vec<String>> {
    split_tag(text, "row")
        .into_iter()
        .map(|row| {
            split_tag(row, "cell")
                .into_iter()
                .map(|c| {
                    let v = strip_tags(c);
                    // fechas ISO "2026-10-24T00:00:00.000"
                    if v.len() >= 19 && v.as_bytes().get(10) == Some(&b'T') { v[..10].to_string() } else { v }
                })
                .collect()
        })
        .filter(|r: &Vec<String>| r.iter().any(|c| !c.is_empty()))
        .collect()
}

fn read_excel_real(path: &Path) -> Result<Vec<Vec<String>>, ImportError> {
    let mut wb = open_workbook_auto(path).map_err(|e| ImportError::Excel(e.to_string()))?;
    let range = wb.worksheet_range_at(0).ok_or(ImportError::Unknown)?.map_err(|e| ImportError::Excel(e.to_string()))?;
    Ok(range
        .rows()
        .map(|row| {
            row.iter()
                .map(|c| match c {
                    Data::Empty => String::new(),
                    Data::String(s) => s.trim().to_string(),
                    Data::Float(f) => format!("{f}"),
                    Data::Int(i) => i.to_string(),
                    Data::Bool(b) => b.to_string(),
                    Data::DateTime(dt) => excel_serial_to_date(dt.as_f64()).map(|d| d.to_string()).unwrap_or_default(),
                    Data::DateTimeIso(s) => s.chars().take(10).collect(),
                    _ => String::new(),
                })
                .collect()
        })
        .collect())
}

// ---------------------------------------------------------------- detección de columnas

type Keys = &'static [&'static str];
const K_DATE: Keys = &["fecha operacion", "fecha de operacion", "f operacion", "completed date", "fecha", "fecha contable", "booking date", "transaction date", "date", "started date"];
const K_VALUE: Keys = &["fecha valor", "f valor", "value date", "fecha de valor"];
const K_DESC: Keys = &["concepto", "descripcion", "description", "concept", "detalle", "merchant", "beneficiario", "payee", "descripcion ampliada"];
const K_DETAIL: Keys = &["movimiento", "observaciones", "mas datos", "notes", "referencia", "informacion adicional"];
const K_AMOUNT: Keys = &["importe", "amount", "cantidad", "importe eur", "importe en euros", "monto"];
const K_DEBIT: Keys = &["cargo", "cargos", "debe", "debit", "salida", "gasto", "reintegros"];
const K_CREDIT: Keys = &["abono", "abonos", "haber", "credit", "entrada", "ingreso", "ingresos"];
const K_BALANCE: Keys = &["saldo", "balance", "disponible", "saldo disponible", "saldo contable"];
const K_FEE: Keys = &["fee", "comision", "comisiones"];
const K_STATE: Keys = &["state", "estado"];
const K_CURRENCY: Keys = &["currency", "divisa", "moneda"];

fn find(cols: &[String], keys: Keys, used: &mut Vec<usize>) -> Option<usize> {
    let pick = |pred: &dyn Fn(&str, &str) -> bool| {
        for k in keys {
            for (i, c) in cols.iter().enumerate() {
                if !used.contains(&i) && pred(c, k) {
                    return Some(i);
                }
            }
        }
        None
    };
    let r = pick(&|c, k| c == k).or_else(|| pick(&|c, k| k.len() > 4 && c.starts_with(k)));
    if let Some(i) = r {
        used.push(i);
    }
    r
}

pub fn guess_mapping(rows: &[Vec<String>]) -> Option<Mapping> {
    let mut best: Option<(usize, Mapping, usize)> = None;
    for (ri, row) in rows.iter().take(40).enumerate() {
        let cols: Vec<String> = row.iter().map(|c| norm(c)).collect();
        let mut used = Vec::new();
        let mut m = Mapping { header_row: Some(ri), ..Default::default() };
        m.value_date = find(&cols, K_VALUE, &mut used);
        m.date = find(&cols, K_DATE, &mut used);
        m.amount = find(&cols, K_AMOUNT, &mut used);
        m.debit = find(&cols, K_DEBIT, &mut used);
        m.credit = find(&cols, K_CREDIT, &mut used);
        m.balance = find(&cols, K_BALANCE, &mut used);
        m.fee = find(&cols, K_FEE, &mut used);
        m.state = find(&cols, K_STATE, &mut used);
        m.currency = find(&cols, K_CURRENCY, &mut used);
        m.description = find(&cols, K_DESC, &mut used);
        m.detail = find(&cols, K_DETAIL, &mut used);
        if m.date.is_none() && m.value_date.is_some() {
            m.date = m.value_date.take();
        }
        let has_amount = m.amount.is_some() || (m.debit.is_some() && m.credit.is_some());
        if m.date.is_none() || !has_amount {
            continue;
        }
        if m.description.is_none() {
            m.description = m.detail.take();
        }
        let score = used.len();
        if best.as_ref().map_or(true, |b| score > b.2) {
            best = Some((ri, m, score));
        }
    }
    best.map(|b| b.1).or_else(|| guess_by_content(rows))
}

/// Ficheros sin cabecera: fecha = primera columna con fechas, importe = última numérica con signo.
fn guess_by_content(rows: &[Vec<String>]) -> Option<Mapping> {
    let sample: Vec<&Vec<String>> = rows.iter().filter(|r| r.iter().any(|c| parse_date(c).is_some())).take(20).collect();
    if sample.len() < 2 {
        return None;
    }
    let ncols = sample.iter().map(|r| r.len()).max()?;
    let frac = |i: usize, f: &dyn Fn(&str) -> bool| sample.iter().filter(|r| r.get(i).map_or(false, |c| f(c))).count() * 100 / sample.len();
    let date = (0..ncols).find(|&i| frac(i, &|c| parse_date(c).is_some()) > 80)?;
    let numeric: Vec<usize> = (0..ncols).filter(|&i| i != date && frac(i, &|c| parse_amount(c, Dec::Comma).is_some() && parse_date(c).is_none()) > 80).collect();
    let amount = *numeric.iter().find(|&&i| frac(i, &|c| c.trim_start().starts_with('-')) > 0).or(numeric.first())?;
    let balance = numeric.iter().copied().filter(|&i| i > amount).last();
    let description = (0..ncols).filter(|&i| i != date && !numeric.contains(&i)).max_by_key(|&i| sample.iter().map(|r| r.get(i).map_or(0, |c| c.len())).sum::<usize>());
    Some(Mapping { header_row: None, date: Some(date), amount: Some(amount), balance, description, ..Default::default() })
}

fn bank_guess(cols: &[String]) -> Option<String> {
    let has = |k: &str| cols.iter().any(|c| c == k);
    if has("started date") && has("product") {
        Some("Revolut".into())
    } else if has("f valor") && has("disponible") {
        Some("BBVA".into())
    } else if has("fecha operacion") && has("fecha valor") && has("importe") && has("saldo") {
        Some("Openbank / Santander".into())
    } else {
        None
    }
}

pub fn apply_mapping(rows: &[Vec<String>], m: &Mapping) -> (Vec<ParsedTx>, usize) {
    let start = m.header_row.map(|h| h + 1).unwrap_or(0);
    let body = &rows[start.min(rows.len())..];
    fn get(r: &[String], i: Option<usize>) -> &str {
        i.and_then(|i| r.get(i)).map(|s| s.as_str()).unwrap_or("")
    }
    let num_cols = [m.amount, m.debit, m.credit, m.balance, m.fee];
    let dec = detect_decimal(body.iter().flat_map(|r| num_cols.iter().map(move |c| get(r, *c))));
    let (mut out, mut skipped) = (Vec::new(), 0);
    for r in body {
        let Some(date) = parse_date(get(r, m.date)) else {
            if r.iter().any(|c| !c.is_empty()) { skipped += 1 }
            continue;
        };
        if m.state.is_some() {
            let st = norm(get(r, m.state));
            if !st.is_empty() && !["completed", "completado", "completada", "liquidado"].contains(&st.as_str()) {
                skipped += 1;
                continue;
            }
        }
        let amount = if m.amount.is_some() {
            parse_amount(get(r, m.amount), dec)
        } else {
            let d = parse_amount(get(r, m.debit), dec).unwrap_or(0).abs();
            let c = parse_amount(get(r, m.credit), dec).unwrap_or(0).abs();
            if get(r, m.debit).is_empty() && get(r, m.credit).is_empty() { None } else { Some(c - d) }
        };
        let Some(mut amount) = amount else {
            skipped += 1;
            continue;
        };
        if let Some(fee) = parse_amount(get(r, m.fee), dec) {
            amount -= fee.abs();
        }
        if m.invert {
            amount = -amount;
        }
        let mut desc = get(r, m.description).to_string();
        let detail = get(r, m.detail);
        if !detail.is_empty() && !desc.contains(detail) {
            desc = if desc.is_empty() { detail.to_string() } else { format!("{desc} · {detail}") };
        }
        let cur = get(r, m.currency);
        out.push(ParsedTx {
            date: date.to_string(),
            value_date: parse_date(get(r, m.value_date)).map(|d| d.to_string()),
            description: if desc.is_empty() { "(sin concepto)".into() } else { desc },
            amount_cents: amount,
            balance_cents: parse_amount(get(r, m.balance), dec),
            currency: if cur.len() == 3 { cur.to_uppercase() } else { "EUR".into() },
            external_id: None,
        });
    }
    (out, skipped)
}

// ---------------------------------------------------------------- OFX

fn ofx_tag(block: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let i = block.to_uppercase().find(&open)? + open.len();
    let rest = &block[i..];
    let end = rest.find('<').unwrap_or(rest.len());
    let v = rest[..end].trim();
    if v.is_empty() { None } else { Some(v.to_string()) }
}

pub fn parse_ofx(text: &str) -> (Vec<ParsedTx>, usize, Option<i64>) {
    let up = text.to_uppercase();
    let (mut out, mut skipped) = (Vec::new(), 0);
    let mut pos = 0;
    while let Some(s) = up[pos..].find("<STMTTRN>") {
        let start = pos + s;
        let end = up[start..].find("</STMTTRN>").map(|e| start + e).unwrap_or(up.len());
        let block = &text[start..end];
        pos = end.max(start + 1);
        let date = ofx_tag(block, "DTPOSTED").and_then(|d| parse_date(&d.chars().take(8).collect::<String>()));
        let amount = ofx_tag(block, "TRNAMT").and_then(|a| parse_amount(&a, Dec::Dot));
        match (date, amount) {
            (Some(d), Some(a)) => {
                let name = ofx_tag(block, "NAME").unwrap_or_default();
                let memo = ofx_tag(block, "MEMO").unwrap_or_default();
                let desc = match (name.is_empty(), memo.is_empty() || name.contains(&memo)) {
                    (true, _) => memo,
                    (false, true) => name,
                    (false, false) => format!("{name} · {memo}"),
                };
                out.push(ParsedTx {
                    date: d.to_string(),
                    value_date: None,
                    description: if desc.is_empty() { "(sin concepto)".into() } else { desc },
                    amount_cents: a,
                    balance_cents: None,
                    currency: "EUR".into(),
                    external_id: ofx_tag(block, "FITID"),
                });
            }
            _ => skipped += 1,
        }
    }
    let bal = up.find("<LEDGERBAL>").and_then(|i| ofx_tag(&text[i..], "BALAMT")).and_then(|b| parse_amount(&b, Dec::Dot));
    (out, skipped, bal)
}

// ---------------------------------------------------------------- Norma 43

pub fn parse_n43(text: &str) -> (Vec<ParsedTx>, usize, Option<i64>, Option<String>) {
    let (mut out, mut skipped, mut final_bal, mut account) = (Vec::<ParsedTx>::new(), 0, None, None);
    let mut cur_currency = "EUR".to_string();
    let sub = |l: &str, a: usize, b: usize| l.get(a - 1..b.min(l.len())).unwrap_or("").to_string();
    let ymd = |s: &str| NaiveDate::parse_from_str(&format!("20{s}"), "%Y%m%d").ok();
    for line in text.lines() {
        let l = line.trim_end_matches(['\r', '\n']);
        match l.get(0..2) {
            Some("11") => {
                account = Some(format!("{}{}{}", sub(l, 3, 6), sub(l, 7, 10), sub(l, 11, 20)));
                if sub(l, 48, 50) == "978" { cur_currency = "EUR".into() }
            }
            Some("22") => {
                let date = ymd(&sub(l, 11, 16));
                let vdate = ymd(&sub(l, 17, 22));
                let amt: Option<i64> = sub(l, 29, 42).trim().parse().ok();
                match (date, amt) {
                    (Some(d), Some(a)) => {
                        let signed = if sub(l, 28, 28) == "1" { -a } else { a };
                        let r2 = sub(l, 65, 80).trim().to_string();
                        let doc = sub(l, 43, 52).trim().to_string();
                        out.push(ParsedTx {
                            date: d.to_string(),
                            value_date: vdate.map(|d| d.to_string()),
                            description: if r2.is_empty() { "(sin concepto)".into() } else { r2 },
                            amount_cents: signed,
                            balance_cents: None,
                            currency: cur_currency.clone(),
                            external_id: if doc.chars().all(|c| c == '0') { None } else { Some(doc) },
                        });
                    }
                    _ => skipped += 1,
                }
            }
            Some("23") => {
                if let Some(last) = out.last_mut() {
                    let text = format!("{} {}", sub(l, 5, 42).trim(), sub(l, 43, 80).trim()).trim().to_string();
                    if !text.is_empty() {
                        if sub(l, 3, 4) == "01" || last.description == "(sin concepto)" {
                            last.description = text;
                        } else if !last.description.contains(&text) {
                            last.description = format!("{} · {}", last.description, text);
                        }
                    }
                }
            }
            Some("33") => {
                if let Ok(v) = sub(l, 60, 73).trim().parse::<i64>() {
                    final_bal = Some(if sub(l, 59, 59) == "1" { -v } else { v });
                }
            }
            _ => {}
        }
    }
    (out, skipped, final_bal, account)
}

// ---------------------------------------------------------------- entrada principal

pub fn parse_file(path: &Path, mapping: Option<Mapping>) -> Result<Parsed, ImportError> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let rows = if matches!(ext.as_str(), "xlsx" | "xls" | "xlsm" | "ods") {
        read_excel(path)?
    } else {
        let text = decode(&std::fs::read(path)?);
        let head = text.trim_start();
        let up: String = head.chars().take(2000).collect::<String>().to_uppercase();
        if up.contains("<OFX>") || up.starts_with("OFXHEADER") || up.contains("<STMTTRN>") {
            let (mut txs, skipped, bal) = parse_ofx(&text);
            if let (Some(b), Some(last)) = (bal, txs.iter_mut().max_by(|a, b| a.date.cmp(&b.date))) {
                last.balance_cents = Some(b);
            }
            return Ok(Parsed { format: "ofx".into(), bank_guess: None, columns: vec![], mapping: None, txs, skipped, account_hint: None });
        }
        let first = head.lines().next().unwrap_or("");
        if first.starts_with("11") && first.len() >= 40 && head.lines().any(|l| l.starts_with("22")) {
            let (mut txs, skipped, bal, acc) = parse_n43(&text);
            if let (Some(b), Some(last)) = (bal, txs.last_mut()) {
                last.balance_cents = Some(b);
            }
            return Ok(Parsed { format: "n43".into(), bank_guess: None, columns: vec![], mapping: None, txs, skipped, account_hint: acc });
        }
        read_csv(&text)
    };
    let format = if matches!(ext.as_str(), "xlsx" | "xls" | "xlsm" | "ods") { "excel" } else { "csv" };
    let m = match mapping {
        Some(m) => m,
        None => guess_mapping(&rows).ok_or(ImportError::NoColumns)?,
    };
    let columns: Vec<String> = match m.header_row {
        Some(h) => rows.get(h).cloned().unwrap_or_default(),
        None => (1..=rows.iter().map(|r| r.len()).max().unwrap_or(0)).map(|i| format!("Columna {i}")).collect(),
    };
    let ncols: Vec<String> = columns.iter().map(|c| norm(c)).collect();
    let (txs, skipped) = apply_mapping(&rows, &m);
    Ok(Parsed { format: format.into(), bank_guess: bank_guess(&ncols), columns, mapping: Some(m), txs, skipped, account_hint: None })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str, content: &[u8]) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("cwf-{}-{name}", uuid::Uuid::new_v4()));
        std::fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn amounts() {
        assert_eq!(parse_amount("-1.234,56", Dec::Comma), Some(-123456));
        assert_eq!(parse_amount("1,234.56", Dec::Dot), Some(123456));
        assert_eq!(parse_amount("12,5 €", Dec::Comma), Some(1250));
        assert_eq!(parse_amount("(45.10)", Dec::Dot), Some(-4510));
        assert_eq!(parse_amount("−9,99", Dec::Comma), Some(-999));
        assert_eq!(detect_decimal(["-12,50", "1.200,00"].into_iter()), Dec::Comma);
        assert_eq!(detect_decimal(["-12.50", "1200.00"].into_iter()), Dec::Dot);
    }

    #[test]
    fn openbank_csv_latin1() {
        let s = "Movimientos de la cuenta\n\nFecha Operación;Fecha Valor;Concepto;Importe;Saldo\n24/10/2026;24/10/2026;COMPRA EN MERCADONA, CARTAGENA;-54,30;6.912,10\n23/10/2026;23/10/2026;TRANSFERENCIA DE CLIENTE WEB;1.210,00;6.966,40\n";
        let (bytes, _, _) = encoding_rs::WINDOWS_1252.encode(s);
        let p = tmp("ob.csv", &bytes);
        let r = parse_file(&p, None).unwrap();
        assert_eq!(r.txs.len(), 2);
        assert_eq!(r.txs[0].amount_cents, -5430);
        assert_eq!(r.txs[1].amount_cents, 121000);
        assert_eq!(r.bank_guess.as_deref(), Some("Openbank / Santander"));
        assert_eq!(latest_balance(&r.txs), Some(691210));
    }

    #[test]
    fn revolut_csv() {
        let s = "Type,Product,Started Date,Completed Date,Description,Amount,Fee,Currency,State,Balance\nCARD_PAYMENT,Current,2026-10-23 10:01:02,2026-10-23 12:00:00,Spotify,-11.99,0.00,EUR,COMPLETED,1362.48\nCARD_PAYMENT,Current,2026-10-24 10:01:02,,Amazon,-20.00,0.00,EUR,PENDING,\nTOPUP,Current,2026-10-20 10:01:02,2026-10-20 10:02:00,Top-up,100.00,0.00,EUR,COMPLETED,1374.47\n";
        let r = parse_file(&tmp("rev.csv", s.as_bytes()), None).unwrap();
        assert_eq!(r.txs.len(), 2);
        assert_eq!(r.skipped, 1);
        assert_eq!(r.txs[0].amount_cents, -1199);
        assert_eq!(r.bank_guess.as_deref(), Some("Revolut"));
        assert_eq!(latest_balance(&r.txs), Some(136248));
    }

    #[test]
    fn bbva_debit_credit_and_content_guess() {
        let s = "F.Valor;Fecha;Concepto;Movimiento;Cargo;Abono;Disponible\n21/10/2026;21/10/2026;Cuota autónomo;Seg. Social;230,15;;4.205,77\n20/10/2026;20/10/2026;Nómina;Empresa;;1.500,00;4.435,92\n";
        let r = parse_file(&tmp("bbva.csv", s.as_bytes()), None).unwrap();
        assert_eq!(r.txs[0].amount_cents, -23015);
        assert_eq!(r.txs[1].amount_cents, 150000);
        assert!(r.txs[0].description.contains("Seg. Social"));
        let s2 = "01/10/2026;Café;-2,50;100,00\n02/10/2026;Pan;-1,20;98,80\n03/10/2026;Bizum;10,00;108,80\n";
        let r2 = parse_file(&tmp("nohead.csv", s2.as_bytes()), None).unwrap();
        assert_eq!(r2.txs.len(), 3);
        assert_eq!(r2.txs[2].amount_cents, 1000);
        assert_eq!(latest_balance(&r2.txs), Some(10880));
    }

    #[test]
    fn fake_xls_html_and_xml() {
        let html = "<html><head><meta charset='utf-8'></head><body><table><tr><td colspan=5>Movimientos</td></tr>\n<tr><th>Fecha Operaci&oacute;n</th><th>Fecha Valor</th><th>Concepto</th><th>Importe</th><th>Saldo</th></tr>\n<tr><td>24/10/2026</td><td>24/10/2026</td><td>COMPRA&nbsp;EN MERCADONA</td><td>-54,30</td><td>6.912,10</td></tr><tr><td>23/10/2026</td><td>23/10/2026</td><td><b>BIZUM</b> DE ANA</td><td>20,00</td><td>6.966,40</td></tr></table></body></html>";
        let r = parse_file(&tmp("ob.xls", html.as_bytes()), None).unwrap();
        assert_eq!(r.txs.len(), 2);
        assert_eq!(r.txs[0].description, "COMPRA EN MERCADONA");
        assert_eq!(r.txs[0].amount_cents, -5430);
        assert_eq!(r.txs[1].description, "BIZUM DE ANA");
        assert_eq!(latest_balance(&r.txs), Some(691210));
        let xml = "<?xml version=\"1.0\"?><Workbook xmlns=\"urn:schemas-microsoft-com:office:spreadsheet\"><Worksheet><Table><Row><Cell><Data ss:Type=\"String\">Fecha</Data></Cell><Cell><Data ss:Type=\"String\">Concepto</Data></Cell><Cell><Data>Importe</Data></Cell></Row><Row><Cell><Data ss:Type=\"DateTime\">2026-10-22T00:00:00.000</Data></Cell><Cell><Data>REPSOL</Data></Cell><Cell><Data ss:Type=\"Number\">-48</Data></Cell></Row></Table></Worksheet></Workbook>";
        let r = parse_file(&tmp("bbva.xls", xml.as_bytes()), None).unwrap();
        assert_eq!(r.txs.len(), 1);
        assert_eq!(r.txs[0].date, "2026-10-22");
        assert_eq!(r.txs[0].amount_cents, -4800);
        let tsv = "Fecha\tConcepto\tImporte\n01/10/2026\tPan\t-1,20\n";
        let r = parse_file(&tmp("t.xls", tsv.as_bytes()), None).unwrap();
        assert_eq!(r.txs[0].amount_cents, -120);
    }

    #[test]
    fn ofx() {
        let s = "OFXHEADER:100\n<OFX><BANKMSGSRSV1><STMTTRNRS><STMTRS><BANKTRANLIST>\n<STMTTRN><TRNTYPE>DEBIT<DTPOSTED>20261022<TRNAMT>-48.00<FITID>A1<NAME>REPSOL<MEMO>Gasolinera</STMTTRN>\n<STMTTRN><TRNTYPE>CREDIT<DTPOSTED>20261021120000<TRNAMT>15.5<FITID>A2<NAME>Bizum</STMTTRN>\n</BANKTRANLIST><LEDGERBAL><BALAMT>500.25<DTASOF>20261022</LEDGERBAL></STMTRS></STMTTRNRS></BANKMSGSRSV1></OFX>";
        let r = parse_file(&tmp("x.ofx", s.as_bytes()), None).unwrap();
        assert_eq!(r.txs.len(), 2);
        assert_eq!(r.txs[0].description, "REPSOL · Gasolinera");
        assert_eq!(r.txs[1].amount_cents, 1550);
        assert_eq!(latest_balance(&r.txs), Some(50025));
    }

    #[test]
    fn norma43() {
        let pad = |s: String| format!("{s:<80}");
        let l11 = pad(format!("11{}{}{}{}{}{}{}{}", "0073", "0100", "0123456789", "261001", "261031", "2", "00000000100000", "978"));
        let l22 = pad(format!("22{}{}{}{}{}{}{}{}{}{}", "    ", "0100", "261005", "261005", "12", "345", "1", "00000000005430", "0000000000", "            "));
        let l23 = pad(format!("23{}{:<38}{:<38}", "01", "COMPRA MERCADONA", ""));
        let l33 = pad(format!("33{}{}{}{}{}{}{}{}{}", "0073", "0100", "0123456789", "00001", "00000000005430", "00000", "00000000000000", "2", "00000000094570"));
        let s = format!("{l11}\n{l22}\n{l23}\n{l33}\n88\n");
        let r = parse_file(&tmp("n43.txt", s.as_bytes()), None).unwrap();
        assert_eq!(r.format, "n43");
        assert_eq!(r.txs.len(), 1);
        assert_eq!(r.txs[0].amount_cents, -5430);
        assert_eq!(r.txs[0].description, "COMPRA MERCADONA");
        assert_eq!(r.txs[0].date, "2026-10-05");
        assert_eq!(latest_balance(&r.txs), Some(94570));
    }
}
