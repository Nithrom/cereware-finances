<?php
/**
 * CereWare Finances — servidor de sincronización.
 *
 * Solo almacena paquetes cifrados de extremo a extremo (AES-256-GCM). No puede leer
 * ningún dato: la clave vive únicamente en tus dispositivos.
 *
 *   POST sync.php?a=push   cabeceras X-Space, X-Token · cuerpo {"device","blob"} → {"seq"}
 *   GET  sync.php?a=pull&since=N  cabeceras X-Space, X-Token → {"items":[{seq,device,blob}],"more"}
 *
 * Cada "espacio" (un conjunto de dispositivos emparejados) se crea en su primer uso y queda
 * protegido por su token (se guarda solo su hash).
 */

declare(strict_types=1);

$config = [
    // Ruta del fichero SQLite. Mejor fuera de la carpeta pública (p. ej. __DIR__ . '/../cwf-data/sync.sqlite').
    'db' => __DIR__ . '/data/sync.sqlite',
    'max_blob' => 8 * 1024 * 1024,   // 8 MB por paquete
    'page' => 200,                     // paquetes por petición de descarga
    'allow_new_spaces' => true,        // pon false tras emparejar tus dispositivos
];
if (is_file(__DIR__ . '/config.php')) {
    $config = array_merge($config, require __DIR__ . '/config.php');
}

header('Content-Type: application/json; charset=utf-8');
header('Cache-Control: no-store');

function fail(int $code, string $msg): never {
    http_response_code($code);
    echo json_encode(['error' => $msg]);
    exit;
}

$space = $_SERVER['HTTP_X_SPACE'] ?? '';
$token = $_SERVER['HTTP_X_TOKEN'] ?? '';
if (!preg_match('/^[a-f0-9]{32}$/', $space) || !preg_match('/^[a-f0-9]{48}$/', $token)) {
    fail(401, 'credenciales no válidas');
}

$dir = dirname($config['db']);
if (!is_dir($dir) && !mkdir($dir, 0700, true)) fail(500, 'no se puede crear la carpeta de datos');
if (!is_file($dir . '/.htaccess')) @file_put_contents($dir . '/.htaccess', "Require all denied\nDeny from all\n");

try {
    $db = new PDO('sqlite:' . $config['db'], null, null, [PDO::ATTR_ERRMODE => PDO::ERRMODE_EXCEPTION]);
    $db->exec('PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;');
    $db->exec('CREATE TABLE IF NOT EXISTS spaces (id TEXT PRIMARY KEY, token_hash TEXT NOT NULL, created TEXT NOT NULL)');
    $db->exec('CREATE TABLE IF NOT EXISTS blobs (seq INTEGER PRIMARY KEY AUTOINCREMENT, space TEXT NOT NULL, device TEXT NOT NULL, blob TEXT NOT NULL, created TEXT NOT NULL)');
    $db->exec('CREATE INDEX IF NOT EXISTS idx_blobs_space ON blobs(space, seq)');
} catch (Throwable $e) {
    fail(500, 'base de datos no disponible (¿pdo_sqlite activado?)');
}

$hash = hash('sha256', $token);
$st = $db->prepare('SELECT token_hash FROM spaces WHERE id=?');
$st->execute([$space]);
$known = $st->fetchColumn();
if ($known === false) {
    if (!$config['allow_new_spaces']) fail(403, 'este servidor no admite espacios nuevos');
    $db->prepare('INSERT INTO spaces(id, token_hash, created) VALUES(?,?,?)')->execute([$space, $hash, gmdate('c')]);
} elseif (!hash_equals($known, $hash)) {
    fail(403, 'token incorrecto');
}

$action = $_GET['a'] ?? '';
if ($action === 'push' && $_SERVER['REQUEST_METHOD'] === 'POST') {
    $raw = file_get_contents('php://input', false, null, 0, $config['max_blob'] + 1024);
    if ($raw === false || strlen($raw) > $config['max_blob']) fail(413, 'paquete demasiado grande');
    $in = json_decode($raw, true);
    $device = is_array($in) ? (string)($in['device'] ?? '') : '';
    $blob = is_array($in) ? (string)($in['blob'] ?? '') : '';
    if (!preg_match('/^[a-f0-9]{16}$/', $device) || !preg_match('/^[A-Za-z0-9_\-]+$/', $blob)) fail(400, 'paquete no válido');
    $db->prepare('INSERT INTO blobs(space, device, blob, created) VALUES(?,?,?,?)')->execute([$space, $device, $blob, gmdate('c')]);
    echo json_encode(['seq' => (int)$db->lastInsertId()]);
} elseif ($action === 'pull') {
    $since = max(0, (int)($_GET['since'] ?? 0));
    $limit = (int)$config['page'];
    $st = $db->prepare('SELECT seq, device, blob FROM blobs WHERE space=? AND seq>? ORDER BY seq LIMIT ' . ($limit + 1));
    $st->execute([$space, $since]);
    $rows = $st->fetchAll(PDO::FETCH_ASSOC);
    $more = count($rows) > $limit;
    $rows = array_slice($rows, 0, $limit);
    foreach ($rows as &$r) $r['seq'] = (int)$r['seq'];
    echo json_encode(['items' => $rows, 'more' => $more]);
} else {
    fail(400, 'acción no válida');
}
