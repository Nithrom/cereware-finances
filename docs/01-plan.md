# CereWare Finances — Plan de producto v1

## Visión
App local de finanzas personales y de autónomo para Windows (Android después). Lee tus cuentas bancarias vía PSD2 (solo lectura, sin datos de tarjeta), agrupa y analiza gastos, gestiona ahorro y prevé el futuro. Producto distribuible de la marca CereWare.

## Stack
- **Tauri 2** (Rust backend + frontend web: React/TS + Vite) → Windows ahora, Android con el mismo código.
- **SQLite cifrado (SQLCipher)** local. Clave derivada de PIN/contraseña (Argon2), opcional Windows Hello.
- Sin servidor propio para datos del usuario: todo vive en el equipo.

## Conexión bancaria
- **Proveedor PSD2 principal: Enable Banking** (cobertura amplia en ES/UE: BBVA, Openbank, Revolut, Santander, CaixaBank, ING, Sabadell…). Capa `BankProvider` abstracta para añadir otros (Tink, Salt Edge, Yapily).
- GoCardless Bank Account Data descartado: registros nuevos cerrados.
- **Modelo para producto distribuido** (punto crítico):
  - Fase 1 — *Trae tu propia clave*: cada usuario registra su app gratuita en Enable Banking (modo cuentas propias) y pega sus credenciales. Coste 0, sin backend.
  - Fase 2 — *CereWare Connect*: contrato comercial con agregador + micro-backend que solo firma peticiones (no almacena movimientos). Posible plan de pago.
- **Importación de extractos** siempre disponible: CSV, XLSX, OFX/QIF, Norma 43, con plantillas por banco y detección de duplicados.
- Consentimiento PSD2 de 180 días: aviso de renovación antes de caducar. Sincronización manual + automática al abrir (límite ~4 llamadas/día por cuenta).

## Funciones v1
1. **Panel**: saldo consolidado, gasto mes vs media, evolución de patrimonio, próximos cargos.
2. **Movimientos**: búsqueda, filtros, notas, etiquetas, dividir movimiento, transferencias internas detectadas (no cuentan como gasto).
3. **Categorías y reglas**: árbol de categorías, reglas automáticas (texto/importe/cuenta), aprendizaje de correcciones.
4. **Presupuestos**: por categoría/mes, arrastre de sobrante opcional, avisos al 80/100 %.
5. **Suscripciones y recibos**: detección automática de periodicidad, alertas de subida de precio, coste anual.
6. **Huchas**: objetivo + fecha, aportación sugerida, vinculables a cuenta real o virtuales.
7. **Previsión**: movimientos programados/recurrentes, proyección de saldo 30/60/90 días, alerta de saldo bajo.
8. **Modo autónomo**: separar personal/profesional por cuenta o movimiento; resumen de ingresos, gastos, beneficio, clientes y gastos por categoría. Los impuestos (303/130) se gestionan en CereWare Invoicer.
9. **Informes**: por categoría, periodo y comparativa interanual; exportar PDF/XLSX/CSV.
10. **Privacidad y datos**: bloqueo por PIN/Hello, autobloqueo, backup cifrado (.cwbak), modo ocultar importes.

## Futuro (v2+)
App Android con sincronización cifrada extremo a extremo entre dispositivos, inversiones/cripto, multi-divisa avanzada, OCR de tickets, perfiles compartidos (pareja), CereWare Connect.

## Fases de trabajo
1. ✅ Planificación (este documento)
2. Marca: nombre/logo, paleta, tipografía, estética, mockups de pantallas clave
3. Código: esqueleto Tauri + BD cifrada → importación extractos → movimientos/categorías → panel/presupuestos → Enable Banking → suscripciones/huchas/previsión → modo autónomo → informes → instalador (MSI/NSIS) + autoactualización
