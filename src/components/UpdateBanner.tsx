import { useState } from "react";
import { Download, X } from "lucide-react";
import type { Update } from "../lib/update";
import { errMsg } from "../lib/api";

/** Aviso discreto de versión nueva (App lo comprueba una vez al desbloquear). */
export function UpdateBanner({ update: u, onClose }: { update: Update | null; onClose: () => void }) {
  const [busy, setBusy] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  if (!u) return null;
  async function go() {
    setErr(null); setBusy("Descargando…");
    try { await u!.install((p) => setBusy(p === null ? "Descargando…" : `Descargando ${p} %`)); setBusy("Abriendo instalador…"); }
    catch (e) { setErr(errMsg(e)); setBusy(null); }
  }
  return (
    <div className="banner info update-banner">
      <Download size={16} />
      <span className="grow"><b>CereWare Finances {u.version} disponible.</b> {err ?? u.notes ?? ""}</span>
      <button className="btn p sm" onClick={go} disabled={!!busy}>{busy ?? "Actualizar"}</button>
      {!busy && <button className="btn g icon" aria-label="Más tarde" onClick={onClose}><X size={14} /></button>}
    </div>
  );
}
