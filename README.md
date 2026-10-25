# CereWare Finances

App local de finanzas personales y de autónomo para Windows (Android después).
Tauri 2 + React/TypeScript. Conexión bancaria PSD2 de solo lectura (sin datos de tarjeta).

## Privacidad
Todos los datos viven en tu equipo en un único fichero cifrado (`cereware-finances.cwf`
en `%APPDATA%\com.cereware.finances`). La base SQLite se descifra solo en memoria:
clave derivada de tu contraseña con Argon2id (64 MiB, 3 pasadas) y cifrado AES-256-GCM.

## Desarrollo en Windows
Requisitos (una vez):
1. [Node.js 20+](https://nodejs.org)
2. [Rust](https://rustup.rs) (toolchain MSVC)
3. Visual Studio Build Tools con "Desarrollo para el escritorio con C++"
4. WebView2 (ya incluido en Windows 11)

```
npm install
npm run tauri dev      # modo desarrollo
npm run tauri build    # instalador (MSI/NSIS) en src-tauri/target/release/bundle
```

Tests del backend: `cd src-tauri && cargo test`

## Funciones
- Importación de extractos: CSV, Excel, OFX y Norma 43 (Openbank, BBVA, Revolut y genéricos)
- Conexión bancaria PSD2 vía Enable Banking (beta, con credenciales propias)
- Movimientos con categorización automática y reglas aprendidas
- Panel, presupuestos, suscripciones, huchas, previsión, modo autónomo (303/130), informes
- Copias de seguridad cifradas, cambio de contraseña, bloqueo automático

## Estructura
- `src/` interfaz (React): `screens/`, `components/`, `lib/`, `styles/tokens.css`
- `src-tauri/src/` backend (Rust): `vault.rs` (cifrado), `schema.rs` (BD), `import.rs` (extractos), `ledger.rs` (movimientos y reglas), `analytics.rs`, `planning.rs` (recurrentes, huchas, previsión), `fiscal.rs`, `bank.rs` (Enable Banking), `commands.rs`
- `docs/` plan, propuestas de logo y maqueta · `brand/` logo final y tokens
