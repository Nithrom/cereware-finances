import { useEffect, useState } from "react";
import { Upload, RefreshCw, Landmark, FileSpreadsheet } from "lucide-react";
import { api, type Account, type DashboardSummary } from "../lib/api";
import { greeting, money, today } from "../lib/format";
import type { Route } from "../components/Sidebar";

export function Dashboard({ onRoute }: { onRoute: (r: Route) => void }) {
  const [sum, setSum] = useState<DashboardSummary | null>(null);
  const [accounts, setAccounts] = useState<Account[]>([]);

  useEffect(() => {
    api.dashboard().then(setSum).catch(console.error);
    api.accounts().then(setAccounts).catch(console.error);
  }, []);

  const empty = sum !== null && sum.accounts === 0;

  return (
    <div className="page">
      <header className="top">
        <div>
          <h1>{greeting()}</h1>
          <div className="muted cap">{today()}</div>
        </div>
        <div className="row-gap">
          <button className="btn g" onClick={() => onRoute("cuentas")}><Upload size={16} />Importar extracto</button>
          <button className="btn p" disabled={empty}><RefreshCw size={16} />Sincronizar bancos</button>
        </div>
      </header>

      {empty ? (
        <section className="card empty">
          <h2>Empecemos conectando tus cuentas</h2>
          <p className="muted">Conecta tu banco en modo solo lectura o importa un extracto (CSV, Excel, OFX o Norma 43).</p>
          <div className="empty-actions">
            <button className="choice" onClick={() => onRoute("cuentas")}>
              <Landmark size={22} /><b>Conectar banco</b><span>Openbank, BBVA, Revolut y muchos más bancos europeos</span>
            </button>
            <button className="choice" onClick={() => onRoute("cuentas")}>
              <FileSpreadsheet size={22} /><b>Importar extracto</b><span>Funciona con cualquier banco, sin conexión</span>
            </button>
          </div>
        </section>
      ) : (
        <div className="grid">
          <div className="card hero">
            <div className="hero-cherry" />
            <div className="lbl">Saldo total</div>
            <div className="big num">{money(sum?.total_balance_cents ?? 0)}</div>
            <div className="accts">
              {accounts.slice(0, 4).map((a) => (
                <div className="acct" key={a.id}>{a.institution}<b className="num">{money(a.balance_cents)}</b></div>
              ))}
            </div>
          </div>
          <div className="kpi">
            <div className="card"><div className="muted">Ingresos del mes</div><div className="v num inc">{money(sum?.month_income_cents ?? 0, { sign: true })}</div></div>
            <div className="card"><div className="muted">Gastos del mes</div><div className="v num exp">{money(sum?.month_expense_cents ?? 0)}</div></div>
            <div className="card"><div className="muted">Movimientos</div><div className="v num">{sum?.transactions ?? 0}</div></div>
          </div>
        </div>
      )}
    </div>
  );
}
