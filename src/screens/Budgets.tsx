import { useCallback, useEffect, useState } from "react";
import { ChevronLeft, ChevronRight, Wand2 } from "lucide-react";
import { api2, errMsg, type BudgetRow } from "../lib/api";
import { fmtMonth, money, parseMoney } from "../lib/format";
import { CatIcon, Toast } from "../components/ui";
import { Progress } from "../components/charts";
import type { Nav } from "../App";

const shift = (ym: string, n: number) => {
  const [y, m] = ym.split("-").map(Number);
  const d = new Date(y, m - 1 + n, 15);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
};
const thisMonth = () => new Date().toISOString().slice(0, 7);

export function Budgets({ onNav }: { onNav: Nav }) {
  const [month, setMonth] = useState(thisMonth());
  const [rows, setRows] = useState<BudgetRow[]>([]);
  const [editing, setEditing] = useState<string | null>(null);
  const [val, setVal] = useState("");
  const [toast, setToast] = useState<string | null>(null);
  const load = useCallback(() => api2.budgets(month).then(setRows).catch((e) => setToast(errMsg(e))), [month]);
  useEffect(() => { load(); }, [load]);

  const withB = rows.filter((r) => r.budget_cents);
  const totalB = withB.reduce((s, r) => s + (r.budget_cents ?? 0), 0);
  const totalS = withB.reduce((s, r) => s + r.spent_cents, 0);
  const daysLeft = month === thisMonth() ? new Date(new Date().getFullYear(), new Date().getMonth() + 1, 0).getDate() - new Date().getDate() + 1 : 0;

  async function save(id: string, cents: number | null) {
    try { await api2.budgetSet(id, null, cents); setEditing(null); load(); } catch (e) { setToast(errMsg(e)); }
  }
  async function suggest() {
    const todo = rows.filter((r) => !r.budget_cents && r.avg_cents > 0);
    for (const r of todo) await api2.budgetSet(r.category_id, null, Math.ceil(r.avg_cents / 1000) * 1000);
    setToast(todo.length ? `${todo.length} presupuestos creados a partir de tu media de 3 meses` : "No hay datos suficientes para sugerir presupuestos");
    load();
  }

  return (
    <div className="page">
      <header className="top">
        <div><h1>Presupuestos</h1><div className="muted">Límite mensual por categoría. Se aplica a todos los meses.</div></div>
        <div className="row-gap">
          <div className="month-nav">
            <button className="icon-btn" onClick={() => setMonth(shift(month, -1))} aria-label="Mes anterior"><ChevronLeft size={18} /></button>
            <b className="cap">{fmtMonth(month)}</b>
            <button className="icon-btn" onClick={() => setMonth(shift(month, 1))} aria-label="Mes siguiente"><ChevronRight size={18} /></button>
          </div>
          <button className="btn g" onClick={suggest}><Wand2 size={16} />Sugerir según mi media</button>
        </div>
      </header>

      {withB.length > 0 && (
        <div className="card summary">
          <div><span className="muted">Presupuestado</span><b className="num">{money(totalB)}</b></div>
          <div><span className="muted">Gastado</span><b className="num">{money(totalS)}</b></div>
          <div><span className="muted">Disponible</span><b className={`num ${totalB - totalS < 0 ? "exp" : "inc"}`}>{money(totalB - totalS)}</b></div>
          {daysLeft > 0 && totalB > totalS && <div><span className="muted">Por día ({daysLeft} días)</span><b className="num">{money(Math.floor((totalB - totalS) / daysLeft))}</b></div>}
          <div className="grow"><Progress value={totalS} max={totalB} /></div>
        </div>
      )}

      <div className="card">
        {rows.map((r) => {
          const pct = r.budget_cents ? Math.round((r.spent_cents / r.budget_cents) * 100) : null;
          return (
            <div className="budget-row" key={r.category_id}>
              <CatIcon icon={r.icon} color={r.color} />
              <div className="grow">
                <div className="h">
                  <button className="link strong" onClick={() => onNav("movimientos", { category_id: r.category_id, from: month + "-01", to: month + "-31" })}>{r.name}</button>
                  <span className="num muted small">{money(r.spent_cents)}{r.budget_cents ? ` de ${money(r.budget_cents)}` : ""} · media {money(r.avg_cents)}</span>
                </div>
                {r.budget_cents ? <Progress value={r.spent_cents} max={r.budget_cents} /> : <div className="muted small">Sin presupuesto</div>}
              </div>
              {pct !== null && <span className={`pill ${pct >= 100 ? "exp" : pct >= 80 ? "warn" : ""}`}>{pct} %</span>}
              {editing === r.category_id ? (
                <form className="inline-edit" onSubmit={(e) => { e.preventDefault(); save(r.category_id, parseMoney(val)); }}>
                  <input className="input sm num" autoFocus value={val} onChange={(e) => setVal(e.target.value)} placeholder="0,00" />
                  <button className="btn p sm">OK</button>
                  {r.budget_cents && <button type="button" className="btn g sm" onClick={() => save(r.category_id, null)}>Quitar</button>}
                </form>
              ) : (
                <button className="btn g sm" onClick={() => { setEditing(r.category_id); setVal(r.budget_cents ? (r.budget_cents / 100).toString().replace(".", ",") : ""); }}>
                  {r.budget_cents ? "Editar" : "Fijar"}
                </button>
              )}
            </div>
          );
        })}
      </div>
      {toast && <Toast msg={toast} onDone={() => setToast(null)} />}
    </div>
  );
}
