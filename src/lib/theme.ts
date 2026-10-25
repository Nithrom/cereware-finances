import { useEffect, useState } from "react";

export type ThemePref = "system" | "light" | "dark" | "oled";
const KEY = "cwf-theme";

const mq = () => window.matchMedia("(prefers-color-scheme: dark)");

function apply(pref: ThemePref) {
  const t = pref === "system" ? (mq().matches ? "dark" : "light") : pref;
  document.documentElement.dataset.theme = t;
  // Barra de estado del móvil con el color de la cabecera del tema
  const color = getComputedStyle(document.documentElement).getPropertyValue("--surface").trim();
  document.querySelectorAll('meta[name="theme-color"]').forEach((m) => m.setAttribute("content", color || "#FFFFFF"));
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
