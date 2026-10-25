# Servidor de sincronización de CereWare Finances

Un único fichero PHP (8.1+) con SQLite. Guarda solo paquetes **cifrados de extremo a extremo**:
el servidor no puede leer tus movimientos; la clave está únicamente en tus dispositivos.

## Instalación (hosting con DirectAdmin / cPanel / Arsys)
1. Crea un subdominio con HTTPS, p. ej. `https://sync.tudominio.es`.
2. Sube `sync.php` y `.htaccess` a su carpeta pública.
3. Comprueba que PHP tiene `pdo_sqlite` (casi siempre activo).
4. (Recomendado) Crea `config.php` para guardar la base de datos fuera de la carpeta pública:
   ```php
   <?php return ['db' => __DIR__ . '/../cwf-data/sync.sqlite'];
   ```
5. En la app de Windows: **Ajustes → Sincronización → Activar** con `https://sync.tudominio.es/sync.php`.
6. En el móvil: escanea el QR que muestra el PC.
7. Cuando tengas tus dispositivos emparejados, añade `'allow_new_spaces' => false` a `config.php`.

## Probar en local
```
php -S 0.0.0.0:8787 -t server
```
y usa `http://IP-DEL-PC:8787/sync.php` (solo para pruebas; en producción usa HTTPS).
