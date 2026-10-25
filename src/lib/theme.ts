import { useEffect, useState } from "react";

export type ThemePref = "system" | "light" | "dark" | "oled";
const KEY = "cwf-theme";

const mq = () => window.matchMedia("(prefers-color-scheme: dark)");

function apply(pref: ThemePref) {
  const t = pref === "system" ? (mq().matches ? "dark" : "light") : pref;
  document.documentElement.dataset.theme = t;
}

function read(): ThemePref {
  try { return (localStorage.getItem(KEY) as ThemePref) || "system"; } catch { return "system"; }
}

/** El tema se guarda en localStorage para aplicarlo antes de desbloquear el almacén. */
export function useTheme() {
  const [pref, setPref] = useState<ThemePref>(read);
  useEffect(() => {
    apply(pref);
    try { localStorage.setItem(KEY, pref); } catch { /* sin almacenamiento */ }
    if (pref !== "system") return;
    const m = mq();
    const on = () => apply("system");
    m.addEventListener("change", on);
    return () => m.removeEventListener("change", on);
  }, [pref]);
  return [pref, setPref] as const;
}
