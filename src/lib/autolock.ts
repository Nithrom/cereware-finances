import { useEffect } from "react";

const KEY = "cwf-autolock";

export const getAutolock = () => { try { return Number(localStorage.getItem(KEY) ?? "15"); } catch { return 15; } };
export const setAutolock = (m: number) => {
  try { localStorage.setItem(KEY, String(m)); } catch { /* */ }
  window.dispatchEvent(new Event("cwf-autolock"));
};

/** Llama a `onLock` tras N minutos sin actividad (ratón, teclado, toques) o al volver tras N minutos fuera. */
export function useAutolock(active: boolean, onLock: () => void) {
  useEffect(() => {
    if (!active) return;
    let timer: number | undefined;
    const reset = () => {
      window.clearTimeout(timer);
      const m = getAutolock();
      if (m > 0) timer = window.setTimeout(onLock, m * 60_000);
    };
    const evs = ["mousemove", "keydown", "mousedown", "wheel", "touchstart", "pointerdown", "scroll", "cwf-autolock"];
    evs.forEach((e) => window.addEventListener(e, reset, { passive: true, capture: true }));
    // En el móvil los temporizadores se congelan en segundo plano: se mide el tiempo fuera
    let hiddenAt = 0;
    const vis = () => {
      if (document.visibilityState === "hidden") { hiddenAt = Date.now(); return; }
      const m = getAutolock();
      if (hiddenAt && m > 0 && Date.now() - hiddenAt > m * 60_000) onLock(); else reset();
    };
    document.addEventListener("visibilitychange", vis);
    reset();
    return () => {
      window.clearTimeout(timer);
      evs.forEach((e) => window.removeEventListener(e, reset, { capture: true }));
      document.removeEventListener("visibilitychange", vis);
    };
  }, [active, onLock]);
}
