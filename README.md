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

## Android
Mismo código que Windows. Requisitos extra: Android Studio (SDK 34 + NDK 27) y
`rustup target add aarch64-linux-android`. Variables: `ANDROID_HOME` y `NDK_HOME`.

```
npm run tauri android dev               # en un móvil conectado por USB
npm run tauri android build -- --apk --target aarch64
```
El APK sin firmar queda en `src-tauri/gen/android/app/build/outputs/apk/universal/release/`.
Fírmalo con tu keystore (`apksigner sign --ks cwf-release.jks …`).

## Sincronización entre dispositivos
Servidor propio en `server/` (PHP + SQLite, ver `server/README.md`). Los datos viajan
cifrados de extremo a extremo; el servidor no puede leerlos.

## Funciones
- Importación de extractos: CSV, Excel, OFX y Norma 43 (Openbank, BBVA, Revolut y genéricos)
- Conexión bancaria PSD2 vía Enable Banking (beta, con credenciales propias)
- Movimientos con categorización automática y reglas aprendidas
- Panel, presupuestos, suscripciones, huchas, previsión, resumen de autónomo (impuestos en CereWare Invoicer), informes
- Copias de seguridad cifradas, cambio de contraseña, bloqueo automático

## Estructura
- `src/` interfaz (React): `screens/`, `components/`, `lib/`, `styles/tokens.css`
- `src-tauri/src/` backend (Rust): `vault.rs` (cifrado), `schema.rs` (BD), `import.rs` (extractos), `ledger.rs` (movimientos y reglas), `analytics.rs`, `planning.rs` (recurrentes, huchas, previsión), `bank.rs` (Enable Banking), `commands.rs`
- `docs/` plan, propuestas de logo y maqueta · `brand/` logo final y tokens

## Actualizaciones automáticas
La app busca versiones nuevas en `https://cerezuela-software.com/cwf/latest.json` al desbloquear
(y en Ajustes → Buscar actualizaciones). Windows se actualiza solo (instalador firmado); Android
descarga el APK y abre el instalador.

Publicar una versión:
1. Sube la versión en `src-tauri/tauri.conf.json`, `package.json` y `src-tauri/Cargo.toml`.
2. En Windows: `.\scripts\release-windows.ps1 "Novedades"` (usa `firmas\cwf-updater.key` y `firmas\firma-updater.txt`).
3. Copia el APK firmado como `release\CereWare-Finances-<versión>.apk` y vuelve a ejecutar `node scripts/latest.mjs "Novedades"`.
4. Sube el contenido de `release\` a `public_html/cwf/` del servidor.
