import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, Copy, X } from "lucide-react";
import { LogoMark } from "./Logo";

/** Barra de título propia (la ventana no tiene decoraciones del sistema). */
export function TitleBar() {
  const [max, setMax] = useState(false);
  useEffect(() => {
    let off: (() => void) | undefined;
    try {
      const w = getCurrentWindow();
      w.isMaximized().then(setMax).catch(() => {});
      w.onResized(() => w.isMaximized().then(setMax).catch(() => {})).then((u) => (off = u)).catch(() => {});
    } catch { /* fuera de Tauri */ }
    return () => off?.();
  }, []);
  const w = () => { try { return getCurrentWindow(); } catch { return null; } };
  return (
    <div className="titlebar" data-tauri-drag-region>
      <div className="tb-title" data-tauri-drag-region><LogoMark size={16} /><span data-tauri-drag-region>CereWare Finances</span></div>
      <div className="tb-btns">
        <button aria-label="Minimizar" onClick={() => w()?.minimize()}><Minus size={15} /></button>
        <button aria-label={max ? "Restaurar" : "Maximizar"} onClick={() => w()?.toggleMaximize()}>{max ? <Copy size={13} /> : <Square size={12} />}</button>
        <button aria-label="Cerrar" className="tb-close" onClick={() => w()?.close()}><X size={16} /></button>
      </div>
    </div>
  );
}
