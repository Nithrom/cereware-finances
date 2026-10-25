import { useState, type FormEvent } from "react";
import { ShieldCheck } from "lucide-react";
import { LogoMark } from "../components/Logo";
import { api } from "../lib/api";

/** Crear o desbloquear el almacén cifrado. */
export function LockScreen({ exists, onUnlocked }: { exists: boolean; onUnlocked: () => void }) {
  const [pw, setPw] = useState("");
  const [pw2, setPw2] = useState("");
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit(e: FormEvent) {
    e.preventDefault();
    setErr(null);
    if (!exists && pw !== pw2) return setErr("Las contraseñas no coinciden");
    setBusy(true);
    try {
      await (exists ? api.vaultUnlock(pw) : api.vaultCreate(pw));
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
        {err && <div className="form-error">{err}</div>}
        <button className="btn p block" disabled={busy || pw.length === 0}>
          {busy ? (exists ? "Desbloqueando…" : "Creando…") : exists ? "Desbloquear" : "Crear almacén"}
        </button>
        <div className="lock-foot"><ShieldCheck size={14} /> Cifrado AES-256 · todo en local</div>
      </form>
    </div>
  );
}
