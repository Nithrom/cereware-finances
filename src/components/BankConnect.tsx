import { useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Search, ExternalLink, CheckCircle2, Loader2, KeyRound, Landmark } from "lucide-react";
import { bankApi, errMsg, type Aspsp, type BankConfigView } from "../lib/api";
import { Field, Modal } from "./ui";

const COUNTRIES = [["ES", "España"], ["PT", "Portugal"], ["FR", "Francia"], ["DE", "Alemania"], ["IT", "Italia"], ["LT", "Lituania (Revolut)"], ["NL", "Países Bajos"], ["BE", "Bélgica"], ["IE", "Irlanda"]];
const FAV = ["openbank", "bbva", "revolut", "santander", "caixabank", "ing", "sabadell", "bankinter", "unicaja", "kutxabank", "abanca", "ibercaja", "cajamar"];

type Step = "config" | "pick" | "auth" | "done";

export function BankConnect({ onClose, onDone }: { onClose: () => void; onDone: () => void }) {
  const [cfg, setCfg] = useState<BankConfigView | null | undefined>(undefined);
  const [step, setStep] = useState<Step>("pick");
  const [country, setCountry] = useState("ES");
  const [banks, setBanks] = useState<Aspsp[]>([]);
  const [q, setQ] = useState("");
  const [bank, setBank] = useState<Aspsp | null>(null);
  const [auto, setAuto] = useState(false);
  const [pasted, setPasted] = useState("");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [result, setResult] = useState<{ accounts: string[]; imported: number } | null>(null);

  useEffect(() => { bankApi.config().then((c) => { setCfg(c); if (!c || !c.has_key) setStep("config"); }); }, []);
  useEffect(() => {
    if (step !== "pick" || !cfg?.has_key) return;
    setBusy(true); setErr(null);
    bankApi.aspsps(country).then(setBanks).catch((e) => setErr(errMsg(e))).finally(() => setBusy(false));
  }, [step, country, cfg]);

  // Captura automática (redirect a localhost)
  useEffect(() => {
    if (step !== "auth" || !auto) return;
    const t = setInterval(async () => { const c = await bankApi.poll(); if (c) { clearInterval(t); finish(c); } }, 1500);
    return () => clearInterval(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [step, auto]);

  const list = useMemo(() => {
    const n = q.trim().toLowerCase();
    const f = banks.filter((b) => !n || b.name.toLowerCase().includes(n));
    const rank = (b: Aspsp) => { const i = FAV.findIndex((x) => b.name.toLowerCase().includes(x)); return i < 0 ? 99 : i; };
    return [...f].sort((a, b) => rank(a) - rank(b) || a.name.localeCompare(b.name));
  }, [banks, q]);

  async function start(b: Aspsp) {
    setBank(b); setErr(null); setBusy(true);
    try {
      const r = await bankApi.start(b.name, b.country, Math.min(b.max_consent_days ?? 180, 180));
      setAuto(r.auto_capture);
      setStep("auth");
      await openUrl(r.url);
    } catch (e) { setErr(errMsg(e)); } finally { setBusy(false); }
  }
  async function finish(code: string) {
    setBusy(true); setErr(null);
    try { setResult(await bankApi.finish(code, bank?.name ?? "")); setStep("done"); }
    catch (e) { setErr(errMsg(e)); } finally { setBusy(false); }
  }

  if (cfg === undefined) return null;
  if (step === "config") return <BankConfigForm cfg={cfg} onClose={onClose} onSaved={() => bankApi.config().then((c) => { setCfg(c); setStep("pick"); })} />;

  return (
    <Modal title={step === "done" ? "Banco conectado" : "Conectar banco"} onClose={step === "done" ? onDone : onClose} width={640}
      footer={step === "done" ? <button className="btn p" onClick={onDone}>Listo</button>
        : step === "auth" ? <><button className="btn g" onClick={() => setStep("pick")}>Atrás</button><span className="grow" />
          {!auto && <button className="btn p" disabled={busy || !pasted.trim()} onClick={() => finish(pasted)}>{busy ? "Conectando…" : "Terminar conexión"}</button>}</>
        : <><button className="link" onClick={() => setStep("config")}>Credenciales de Enable Banking</button><span className="grow" /><button className="btn g" onClick={onClose}>Cancelar</button></>}>
      {err && <div className="banner warn" style={{ marginBottom: 12 }}>{err}</div>}
      {step === "pick" && (
        <div className="stack">
          <div className="filters">
            <div className="search"><Search size={16} /><input autoFocus placeholder="Busca tu banco" value={q} onChange={(e) => setQ(e.target.value)} /></div>
            <select className="input sm" value={country} onChange={(e) => setCountry(e.target.value)}>
              {COUNTRIES.map(([c, n]) => <option key={c} value={c}>{n}</option>)}
            </select>
          </div>
          <div className="bank-list">
            {busy && <div className="muted row-gap"><Loader2 size={16} className="spin" />Cargando bancos…</div>}
            {list.map((b) => (
              <button key={b.name + b.country} className="bank-item" onClick={() => start(b)} disabled={busy}>
                {b.logo ? <img src={b.logo} alt="" /> : <span className="bank-ph"><Landmark size={16} /></span>}
                <span className="grow">{b.name}</span>
                <ExternalLink size={14} className="muted" />
              </button>
            ))}
            {!busy && list.length === 0 && <div className="muted small">No hay bancos que coincidan.</div>}
          </div>
          <div className="muted small">Acceso de solo lectura (PSD2). Nunca vemos tus claves: inicias sesión en la web de tu banco.</div>
        </div>
      )}
      {step === "auth" && (
        <div className="stack">
          <div className="auth-box">
            <ExternalLink size={22} />
            <div><b>Autoriza el acceso en tu navegador</b><div className="muted small">Se ha abierto la web de {bank?.name}. Inicia sesión y acepta compartir tus cuentas (solo lectura).</div></div>
          </div>
          {auto ? (
            <div className="row-gap muted"><Loader2 size={16} className="spin" />Esperando la autorización… volverás aquí automáticamente.</div>
          ) : (
            <Field label="Cuando termines, copia la dirección completa de la barra del navegador y pégala aquí"
              hint={cfg?.redirect_url.startsWith("https://localhost") ? "El navegador mostrará un error de conexión al volver: es normal. Lo que importa es la dirección, que contiene ?code=…" : "Empieza por tu URL de vuelta y contiene ?code=…"}>
              <textarea className="input" rows={3} value={pasted} onChange={(e) => setPasted(e.target.value)} placeholder={`${cfg?.redirect_url ?? "https://…"}?state=…&code=…`} />
            </Field>
          )}
        </div>
      )}
      {step === "done" && result && (
        <div className="result">
          <CheckCircle2 size={40} className="inc" />
          <div className="center"><b>{result.accounts.length} cuentas vinculadas</b> · {result.imported} movimientos importados</div>
          <div className="muted small center">{result.accounts.join(" · ")}</div>
        </div>
      )}
    </Modal>
  );
}

export function BankConfigForm({ cfg, onClose, onSaved }: { cfg: BankConfigView | null; onClose: () => void; onSaved: () => void }) {
  const [appId, setAppId] = useState(cfg?.app_id ?? "");
  const [redirect, setRedirect] = useState(cfg?.redirect_url ?? "https://localhost:8765/callback");
  const [keyPath, setKeyPath] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  async function pickKey() {
    const f = await open({ multiple: false, filters: [{ name: "Clave privada", extensions: ["pem", "key", "txt"] }] });
    if (typeof f === "string") setKeyPath(f);
  }
  async function save() {
    if (!appId.trim()) return setErr("Indica el ID de la aplicación");
    if (!keyPath && !cfg?.has_key) return setErr("Selecciona el fichero de la clave privada");
    try { await bankApi.configSet(appId, keyPath, redirect); onSaved(); } catch (e) { setErr(errMsg(e)); }
  }
  return (
    <Modal title="Credenciales de Enable Banking" onClose={onClose} width={620}
      footer={<><button className="btn g" onClick={onClose}>Cancelar</button><button className="btn p" onClick={save}>Guardar</button></>}>
      <div className="stack">
        <div className="banner info"><KeyRound size={16} /><span>CereWare Finances usa <b>Enable Banking</b> para leer tus cuentas (PSD2). Crea gratis una aplicación con tus propias cuentas y pega aquí sus datos. Se guardan cifrados en tu almacén.</span></div>
        <ol className="steps small">
          <li>Regístrate en <button className="link" onClick={() => openUrl("https://enablebanking.com/sign-in/")}>enablebanking.com</button> y crea una aplicación en el panel (entorno <b>Production</b>).</li>
          <li>Añade como URL de redirección: <code>{redirect || "https://localhost:8765/callback"}</code></li>
          <li>Descarga la clave privada (.pem) que genera y vincula tus cuentas propias en el panel para activar la app.</li>
          <li>Copia el <b>Application ID</b> y selecciona aquí el fichero .pem.</li>
        </ol>
        <Field label="Application ID"><input className="input" value={appId} onChange={(e) => setAppId(e.target.value)} placeholder="aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee" /></Field>
        <Field label="Clave privada (.pem)">
          <div className="row-gap"><button className="btn g" onClick={pickKey}><KeyRound size={16} />{keyPath ? "Cambiar fichero" : cfg?.has_key ? "Sustituir clave guardada" : "Seleccionar fichero"}</button>
            <span className="muted small ellipsis" style={{ maxWidth: 300 }}>{keyPath ? keyPath.split(/[\\/]/).pop() : cfg?.has_key ? "Clave guardada ✓" : ""}</span></div>
        </Field>
        <Field label="URL de redirección" hint="Debe coincidir exactamente con la registrada en Enable Banking (solo admite https). Al volver del banco tendrás que copiar la dirección del navegador y pegarla en la app.">
          <input className="input" value={redirect} onChange={(e) => setRedirect(e.target.value)} />
        </Field>
        {err && <div className="form-error">{err}</div>}
      </div>
    </Modal>
  );
}
