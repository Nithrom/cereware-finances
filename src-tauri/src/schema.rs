//! Esquema y migraciones (PRAGMA user_version).

use rusqlite::Connection;

const MIGRATIONS: &[&str] = &[
    // v1
    r#"
    CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);

    CREATE TABLE accounts (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        institution TEXT NOT NULL,
        kind TEXT NOT NULL DEFAULT 'checking',      -- checking | savings | card | cash | investment
        currency TEXT NOT NULL DEFAULT 'EUR',
        iban_masked TEXT,
        balance_cents INTEGER NOT NULL DEFAULT 0,
        provider TEXT NOT NULL DEFAULT 'manual',    -- manual | import | enablebanking
        provider_ref TEXT,
        consent_expires_at TEXT,
        is_business INTEGER NOT NULL DEFAULT 0,
        color TEXT,
        archived INTEGER NOT NULL DEFAULT 0,
        created_at TEXT NOT NULL DEFAULT (datetime('now')),
        updated_at TEXT NOT NULL DEFAULT (datetime('now'))
    );

    CREATE TABLE categories (
        id TEXT PRIMARY KEY,
        parent_id TEXT REFERENCES categories(id) ON DELETE SET NULL,
        name TEXT NOT NULL,
        kind TEXT NOT NULL,                         -- expense | income | transfer
        color TEXT NOT NULL,
        icon TEXT NOT NULL,
        sort INTEGER NOT NULL DEFAULT 0,
        is_system INTEGER NOT NULL DEFAULT 0
    );

    CREATE TABLE transactions (
        id TEXT PRIMARY KEY,
        account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
        booking_date TEXT NOT NULL,
        value_date TEXT,
        amount_cents INTEGER NOT NULL,
        currency TEXT NOT NULL DEFAULT 'EUR',
        description TEXT NOT NULL,
        counterparty TEXT,
        category_id TEXT REFERENCES categories(id) ON DELETE SET NULL,
        notes TEXT,
        tags TEXT NOT NULL DEFAULT '[]',
        is_business INTEGER NOT NULL DEFAULT 0,
        vat_rate REAL,
        external_id TEXT,
        import_hash TEXT UNIQUE,
        transfer_group TEXT,
        created_at TEXT NOT NULL DEFAULT (datetime('now')),
        updated_at TEXT NOT NULL DEFAULT (datetime('now'))
    );
    CREATE INDEX idx_tx_date ON transactions(booking_date);
    CREATE INDEX idx_tx_account ON transactions(account_id);
    CREATE INDEX idx_tx_category ON transactions(category_id);

    CREATE TABLE transaction_splits (
        id TEXT PRIMARY KEY,
        transaction_id TEXT NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
        category_id TEXT REFERENCES categories(id) ON DELETE SET NULL,
        amount_cents INTEGER NOT NULL,
        notes TEXT
    );

    CREATE TABLE rules (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        priority INTEGER NOT NULL DEFAULT 100,
        match_field TEXT NOT NULL DEFAULT 'description', -- description | counterparty
        match_op TEXT NOT NULL DEFAULT 'contains',        -- contains | equals | starts | regex
        match_value TEXT NOT NULL,
        amount_min_cents INTEGER,
        amount_max_cents INTEGER,
        account_id TEXT REFERENCES accounts(id) ON DELETE CASCADE,
        set_category_id TEXT REFERENCES categories(id) ON DELETE CASCADE,
        set_business INTEGER,
        learned INTEGER NOT NULL DEFAULT 0,
        enabled INTEGER NOT NULL DEFAULT 1
    );

    CREATE TABLE budgets (
        id TEXT PRIMARY KEY,
        category_id TEXT NOT NULL REFERENCES categories(id) ON DELETE CASCADE,
        month TEXT,                                 -- 'YYYY-MM' o NULL = por defecto
        amount_cents INTEGER NOT NULL,
        rollover INTEGER NOT NULL DEFAULT 0,
        UNIQUE(category_id, month)
    );

    CREATE TABLE recurring (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        account_id TEXT REFERENCES accounts(id) ON DELETE SET NULL,
        category_id TEXT REFERENCES categories(id) ON DELETE SET NULL,
        amount_cents INTEGER NOT NULL,
        interval TEXT NOT NULL,                     -- weekly | monthly | quarterly | yearly
        next_date TEXT NOT NULL,
        match_value TEXT,
        source TEXT NOT NULL DEFAULT 'manual',      -- manual | detected
        is_subscription INTEGER NOT NULL DEFAULT 0,
        active INTEGER NOT NULL DEFAULT 1
    );

    CREATE TABLE goals (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        target_cents INTEGER NOT NULL,
        saved_cents INTEGER NOT NULL DEFAULT 0,
        target_date TEXT,
        account_id TEXT REFERENCES accounts(id) ON DELETE SET NULL,
        color TEXT NOT NULL DEFAULT '#E5383B',
        archived INTEGER NOT NULL DEFAULT 0
    );
    "#,
];

pub fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    let current: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        conn.execute_batch(&format!("BEGIN; {sql} PRAGMA user_version = {}; COMMIT;", i + 1))?;
    }
    Ok(())
}

/// Categorías iniciales: (id, padre, nombre, tipo, color, icono)
const CATEGORIES: &[(&str, Option<&str>, &str, &str, &str, &str)] = &[
    ("vivienda", None, "Vivienda", "expense", "#6C8EF5", "home"),
    ("alquiler", Some("vivienda"), "Alquiler / hipoteca", "expense", "#6C8EF5", "key"),
    ("suministros", Some("vivienda"), "Luz, agua y gas", "expense", "#6C8EF5", "zap"),
    ("internet", Some("vivienda"), "Internet y móvil", "expense", "#6C8EF5", "wifi"),
    ("alimentacion", None, "Alimentación", "expense", "#F5A524", "shopping-cart"),
    ("supermercado", Some("alimentacion"), "Supermercado", "expense", "#F5A524", "shopping-basket"),
    ("restaurantes", Some("alimentacion"), "Restaurantes", "expense", "#F5A524", "utensils"),
    ("transporte", None, "Transporte", "expense", "#2BB3A3", "car"),
    ("combustible", Some("transporte"), "Combustible", "expense", "#2BB3A3", "fuel"),
    ("transporte-publico", Some("transporte"), "Transporte público", "expense", "#2BB3A3", "train-front"),
    ("ocio", None, "Ocio", "expense", "#A78BFA", "gamepad-2"),
    ("suscripciones", Some("ocio"), "Suscripciones", "expense", "#A78BFA", "repeat"),
    ("viajes", Some("ocio"), "Viajes", "expense", "#A78BFA", "plane"),
    ("compras", None, "Compras", "expense", "#F472B6", "shopping-bag"),
    ("tecnologia", Some("compras"), "Tecnología", "expense", "#F472B6", "cpu"),
    ("ropa", Some("compras"), "Ropa", "expense", "#F472B6", "shirt"),
    ("salud", None, "Salud y deporte", "expense", "#FB923C", "heart-pulse"),
    ("autonomo", None, "Autónomo", "expense", "#38BDF8", "briefcase"),
    ("cuota-reta", Some("autonomo"), "Cuota de autónomo", "expense", "#38BDF8", "landmark"),
    ("impuestos", Some("autonomo"), "Impuestos (IVA/IRPF)", "expense", "#38BDF8", "receipt"),
    ("gastos-negocio", Some("autonomo"), "Gastos del negocio", "expense", "#38BDF8", "package"),
    ("otros-gastos", None, "Otros gastos", "expense", "#94A3B8", "circle-ellipsis"),
    ("nomina", None, "Nómina", "income", "#15803D", "wallet"),
    ("facturacion", None, "Facturación", "income", "#15803D", "file-text"),
    ("otros-ingresos", None, "Otros ingresos", "income", "#15803D", "plus-circle"),
    ("transferencia", None, "Transferencia interna", "transfer", "#94A3B8", "arrow-left-right"),
];

pub fn seed(conn: &Connection) -> rusqlite::Result<()> {
    let mut st = conn.prepare(
        "INSERT OR IGNORE INTO categories(id,parent_id,name,kind,color,icon,sort,is_system) VALUES(?,?,?,?,?,?,?,1)",
    )?;
    for (i, (id, parent, name, kind, color, icon)) in CATEGORIES.iter().enumerate() {
        st.execute(rusqlite::params![id, parent, name, kind, color, icon, i as i64])?;
    }
    Ok(())
}
