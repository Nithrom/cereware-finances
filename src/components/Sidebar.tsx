import {
  LayoutDashboard, ArrowLeftRight, PieChart, Repeat, PiggyBank, TrendingUp,
  Briefcase, FileBarChart, Landmark, Settings, Lock, Grid2x2, type LucideIcon,
} from "lucide-react";
import { useState } from "react";
import { Logo } from "./Logo";
import type { ThemePref } from "../lib/theme";

export type Route =
  | "panel" | "movimientos" | "presupuestos" | "suscripciones" | "huchas"
  | "prevision" | "autonomo" | "informes" | "cuentas" | "ajustes";

export const NAV: { id: Route; label: string; icon: LucideIcon; group: number }[] = [
  { id: "panel", label: "Panel", icon: LayoutDashboard, group: 0 },
  { id: "movimientos", label: "Movimientos", icon: ArrowLeftRight, group: 0 },
  { id: "presupuestos", label: "Presupuestos", icon: PieChart, group: 0 },
  { id: "suscripciones", label: "Suscripciones", icon: Repeat, group: 0 },
  { id: "huchas", label: "Huchas", icon: PiggyBank, group: 0 },
  { id: "prevision", label: "Previsión", icon: TrendingUp, group: 0 },
  { id: "autonomo", label: "Autónomo", icon: Briefcase, group: 1 },
  { id: "informes", label: "Informes", icon: FileBarChart, group: 1 },
  { id: "cuentas", label: "Cuentas y bancos", icon: Landmark, group: 1 },
  { id: "ajustes", label: "Ajustes", icon: Settings, group: 1 },
];

const THEMES: { id: ThemePref; label: string }[] = [
  { id: "light", label: "Claro" },
  { id: "dark", label: "Oscuro" },
  { id: "oled", label: "OLED" },
];

type Props = {
  route: Route; onRoute: (r: Route) => void;
  theme: ThemePref; onTheme: (t: ThemePref) => void; onLock: () => void;
};

export function Sidebar({ route, onRoute, theme, onTheme, onLock }: Props) {
  const eff = theme === "system" ? document.documentElement.dataset.theme : theme;
  return (
    <aside className="sidebar">
      <div className="sidebar-brand"><Logo /></div>
      <nav>
        {NAV.map((n, i) => (
          <div key={n.id}>
            {i > 0 && NAV[i - 1].group !== n.group && <div className="sep" />}
            <button className={`nav ${route === n.id ? "on" : ""}`} onClick={() => onRoute(n.id)}>
              <n.icon size={18} strokeWidth={2} />
              {n.label}
            </button>
          </div>
        ))}
      </nav>
      <div className="sidebar-foot">
        <button className="nav" onClick={onLock}><Lock size={18} />Bloquear</button>
        <div className="seg">
          {THEMES.map((t) => (
            <button key={t.id} className={eff === t.id ? "on" : ""} onClick={() => onTheme(t.id)}>{t.label}</button>
          ))}
        </div>
      </div>
    </aside>
  );
}

const TABS: Route[] = ["panel", "movimientos", "presupuestos", "prevision"];

/** Navegación del móvil: 4 pestañas + "Más" con el resto de secciones. */
export function BottomNav({ route, onRoute, theme, onTheme, onLock }: Props) {
  const [more, setMore] = useState(false);
  const eff = theme === "system" ? document.documentElement.dataset.theme : theme;
  const rest = NAV.filter((n) => !TABS.includes(n.id));
  const go = (r: Route) => { setMore(false); onRoute(r); };
  return (
    <>
      <nav className="bottom-nav" aria-label="Secciones">
        {TABS.map((id) => {
          const n = NAV.find((x) => x.id === id)!;
          return <button key={id} className={`tab ${route === id ? "on" : ""}`} onClick={() => go(id)}><n.icon size={22} />{n.label}</button>;
        })}
        <button className={`tab ${rest.some((n) => n.id === route) ? "on" : ""}`} onClick={() => setMore(true)}><Grid2x2 size={22} />Más</button>
      </nav>
      {more && (
        <div className="overlay more-sheet" onMouseDown={(e) => e.target === e.currentTarget && setMore(false)}>
          <div className="modal card">
            <div className="modal-body stack">
              <div className="more-grid">
                {rest.map((n) => <button key={n.id} className="more-item" onClick={() => go(n.id)}><n.icon size={24} />{n.label}</button>)}
                <button className="more-item" onClick={() => { setMore(false); onLock(); }}><Lock size={24} />Bloquear</button>
              </div>
              <div className="seg wide">
                {THEMES.map((t) => <button key={t.id} className={eff === t.id ? "on" : ""} onClick={() => onTheme(t.id)}>{t.label}</button>)}
              </div>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
