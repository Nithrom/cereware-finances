import { useEffect, useState, type FormEvent } from "react";
import { ShieldCheck, Fingerprint } from "lucide-react";
import { bioEnable, bioEnabled, bioUnlock } from "../lib/biometric";
import { LogoMark } from "../components/Logo";
import { api } from "../lib/api";
import { JoinModal } from "../components/SyncPanel";

/** Crear o desbloquear el almacén cifrado. */
export function LockScreen({ exists, onUnlocked, mobile = false }: { exists: boolean; onUnlocked: () => void; mobile?: boolean }) {
  const [pw, setPw] = useState("");
  const [pw2, setPw2] = useState("");
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [join, setJoin] = useState(false);
  const [joining, setJoining] = useState(false);
  const [offerBio, setOfferBio] = useState(false);
  const canBio = mobile && exists && bioEnabled();

  async function tryBio() {
    setErr(null);
    setBusy(true);
    try { await bioUnlock(); onUnlocked(); }
    catch (e) { const m = String(e); if (!/cancel/i.test(m)) setErr(m.replace(/^Error: /, "")); setBusy(false); }
  }
  // Al abrir en el móvil, pedir la huella directamente
  useEffect(() => { if (canBio) tryBio(); }, []); // eslint-disable-line react-hooks/exhaustive-deps

  async function submit(e: FormEvent) {
    e.preventDefault();
    setErr(null);
    if (!exists && pw !== pw2) return setErr("Las contraseñas no coinciden");
    setBusy(true);
    try {
      await (exists ? api.vaultUnlock(pw) : api.vaultCreate(pw));
      if (!exists && join) { setJoining(true); return; }
      if (mobile && !bioEnabled()) { setOfferBio(true); return; }
      onUnlocked();
    } catch (e) {
      setErr(String(e));
      setBusy(false);
    }
  }

  return (
    <div className="lock">
      <form className="lock-card card" onSubmit={submit}>
        <LogoMark size={72} />
        <h1>{exists ? "Bienvenido de nuevo" : "Crea tu almacén"}</h1>
        <p className="muted">
          {exists
            ? "Introduce tu contraseña para desbloquear tus finanzas."
            : "Tus datos se guardan solo en este equipo, cifrados con esta contraseña. Si la pierdes no se pueden recuperar."}
        </p>
        <input className="input" type="password" autoFocus placeholder="Contraseña" value={pw}
          onChange={(e) => setPw(e.target.value)} disabled={busy} />
        {!exists && (
          <input className="input" type="password" placeholder="Repite la contraseña" value={pw2}
            onChange={(e) => setPw2(e.target.value)} disabled={busy} />
        )}
        {!exists && (
          <label className="check small"><input type="checkbox" checked={join} onChange={(e) => setJoin(e.target.checked)} />Ya uso CereWare Finances en otro dispositivo (unirme)</label>
        )}
        {err && <div className="form-error">{err}</div>}
        <button className="btn p block" disabled={busy || pw.length === 0}>
          {busy ? (exists ? "Desbloqueando…" : "Creando…") : exists ? "Desbloquear" : "Crear almacén"}
        </button>
        {canBio && <button type="button" className="btn g block" onClick={tryBio} disabled={busy}><Fingerprint size={18} />Usar huella o cara</button>}
        <div className="lock-foot"><ShieldCheck size={14} /> Cifrado AES-256 · todo en local</div>
      </form>
      {offerBio && (
        <div className="overlay"><div className="modal card" style={{ width: 420 }}>
          <div className="modal-body stack center-col" style={{ paddingTop: 24 }}>
            <Fingerprint size={44} className="exp" />
            <h2>¿Desbloquear con huella o cara?</h2>
            <p className="muted small center">Tu contraseña se guardará cifrada en el almacén seguro del móvil y solo se podrá usar con tu huella o tu cara.</p>
          </div>
          <footer className="modal-foot">
            <button className="btn g" onClick={onUnlocked}>Ahora no</button>
            <button className="btn p" onClick={async () => { try { await bioEnable(pw); } catch { /* cancelado */ } onUnlocked(); }}>Activar</button>
          </footer>
        </div></div>
      )}
      {joining && <JoinModal mobile={mobile} onClose={onUnlocked} onDone={onUnlocked} />}
    </div>
  );
}
