import { useEffect, useState } from "react";
import QRCode from "qrcode";
import { Cloud, RefreshCw, QrCode, ScanLine, Power, Copy } from "lucide-react";
import { errMsg, syncApi, type SyncStatus } from "../lib/api";
import { syncNow } from "../lib/sync";
import { Field, Modal } from "./ui";

const since = (iso: string | null) => {
  if (!iso) return "nunca";
  const m = Math.round((Date.now() - new Date(iso).getTime()) / 60000);
  return m < 1 ? "ahora mismo" : m < 60 ? `hace ${m} min` : m < 1440 ? `hace ${Math.round(m / 60)} h` : new Date(iso).toLocaleDateString("es-ES");
};

export function SyncSection({ onToast, mobile }: { onToast: (m: string) => void; mobile: boolean }) {
  const [st, setSt] = useState<SyncStatus | null>(null);
  const [server, setServer] = useState("");
  const [busy, setBusy] = useState(false);
  const [qr, setQr] = useState<string | null>(null);
  const [join, setJoin] = useState(false);
  const load = () => syncApi.status().then(setSt).catch(() => {});
  useEffect(() => {
    load();
    const h = () => load();
    window.addEventListener("cwf-sync", h);
    return () => window.removeEventListener("cwf-sync", h);
  }, []);

  async function enable() {
    setBusy(true);
    try { const code = await syncApi.enable(server.trim()); setQr(code); load(); onToast("Sincronización activada"); }
    catch (e) { onToast(errMsg(e)); } finally { setBusy(false); }
  }
  async function now() { setBusy(true); await syncNow(); setBusy(false); load(); }
  async function showQr() { try { setQr(await syncApi.code()); } catch (e) { onToast(errMsg(e)); } }
  async function disable() { await syncApi.disable(); load(); onToast("Sincronización desactivada en este dispositivo"); }

  if (!st) return null;
  return (
    <section className="card settings-sec">
      <h3><Cloud size={18} />Sincronización entre dispositivos</h3>
      {st.enabled ? (
        <>
          <div className="set-row">
            <div><b>Activada</b><div className="muted small">Última: {since(st.last_sync)} · {st.pending} cambios pendientes · {st.server}</div></div>
            <button className="btn g" onClick={now} disabled={busy}><RefreshCw size={16} className={busy ? "spin" : ""} />Sincronizar ahora</button>
          </div>
          <div className="set-row">
            <div><b>Añadir otro dispositivo</b><div className="muted small">Escanea el QR con la app del móvil (Ajustes → Sincronización → Unirse).</div></div>
            <button className="btn g" onClick={showQr}><QrCode size={16} />Mostrar QR</button>
          </div>
          <div className="set-row">
            <div><b>Desactivar en este dispositivo</b><div className="muted small">Los datos locales se conservan; deja de recibir y enviar cambios.</div></div>
            <button className="btn g" onClick={disable}><Power size={16} />Desactivar</button>
          </div>
        </>
      ) : (
        <>
          <div className="muted small">Tus dispositivos comparten los datos a través de tu propio servidor, cifrados de extremo a extremo: el servidor no puede leerlos.</div>
          <div className="set-row">
            <Field label="Activar en este dispositivo (el primero)" hint="Dirección de tu servidor, p. ej. https://sync.tudominio.es/sync.php">
              <div className="row-gap"><input className="input grow" value={server} onChange={(e) => setServer(e.target.value)} placeholder="https://…/sync.php" inputMode="url" autoCapitalize="none" autoCorrect="off" />
                <button className="btn p" onClick={enable} disabled={busy || server.trim().length < 10}>{busy ? "Subiendo datos…" : "Activar"}</button></div>
            </Field>
          </div>
          <div className="set-row">
            <div><b>Unirse a otro dispositivo</b><div className="muted small">Si ya usas CereWare Finances en otro equipo, descarga aquí sus datos. Los datos actuales de este dispositivo se sustituyen.</div></div>
            <button className="btn g" onClick={() => setJoin(true)}><ScanLine size={16} />Unirse</button>
          </div>
        </>
      )}
      {qr && <QrModal code={qr} onClose={() => setQr(null)} onToast={onToast} />}
      {join && <JoinModal mobile={mobile} onClose={() => setJoin(false)} onDone={(m) => { setJoin(false); load(); onToast(m); }} />}
    </section>
  );
}

function QrModal({ code, onClose, onToast }: { code: string; onClose: () => void; onToast: (m: string) => void }) {
  const [img, setImg] = useState<string | null>(null);
  useEffect(() => { QRCode.toDataURL(code, { margin: 1, width: 320, errorCorrectionLevel: "M" }).then(setImg); }, [code]);
  return (
    <Modal title="Emparejar otro dispositivo" onClose={onClose} width={440}
      footer={<><button className="btn g" onClick={() => { navigator.clipboard?.writeText(code); onToast("Código copiado"); }}><Copy size={16} />Copiar código</button><button className="btn p" onClick={onClose}>Hecho</button></>}>
      <div className="stack center-col">
        {img && <img src={img} alt="Código QR de emparejamiento" className="qr" />}
        <div className="muted small center">En el móvil: Ajustes → Sincronización → Unirse → Escanear QR.<br />Este código da acceso a tus datos: no lo compartas.</div>
      </div>
    </Modal>
  );
}

export function JoinModal({ mobile, onClose, onDone }: { mobile: boolean; onClose: () => void; onDone: (msg: string) => void }) {
  const [code, setCode] = useState("");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  async function go(c = code) {
    setBusy(true); setErr(null);
    try { const r = await syncApi.join(c.trim()); onDone(`Dispositivo unido · ${r.pulled} datos descargados`); }
    catch (e) { setErr(errMsg(e)); setBusy(false); }
  }
  async function scanQr() {
    try {
      const { scan, Format, requestPermissions } = await import("@tauri-apps/plugin-barcode-scanner");
      await requestPermissions();
      const r = await scan({ windowed: false, formats: [Format.QRCode] });
      if (r?.content) { setCode(r.content); go(r.content); }
    } catch (e) { setErr(errMsg(e)); }
  }
  return (
    <Modal title="Unirse a otro dispositivo" onClose={onClose} width={480}
      footer={<><button className="btn g" onClick={onClose}>Cancelar</button><button className="btn p" disabled={busy || code.trim().length < 10} onClick={() => go()}>{busy ? "Descargando…" : "Unirse"}</button></>}>
      <div className="stack">
        {mobile && <button className="btn p block" onClick={scanQr} disabled={busy}><ScanLine size={18} />Escanear QR del PC</button>}
        <Field label={mobile ? "…o pega el código" : "Pega el código de emparejamiento"} hint="En el otro dispositivo: Ajustes → Sincronización → Mostrar QR → Copiar código.">
          <textarea className="input" rows={3} value={code} onChange={(e) => setCode(e.target.value)} placeholder="CWF1.…" autoCapitalize="none" autoCorrect="off" />
        </Field>
        {err && <div className="form-error">{err}</div>}
      </div>
    </Modal>
  );
}
