import { useEffect, useState } from "react";
import { ChevronLeft, ChevronRight, Info } from "lucide-react";
import { api2, errMsg, type BusinessYear } from "../lib/api";
import { fmtMonthShort, money } from "../lib/format";
import { Bars } from "../components/charts";
import { CatIcon, Toast } from "../components/ui";
import type { Nav } from "../App";

/** Resumen del negocio: ingresos, gastos y beneficio de los movimientos de autónomo. Los impuestos van en CereWare Invoicer. */
export function Freelance({ onNav }: { onNav: Nav }) {
  const [year, setYear] = useState(new Date().getFullYear());
  const [d, setD] = useState<BusinessYear | null>(null);
  const [toast, setToast] = useState<string | null>(null);
  useEffect(() => { api2.businessYear(year).then(setD).catch((e) => setToast(errMsg(e))); }, [year]);

  const profit = d ? d.income_cents - d.expense_cents : 0;
  const active = d?.months.filter((m) => m.income_cents || m.expense_cents).length ?? 0;
  const empty = d && d.income_cents === 0 && d.expense_cents === 0;
  const range = { kind: "business" as const, from: `${year}-01-01`, to: `${year}-12-31` };

  return (
    <div className="page">
      <header className="top">
        <div><h1>Autónomo</h1><div className="muted">Ingresos, gastos y beneficio de tu actividad, separados de lo personal.</div></div>
        <div className="month-nav">
          <button className="icon-btn" onClick={() => setYear(year - 1)} aria-label="Año anterior"><ChevronLeft size={18} /></button>
          <b>{year}</b>
          <button className="icon-btn" onClick={() => setYear(year + 1)} aria-label="Año siguiente"><ChevronRight size={18} /></button>
        </div>
      </header>

      {empty ? (
        <div className="card empty">
          <h2>Sin movimientos de autónomo en {year}</h2>
          <p className="muted">Marca una cuenta como "de autónomo" en Cuentas y bancos, o marca movimientos sueltos como de autónomo.</p>
          <button className="btn p" onClick={() => onNav("cuentas")}>Ir a cuentas</button>
        </div>
      ) : d && (
        <>
          <div className="kpi4">
            <div className="card"><div className="muted">Ingresos</div><div className="v num inc">{money(d.income_cents, { sign: true })}</div></div>
            <div className="card"><div className="muted">Gastos</div><div className="v num exp">{money(-d.expense_cents)}</div></div>
            <div className="card"><div className="muted">Beneficio</div><div className={`v num ${profit < 0 ? "exp" : ""}`}>{money(profit)}</div>
              {d.income_cents > 0 && <span className="pill">{Math.round((profit / d.income_cents) * 100)} % de margen</span>}</div>
            <div className="card"><div className="muted">Beneficio medio mensual</div><div className="v num">{money(active ? Math.round(profit / active) : 0)}</div>
              <span className="pill">{active} meses con actividad</span></div>
          </div>

          <div className="card">
            <h3>Ingresos y gastos por mes</h3>
            <Bars data={d.months.map((m) => ({ label: fmtMonthShort(m.month), a: m.income_cents, b: m.expense_cents }))} />
          </div>

          <div className="grid">
            <div className="card span6">
              <h3>Clientes <small>por ingresos</small></h3>
              {d.clients.map((c) => (
                <div className="row" key={c.name}>
                  <div className="t"><b>{c.name}</b><span>{c.count} cobros</span></div>
                  <div className="a num inc">{money(c.cents)}</div>
                </div>
              ))}
              {d.clients.length === 0 && <div className="muted small">Sin ingresos este año.</div>}
            </div>
            <div className="card span6">
              <h3>Gastos del negocio <small>por categoría</small></h3>
              {d.by_category.map((c) => (
                <button className="row cat-row" key={c.id} onClick={() => onNav("movimientos", { ...range, category_id: c.id === "_none" ? undefined : c.id })}>
                  <CatIcon icon={c.icon} color={c.color} size={32} />
                  <div className="t"><b>{c.name}</b></div>
                  <div className="a num">{money(c.cents)}</div>
                </button>
              ))}
              {d.by_category.length === 0 && <div className="muted small">Sin gastos este año.</div>}
            </div>
          </div>

          <div className="banner info"><Info size={16} /><span>Los impuestos (modelos 303 y 130) se gestionan en <b>CereWare Invoicer</b>.</span></div>
          <div className="row-gap">
            <button className="btn g" onClick={() => onNav("movimientos", range)}>Ver movimientos de autónomo</button>
          </div>
        </>
      )}
      {toast && <Toast msg={toast} onDone={() => setToast(null)} />}
    </div>
  );
}
