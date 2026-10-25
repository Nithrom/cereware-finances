import { useCallback, useEffect, useState } from "react";
import { api, type TxFilter, type VaultStatus } from "./lib/api";
import { useTheme } from "./lib/theme";
import { useAutolock } from "./lib/autolock";
import { useAutoSync } from "./lib/sync";
import { Sidebar, BottomNav, type Route } from "./components/Sidebar";
import { invoke } from "@tauri-apps/api/core";
import { TitleBar } from "./components/TitleBar";
import { UpdateBanner } from "./components/UpdateBanner";
import { refreshWidget } from "./lib/android";
import { checkUpdate, type Update } from "./lib/update";
import { LockScreen } from "./screens/Lock";
import { Dashboard } from "./screens/Dashboard";
import { Accounts } from "./screens/Accounts";
import { Transactions } from "./screens/Transactions";
import { Budgets } from "./screens/Budgets";
import { Subscriptions } from "./screens/Subscriptions";
import { Goals } from "./screens/Goals";
import { Forecast } from "./screens/Forecast";
import { Freelance } from "./screens/Freelance";
import { Reports } from "./screens/Reports";
import { Settings } from "./screens/Settings";

export type Nav = (r: Route, preset?: TxFilter) => void;

export default function App() {
  const [theme, setTheme] = useTheme();
  const [status, setStatus] = useState<VaultStatus | null>(null);
  const [route, setRoute] = useState<Route>("panel");
  const [preset, setPreset] = useState<TxFilter | undefined>();
  const [version, setVersion] = useState(0);
  const [mobile, setMobile] = useState(false);
  useEffect(() => { invoke<string>("app_platform").then((p) => setMobile(p === "android" || p === "ios")).catch(() => {}); }, []);
  const Bar = mobile ? () => null : TitleBar;
  const shell = `shell ${mobile ? "mobile" : ""}`;

  const refresh = useCallback(() => { api.vaultStatus().then(setStatus); }, []);
  useEffect(refresh, [refresh]);
  const lock = useCallback(async () => { await api.vaultLock(); setRoute("panel"); refresh(); }, [refresh]);
  useAutolock(!!status?.unlocked, lock);
  useAutoSync(!!status?.unlocked);
  // Si llegan cambios de otro dispositivo, refresca la pantalla
  useEffect(() => {
    const h = (e: Event) => { const d = (e as CustomEvent).detail; if (d?.state === "ok") { refreshWidget(); if (d.pulled > 0) setVersion((v) => v + 1); } };
    window.addEventListener("cwf-sync", h);
    return () => window.removeEventListener("cwf-sync", h);
  }, []);

  const [update, setUpdate] = useState<Update | null>(null);
  // Widget de Android: al desbloquear y al salir de la app. Y una comprobación de versión nueva.
  useEffect(() => {
    if (!status?.unlocked) return;
    checkUpdate().then(setUpdate).catch(() => {});
    refreshWidget();
    const vis = () => document.visibilityState === "hidden" && refreshWidget();
    document.addEventListener("visibilitychange", vis);
    return () => document.removeEventListener("visibilitychange", vis);
  }, [status?.unlocked]);

  if (!status) return <div className={shell}><Bar /></div>;
  if (!status.unlocked) return <div className={shell}><Bar /><div className="shell-body"><LockScreen exists={status.exists} onUnlocked={refresh} mobile={mobile} /></div></div>;

  const nav: Nav = (r, p) => { setPreset(p); setRoute(r); document.querySelector(".main")?.scrollTo(0, 0); };
  const bump = () => setVersion((v) => v + 1);

  let screen;
  switch (route) {
    case "panel": screen = <Dashboard key={version} onNav={nav} />; break;
    case "movimientos": screen = <Transactions key={JSON.stringify(preset)} preset={preset} onChanged={bump} />; break;
    case "presupuestos": screen = <Budgets onNav={nav} />; break;
    case "suscripciones": screen = <Subscriptions />; break;
    case "huchas": screen = <Goals />; break;
    case "prevision": screen = <Forecast onNav={nav} />; break;
    case "autonomo": screen = <Freelance onNav={nav} />; break;
    case "informes": screen = <Reports onNav={nav} />; break;
    case "cuentas": screen = <Accounts onImported={bump} />; break;
    case "ajustes": screen = <Settings mobile={mobile} onRestored={() => { bump(); nav("panel"); }} />; break;
  }

  return (
    <div className={shell}>
      <Bar />
      <div className="app">
        <Sidebar route={route} onRoute={(r) => nav(r)} theme={theme} onTheme={setTheme} onLock={lock} />
        <main className="main" key={route === "movimientos" ? `m${version}` : version}><UpdateBanner update={update} onClose={() => setUpdate(null)} />{screen}</main>
        <BottomNav route={route} onRoute={(r) => nav(r)} theme={theme} onTheme={setTheme} onLock={lock} />
      </div>
    </div>
  );
}
