import { useEffect } from "react";
import { syncApi } from "./api";

/** Sincroniza (sin solapar ejecuciones) y avisa con el evento "cwf-sync". */
let running: Promise<void> | null = null;
export function syncNow(): Promise<void> {
  if (running) return running;
  running = (async () => {
    window.dispatchEvent(new CustomEvent("cwf-sync", { detail: { state: "running" } }));
    try {
      const st = await syncApi.status();
      if (!st.enabled) return;
      const r = await syncApi.now();
      window.dispatchEvent(new CustomEvent("cwf-sync", { detail: { state: "ok", ...r } }));
    } catch (e) {
      window.dispatchEvent(new CustomEvent("cwf-sync", { detail: { state: "error", error: String(e) } }));
    } finally {
      running = null;
    }
  })();
  return running;
}

/** Al desbloquear, cada 3 minutos y al volver a la app. */
export function useAutoSync(active: boolean) {
  useEffect(() => {
    if (!active) return;
    syncNow();
    const t = window.setInterval(syncNow, 3 * 60_000);
    const vis = () => document.visibilityState === "visible" && syncNow();
    document.addEventListener("visibilitychange", vis);
    return () => { window.clearInterval(t); document.removeEventListener("visibilitychange", vis); };
  }, [active]);
}
