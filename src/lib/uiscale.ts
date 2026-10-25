/** Escala de la interfaz: "auto" según el ancho de pantalla, o un valor fijo (1 = 100 %). */
const KEY = "cwf-scale";
export type Scale = "auto" | number;

export const autoScale = () => {
  const w = window.screen?.width || window.innerWidth;
  return w >= 3400 ? 1.5 : w >= 2400 ? 1.2 : w >= 1900 ? 1.05 : 1;
};

export const getScale = (): Scale => {
  try { const v = localStorage.getItem(KEY); return !v || v === "auto" ? "auto" : Number(v) || "auto"; } catch { return "auto"; }
};

export function applyScale(s: Scale = getScale()) {
  const z = s === "auto" ? autoScale() : s;
  (document.documentElement.style as CSSStyleDeclaration & { zoom: string }).zoom = String(z);
}

export function setScale(s: Scale) {
  try { localStorage.setItem(KEY, String(s)); } catch { /* */ }
  applyScale(s);
  window.dispatchEvent(new Event("resize"));
}
