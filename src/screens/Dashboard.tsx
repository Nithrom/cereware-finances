import { useEffect, useState } from "react";
import { Upload, Landmark, FileSpreadsheet, AlertTriangle, CalendarClock } from "lucide-react";
import { api, api2, type Account, type BudgetRow, type Dashboard as D, type Goal, type Recurring, type TxRow, type Category } from "../lib/api";
import { fmtDate, fmtMonthShort, greeting, money, today } from "../lib/format";
import { CatIcon } from "../components/ui";
import { Donut, LineArea, Progress, Ring } from "../components/charts";
import type { Nav } from "../App";

export function Dashboard({ onNav }: { onNav: Nav }) {
  const [d, setD] = useState<D | null>(null);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [recent, setRecent] = useState<TxRow[]>([]);
  const [budgets, setBudgets] = useState<BudgetRow[]>([]);
  const [rec, setRec] = useState<Recurring[]>([]);
  const [goals, setGoals] = useState<Goal[]>([]);
  const [cats, setCats] = useState<Record<string, Category>>({});

  useEffect(() => {
    api2.dashboard().then((x) => {
      setD(x);
      if (x.accounts === 0) return;
      api2.budgets(x.month).then((b) => setBudgets(b.filter((r) => r.budget_cents)));
      api2.recurring().then((r) => setRec(r.filter((x) => x.active)));
    }).catch(console.error);
    api.accounts().then(setAccounts);
    api.txList({ limit: 6 }).then((p) => setRecent(p.items));
    api2.goals().then(setGoals);
    api.categories().then((c) => setCats(Object.fromEntries(c.map((x) => [x.id, x]))));
  }, []);

  if (!d) return null;
  const empty = d.accounts === 0;
  const upcoming = rec.filter((r) => r.amount_cents < 0 && new Date(r.next_date) <= new Date(Date.now() + 14 * 864e5)).slice(0, 4);
  const pct = (a: number, b: number) => (b ? Math.round(((a - b) / b) * 100) : null);
  const expVs = pct(d.month_expense_cents, d.avg_expense_cents);
  const incVs = pct(d.month_income_cents, d.avg_income_cents);
  const hist = d.balance_history;
  const monthDelta = hist.length > 1 ? hist[hist.length - 1].value_cents - hist[hist.length - 2].value_cents : 0;

  return (
    <div className="page">
      <header className="top">
        <div>
          <h1>{greeting()}</h1>
          <div className="muted cap">{today()}</div>
        </div>
        <div className="row-gap">
          <button className="btn g" onClick={() => onNav("cuentas")}><Upload size={16} />Importar extracto</button>
        </div>
      </header>

      {empty ? (
        <section className="card empty">
          <h2>Empecemos con tus cuentas</h2>
          <p className="muted">Importa un extracto de tu banco (CSV, Excel, OFX o Norma 43) o crea una cuenta manual.</p>
          <div className="empty-actions">
            <button className="choice" onClick={() => onNav("cuentas")}>
              <FileSpreadsheet size={22} /><b>Importar extracto</b><span>Funciona con cualquier banco: Openbank, BBVA, Revolut…</span>
            </button>
            <button className="choice" onClick={() => onNav("cuentas")}>
              <Landmark size={22} /><b>Conectar banco</b><span>Sincronización automática de solo lectura (próximamente)</span>
            </button>
          </div>
        </section>
      ) : (
        <>
          {d.uncategorized > 0 && (
            <div className="banner warn clickable" onClick={() => onNav("movimientos", { kind: "uncategorized" })}>
              <AlertTriangle size={16} /><span><b>{d.uncategorized} movimientos sin categoría.</b> Categorízalos para que los informes sean precisos.</span>
            </div>
          )}
          <div className="grid">
            <div className="card hero">
              <div className="hero-cherry" />
              <div className="lbl">Saldo total</div>
              <div className="big num">{money(d.total_balance_cents)}</div>
              {monthDelta !== 0 && <span className="hero-chip num">{monthDelta > 0 ? "▲" : "▼"} {money(Math.abs(monthDelta))} este mes</span>}
              <div className="accts">
                {accounts.slice(0, 3).map((a) => (
                  <div className="acct" key={a.id}>{a.institution}<b className="num">{money(a.balance_cents)}</b></div>
                ))}
              </div>
            </div>
            <div className="kpi">
              <div className="card"><div className="muted">Ingresos del mes</div><div className="v num inc">{money(d.month_income_cents, { sign: true })}</div>
                {incVs !== null && <span className={`pill ${incVs >= 0 ? "inc" : "exp"}`}>{incVs >= 0 ? "+" : ""}{incVs} % vs media</span>}</div>
              <div className="card"><div className="muted">Gastos del mes</div><div className="v num exp">{money(-d.month_expense_cents)}</div>
                {expVs !== null && <span className={`pill ${expVs <= 0 ? "inc" : "exp"}`}>{expVs >= 0 ? "+" : ""}{expVs} % vs media</span>}</div>
              <div className="card"><div className="muted">Ahorro del mes</div>
                <div className={`v num ${d.month_income_cents - d.month_expense_cents >= 0 ? "" : "exp"}`}>{money(d.month_income_cents - d.month_expense_cents)}</div>
                {d.month_income_cents > 0 && <span className="pill">{Math.round(((d.month_income_cents - d.month_expense_cents) / d.month_income_cents) * 100)} % de lo ingresado</span>}</div>
            </div>

            <div className="card span8">
              <h3>Evolución del saldo <button className="link small" onClick={() => onNav("prevision")}>Ver previsión</button></h3>
              <LineArea points={hist.map((p) => ({ label: fmtMonthShort(p.month), value: p.value_cents }))} />
            </div>
            <div className="card span4">
              <h3>Gastos por categoría <small>este mes</small></h3>
              {d.by_category.length ? (
                <>
                  <Donut total={d.month_expense_cents} slices={d.by_category.slice(0, 6).map((c) => ({ color: c.color, value: c.cents, name: c.name }))} />
                  <div className="legend">
                    {d.by_category.slice(0, 6).map((c) => (
                      <button key={c.id} className="legend-item" onClick={() => onNav("movimientos", { category_id: c.id === "_none" ? undefined : c.id, kind: c.id === "_none" ? "uncategorized" : "expense", from: d.month + "-01" })}>
                        <i style={{ background: c.color }} />{c.name}<span className="num">{Math.round((c.cents / Math.max(d.month_expense_cents, 1)) * 100)} %</span>
                      </button>
                    ))}
                  </div>
                </>
              ) : <div className="muted small">Sin gastos este mes.</div>}
            </div>

            <div className="card span7">
              <h3>Últimos movimientos <button className="link small" onClick={() => onNav("movimientos")}>Ver todos</button></h3>
              {recent.map((t) => {
                const c = t.category_id ? cats[t.category_id] : undefined;
                return (
                  <div className="row" key={t.id}>
                    <CatIcon icon={t.is_transfer ? "arrow-left-right" : c?.icon} color={c?.color} />
                    <div className="t"><b className="ellipsis">{t.description}</b><span>{t.category_name ?? "Sin categoría"} · {t.institution} · {fmtDate(t.date)}</span></div>
                    <div className={`a num ${t.is_transfer ? "muted" : t.amount_cents > 0 ? "inc" : "exp"}`}>{money(t.amount_cents, { sign: true })}</div>
                  </div>
                );
              })}
            </div>
            <div className="span5 col">
              <div className="card">
                <h3>Próximos cargos <button className="link small" onClick={() => onNav("suscripciones")}>Ver todos</button></h3>
                {upcoming.length ? upcoming.map((r) => (
                  <div className="row" key={r.id}>
                    <CatIcon icon={r.category_icon} color={r.category_color} size={32} />
                    <div className="t"><b>{r.name}</b><span>{fmtDate(r.next_date)}</span></div>
                    <div className="a num exp">{money(r.amount_cents)}</div>
                  </div>
                )) : <div className="muted small row-gap"><CalendarClock size={15} />Nada en los próximos 14 días.</div>}
              </div>
              {budgets.length > 0 && (
                <div className="card">
                  <h3>Presupuestos <button className="link small" onClick={() => onNav("presupuestos")}>Gestionar</button></h3>
                  {budgets.slice(0, 4).map((b) => (
                    <div className="bud" key={b.category_id}>
                      <div className="h"><span>{b.name}</span><span className="num muted">{money(b.spent_cents)} / {money(b.budget_cents!)}</span></div>
                      <Progress value={b.spent_cents} max={b.budget_cents!} />
                    </div>
                  ))}
                </div>
              )}
              {goals.length > 0 && (
                <div className="card">
                  <h3>Huchas <button className="link small" onClick={() => onNav("huchas")}>Ver</button></h3>
                  {goals.slice(0, 2).map((g) => (
                    <div className="goal" key={g.id}>
                      <Ring value={g.saved_cents} max={g.target_cents} color={g.color} />
                      <div className="grow"><b>{g.name}</b><div className="muted small num">{money(g.saved_cents)} de {money(g.target_cents)}</div></div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>
        </>
      )}
    </div>
  );
}
