import { useEffect, useState } from "react";
import { Download } from "lucide-react";
import { save } from "@tauri-apps/plugin-dialog";
import { api2, errMsg, type CatSlice, type MonthFlow } from "../lib/api";
import { fmtMonthShort, money } from "../lib/format";
import { Bars, Donut } from "../components/charts";
import { CatIcon, Toast } from "../components/ui";
import type { Nav } from "../App";

const PERIODS = [
  { id: "month", label: "Este mes" },
  { id: "3m", label: "3 meses" },
  { id: "year", label: "Este año" },
  { id: "12m", label: "12 meses" },
] as const;
type P = (typeof PERIODS)[number]["id"];

function range(p: P): [string, string] {
  const now = new Date();
  const iso = (d: Date) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  const to = iso(now);
  if (p === "month") return [iso(new Date(now.getFullYear(), now.getMonth(), 1)), to];
  if (p === "3m") return [iso(new Date(now.getFullYear(), now.getMonth() - 2, 1)), to];
  if (p === "year") return [`${now.getFullYear()}-01-01`, to];
  return [iso(new Date(now.getFullYear(), now.getMonth() - 11, 1)), to];
}

export function Reports({ onNav }: { onNav: Nav }) {
  const [period, setPeriod] = useState<P>("3m");
  const [kind, setKind] = useState<"expense" | "income">("expense");
  const [cats, setCats] = useState<CatSlice[]>([]);
  const [flows, setFlows] = useState<MonthFlow[]>([]);
  const [toast, setToast] = useState<string | null>(null);
  const [from, to] = range(period);
  useEffect(() => { api2.reportCategories(from, to, kind).then(setCats).catch((e) => setToast(errMsg(e))); }, [from, to, kind]);
  useEffect(() => { api2.reportFlows(12).then(setFlows); }, []);
  const total = cats.reduce((s, c) => s + c.cents, 0);
  const inc = flows.reduce((s, f) => s + f.income_cents, 0), exp = flows.reduce((s, f) => s + f.expense_cents, 0);

  async function exportCsv() {
    const dest = await save({ defaultPath: `movimientos-${from}_${to}.csv`, filters: [{ name: "CSV", extensions: ["csv"] }] });
    if (!dest) return;
    try { const n = await api2.exportCsv(dest, { from, to }); setToast(`${n} movimientos exportados`); } catch (e) { setToast(errMsg(e)); }
  }

  return (
    <div className="page">
      <header className="top">
        <div><h1>Informes</h1></div>
        <div className="row-gap">
          <div className="seg">{PERIODS.map((p) => <button key={p.id} className={period === p.id ? "on" : ""} onClick={() => setPeriod(p.id)}>{p.label}</button>)}</div>
          <button className="btn g" onClick={exportCsv}><Download size={16} />Exportar CSV</button>
        </div>
      </header>

      <div className="card">
        <h3>Ingresos y gastos <small>últimos 12 meses</small></h3>
        <Bars data={flows.map((f) => ({ label: fmtMonthShort(f.month), a: f.income_cents, b: f.expense_cents }))} />
        <div className="summary-inline num">
          <span>Ingresos <b className="inc">{money(inc)}</b></span>
          <span>Gastos <b className="exp">{money(-exp)}</b></span>
          <span>Ahorro <b>{money(inc - exp)}</b></span>
          {inc > 0 && <span>Tasa de ahorro <b>{Math.round(((inc - exp) / inc) * 100)} %</b></span>}
        </div>
      </div>

      <div className="grid">
        <div className="card span4 center-col">
          <div className="seg">
            <button className={kind === "expense" ? "on" : ""} onClick={() => setKind("expense")}>Gastos</button>
            <button className={kind === "income" ? "on" : ""} onClick={() => setKind("income")}>Ingresos</button>
          </div>
          <Donut size={200} total={total} label={kind === "expense" ? "gastado" : "ingresado"} slices={cats.map((c) => ({ color: c.color, value: c.cents, name: c.name }))} />
        </div>
        <div className="card span8">
          <h3>Por categoría</h3>
          {cats.map((c) => (
            <button className="row cat-row" key={c.id} onClick={() => onNav("movimientos", { category_id: c.id === "_none" ? undefined : c.id, kind: c.id === "_none" ? "uncategorized" : kind, from, to })}>
              <CatIcon icon={c.icon} color={c.color} />
              <div className="t"><b>{c.name}</b><div className="bar"><i style={{ width: `${(c.cents / Math.max(cats[0]?.cents ?? 1, 1)) * 100}%`, background: c.color }} /></div></div>
              <div className="r"><div className="a num">{money(c.cents)}</div><div className="muted small num">{Math.round((c.cents / Math.max(total, 1)) * 100)} %</div></div>
            </button>
          ))}
          {cats.length === 0 && <div className="muted small">Sin datos en este periodo.</div>}
        </div>
      </div>
      {toast && <Toast msg={toast} onDone={() => setToast(null)} />}
    </div>
  );
}
