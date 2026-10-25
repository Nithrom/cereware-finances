import { useCallback, useEffect, useState } from "react";
import { api, type TxFilter, type VaultStatus } from "./lib/api";
import { useTheme } from "./lib/theme";
import { useAutolock } from "./lib/autolock";
import { Sidebar, type Route } from "./components/Sidebar";
import { TitleBar } from "./components/TitleBar";
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

  const refresh = useCallback(() => { api.vaultStatus().then(setStatus); }, []);
  useEffect(refresh, [refresh]);
  const lock = useCallback(async () => { await api.vaultLock(); setRoute("panel"); refresh(); }, [refresh]);
  useAutolock(!!status?.unlocked, lock);

  if (!status) return <div className="shell"><TitleBar /></div>;
  if (!status.unlocked) return <div className="shell"><TitleBar /><div className="shell-body"><LockScreen exists={status.exists} onUnlocked={refresh} /></div></div>;

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
    case "ajustes": screen = <Settings onRestored={() => { bump(); nav("panel"); }} />; break;
  }

  return (
    <div className="shell">
      <TitleBar />
      <div className="app">
        <Sidebar route={route} onRoute={(r) => nav(r)} theme={theme} onTheme={setTheme} onLock={lock} />
        <main className="main">{screen}</main>
      </div>
    </div>
  );
}
