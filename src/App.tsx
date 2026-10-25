import { useCallback, useEffect, useState } from "react";
import { api, type VaultStatus } from "./lib/api";
import { useTheme } from "./lib/theme";
import { NAV, Sidebar, type Route } from "./components/Sidebar";
import { LockScreen } from "./screens/Lock";
import { Dashboard } from "./screens/Dashboard";
import { Placeholder } from "./screens/Placeholder";

export default function App() {
  const [theme, setTheme] = useTheme();
  const [status, setStatus] = useState<VaultStatus | null>(null);
  const [route, setRoute] = useState<Route>("panel");

  const refresh = useCallback(() => { api.vaultStatus().then(setStatus); }, []);
  useEffect(refresh, [refresh]);

  if (!status) return null;
  if (!status.unlocked) return <LockScreen exists={status.exists} onUnlocked={refresh} />;

  const lock = async () => { await api.vaultLock(); setRoute("panel"); refresh(); };
  const title = NAV.find((n) => n.id === route)!.label;

  return (
    <div className="app">
      <Sidebar route={route} onRoute={setRoute} theme={theme} onTheme={setTheme} onLock={lock} />
      <main className="main">
        {route === "panel" ? <Dashboard onRoute={setRoute} /> : <Placeholder title={title} />}
      </main>
    </div>
  );
}
