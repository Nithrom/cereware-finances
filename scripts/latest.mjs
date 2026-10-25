// Prepara la carpeta release/ para subir a https://cerezuela-software.com/cwf/
// Uso: node scripts/latest.mjs "Notas de la versión"
import fs from "node:fs";
import path from "node:path";

const BASE = "https://cerezuela-software.com/cwf/";
const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/(\w:)/, "$1")), "..");
const conf = JSON.parse(fs.readFileSync(path.join(root, "src-tauri/tauri.conf.json"), "utf8"));
const version = conf.version;
const out = path.join(root, "release");
fs.mkdirSync(out, { recursive: true });

const nsis = path.join(root, "src-tauri/target/release/bundle/nsis");
const exe = fs.existsSync(nsis) && fs.readdirSync(nsis).find((f) => f.includes(version) && f.endsWith("-setup.exe"));
if (!exe || !fs.existsSync(path.join(nsis, exe + ".sig"))) {
  console.error(`No encuentro el instalador firmado de la versión ${version} en ${nsis}. ¿Has ejecutado scripts\\release-windows.ps1?`);
  process.exit(1);
}
const exeOut = `CereWare-Finances-${version}-setup.exe`;
fs.copyFileSync(path.join(nsis, exe), path.join(out, exeOut));
const apk = `CereWare-Finances-${version}.apk`;
const hasApk = fs.existsSync(path.join(out, apk));

const latest = {
  version,
  notes: process.argv[2] || `CereWare Finances ${version}`,
  pub_date: new Date().toISOString(),
  platforms: {
    "windows-x86_64": { signature: fs.readFileSync(path.join(nsis, exe + ".sig"), "utf8").trim(), url: BASE + exeOut },
  },
  ...(hasApk ? { android: { version, url: BASE + apk } } : {}),
};
fs.writeFileSync(path.join(out, "latest.json"), JSON.stringify(latest, null, 2));
console.log(`Listo en ${out}:\n  latest.json\n  ${exeOut}\n  ${hasApk ? apk : "(sin APK: copia " + apk + " aquí y vuelve a ejecutar para incluir Android)"}`);
console.log(`Súbelos a /home/<usuario>/domains/cerezuela-software.com/public_html/cwf/`);
