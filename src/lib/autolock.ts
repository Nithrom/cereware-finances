import { useEffect } from "react";

const KEY = "cwf-autolock";

export const getAutolock = () => { try { return Number(localStorage.getItem(KEY) ?? "15"); } catch { return 15; } };
export const setAutolock = (m: number) => {
  try { localStorage.setItem(KEY, String(m)); } catch { /* */ }
  window.dispatchEvent(new Event("cwf-autolock"));
};

/** Llama a `onLock` tras N minutos sin actividad (ratón, teclado). */
export function useAutolock(active: boolean, onLock: () => void) {
  useEffect(() => {
    if (!active) return;
    let timer: number | undefined;
    const reset = () => {
      window.clearTimeout(timer);
      const m = getAutolock();
      if (m > 0) timer = window.setTimeout(onLock, m * 60_000);
    };
    const evs = ["mousemove", "keydown", "mousedown", "wheel", "cwf-autolock"];
    evs.forEach((e) => window.addEventListener(e, reset, { passive: true }));
    reset();
    return () => { window.clearTimeout(timer); evs.forEach((e) => window.removeEventListener(e, reset)); };
  }, [active, onLock]);
}
