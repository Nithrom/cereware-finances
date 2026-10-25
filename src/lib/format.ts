// useGrouping "always": en es-ES Intl no agrupa los números de 4 cifras (1234,00 €)
const eur = new Intl.NumberFormat("es-ES", { style: "currency", currency: "EUR", useGrouping: "always" } as unknown as Intl.NumberFormatOptions);

export const money = (cents: number, opts: { sign?: boolean } = {}) => {
  const s = eur.format(cents / 100);
  return opts.sign && cents > 0 ? `+${s}` : s.replace("-", "−");
};

/** "12,50" | "-3.000,00" | "12.5" → céntimos */
export const parseMoney = (s: string): number | null => {
  const t = s.trim().replace(/\s|€/g, "");
  if (!t) return null;
  const norm = t.includes(",") ? t.replace(/\./g, "").replace(",", ".") : t;
  const v = Number(norm);
  return Number.isFinite(v) ? Math.round(v * 100) : null;
};

export const today = () =>
  new Intl.DateTimeFormat("es-ES", { weekday: "long", day: "numeric", month: "long" }).format(new Date());

export const isoToday = () => new Date().toISOString().slice(0, 10);

const short = new Intl.DateTimeFormat("es-ES", { day: "numeric", month: "short" });
const long = new Intl.DateTimeFormat("es-ES", { day: "numeric", month: "short", year: "numeric" });
export const fmtDate = (iso: string) => {
  const d = new Date(iso + "T12:00:00");
  return (d.getFullYear() === new Date().getFullYear() ? short : long).format(d).replace(".", "");
};
export const fmtMonth = (ym: string) =>
  new Intl.DateTimeFormat("es-ES", { month: "long", year: "numeric" }).format(new Date(ym + "-15T12:00:00"));
export const fmtMonthShort = (ym: string) =>
  new Intl.DateTimeFormat("es-ES", { month: "short" }).format(new Date(ym + "-15T12:00:00")).replace(".", "");

export const greeting = () => {
  const h = new Date().getHours();
  return h < 13 ? "Buenos días" : h < 21 ? "Buenas tardes" : "Buenas noches";
};

export const initials = (s: string) =>
  s.replace(/[^\p{L}\p{N} ]/gu, " ").trim().split(/\s+/).slice(0, 2).map((w) => w[0]?.toUpperCase() ?? "").join("") || "·";
