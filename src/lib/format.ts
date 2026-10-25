const eur = new Intl.NumberFormat("es-ES", { style: "currency", currency: "EUR" });

export const money = (cents: number, opts: { sign?: boolean } = {}) => {
  const s = eur.format(cents / 100);
  return opts.sign && cents > 0 ? `+${s}` : s.replace("-", "−");
};

export const today = () =>
  new Intl.DateTimeFormat("es-ES", { weekday: "long", day: "numeric", month: "long" }).format(new Date());

export const greeting = () => {
  const h = new Date().getHours();
  return h < 13 ? "Buenos días" : h < 21 ? "Buenas tardes" : "Buenas noches";
};
