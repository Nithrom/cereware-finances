import { useEffect, useState } from "react";
import { ChevronLeft, ChevronRight, Info, Settings2 } from "lucide-react";
import { api2, errMsg, type FiscalConfig, type Quarter } from "../lib/api";
import { fmtDate, money } from "../lib/format";
import { Field, Modal, Toast } from "../components/ui";
import type { Nav } from "../App";

export function Freelance({ onNav }: { onNav: Nav }) {
  const [year, setYear] = useState(new Date().getFullYear());
  const [qs, setQs] = useState<Quarter[]>([]);
  const [cfg, setCfg] = useState<FiscalConfig | null>(null);
  const [showCfg, setShowCfg] = useState(false);
  const [toast, setToast] = useState<string | null>(null);
  const load = () => api2.fiscalYear(year).then(setQs).catch((e) => setToast(errMsg(e)));
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => { load(); }, [year]);
  useEffect(() => { api2.fiscalConfig().then(setCfg); }, []);

  const curQ = Math.floor(new Date().getMonth() / 3) + 1;
  const next = qs.find((q) => q.year === new Date().getFullYear() && q.quarter === curQ) ?? qs[0];
  const tot = (k: keyof Quarter) => qs.reduce((s, q) => s + (q[k] as number), 0);
  const noData = qs.every((q) => q.income_count + q.expense_count === 0);

  return (
    <div className="page">
      <header className="top">
        <div><h1>Autónomo</h1><div className="muted">Estimación de IVA (modelo 303) e IRPF (modelo 130) a partir de tus movimientos de autónomo.</div></div>
        <div className="row-gap">
          <div className="month-nav">
            <button className="icon-btn" onClick={() => setYear(year - 1)} aria-label="Año anterior"><ChevronLeft size={18} /></button>
            <b>{year}</b>
            <button className="icon-btn" onClick={() => setYear(year + 1)} aria-label="Año siguiente"><ChevronRight size={18} /></button>
          </div>
          <button className="btn g" onClick={() => setShowCfg(true)}><Settings2 size={16} />Configurar</button>
        </div>
      </header>

      {noData ? (
        <div className="card empty">
          <h2>Sin movimientos de autónomo en {year}</h2>
          <p className="muted">Marca una cuenta como "de autónomo" en Cuentas y bancos, o marca movimientos sueltos como de autónomo e indica su IVA.</p>
          <button className="btn p" onClick={() => onNav("cuentas")}>Ir a cuentas</button>
        </div>
      ) : (
        <>
          {next && year === new Date().getFullYear() && (
            <div className="card hero fiscal">
              <div className="hero-cherry" />
              <div className="lbl">Trimestre en curso · {next.quarter}T · presentar antes del {fmtDate(next.deadline)}</div>
              <div className="fiscal-row">
                <div><span>Modelo 303 (IVA)</span><b className="num">{money(next.m303_cents)}</b></div>
                <div><span>Modelo 130 (IRPF)</span><b className="num">{money(next.m130_cents)}</b></div>
                <div><span>Total a apartar</span><b className="num">{money(Math.max(0, next.m303_cents) + next.m130_cents)}</b></div>
              </div>
            </div>
          )}
          <div className="card table-wrap">
            <table className="table fiscal-table">
              <thead><tr><th></th>{qs.map((q) => <th key={q.quarter} className="r">{q.quarter}T</th>)}<th className="r">Año</th></tr></thead>
              <tbody>
                <tr><td>Ingresos (base)</td>{qs.map((q) => <td key={q.quarter} className="r num">{money(q.income_base_cents)}</td>)}<td className="r num b">{money(tot("income_base_cents"))}</td></tr>
                <tr><td>IVA repercutido</td>{qs.map((q) => <td key={q.quarter} className="r num">{money(q.vat_output_cents)}</td>)}<td className="r num">{money(tot("vat_output_cents"))}</td></tr>
                <tr><td>Gastos deducibles (base)</td>{qs.map((q) => <td key={q.quarter} className="r num">{money(q.expense_base_cents + q.non_vat_expenses_cents)}</td>)}<td className="r num b">{money(tot("expense_base_cents") + tot("non_vat_expenses_cents"))}</td></tr>
                <tr><td>IVA soportado</td>{qs.map((q) => <td key={q.quarter} className="r num">{money(q.vat_input_cents)}</td>)}<td className="r num">{money(tot("vat_input_cents"))}</td></tr>
                {tot("retention_cents") > 0 && <tr><td>Retenciones soportadas</td>{qs.map((q) => <td key={q.quarter} className="r num">{money(q.retention_cents)}</td>)}<td className="r num">{money(tot("retention_cents"))}</td></tr>}
                <tr className="hl"><td><b>Modelo 303</b> (a ingresar)</td>{qs.map((q) => <td key={q.quarter} className={`r num b ${q.m303_cents < 0 ? "inc" : ""}`}>{money(q.m303_cents)}</td>)}<td className="r num b">{money(tot("m303_cents"))}</td></tr>
                <tr><td>Rendimiento acumulado</td>{qs.map((q) => <td key={q.quarter} className="r num">{money(q.ytd_profit_cents)}</td>)}<td></td></tr>
                <tr className="hl"><td><b>Modelo 130</b> (a ingresar)</td>{qs.map((q) => <td key={q.quarter} className="r num b">{money(q.m130_cents)}</td>)}<td className="r num b">{money(tot("m130_cents"))}</td></tr>
                <tr><td className="muted">Plazo</td>{qs.map((q) => <td key={q.quarter} className="r muted small">{fmtDate(q.deadline)}</td>)}<td></td></tr>
              </tbody>
            </table>
          </div>
          <div className="banner info"><Info size={16} /><span>Estimación orientativa para estimación directa simplificada. No incluye prorratas, recargo de equivalencia ni gastos de difícil justificación. Revísalo con tu gestor antes de presentar.</span></div>
          <div className="row-gap">
            <button className="btn g" onClick={() => onNav("movimientos", { kind: "business", from: `${year}-01-01`, to: `${year}-12-31` })}>Ver movimientos de autónomo</button>
          </div>
        </>
      )}

      {showCfg && cfg && <ConfigForm cfg={cfg} onClose={() => setShowCfg(false)} onSaved={(c) => { setCfg(c); setShowCfg(false); load(); setToast("Configuración guardada"); }} />}
      {toast && <Toast msg={toast} onDone={() => setToast(null)} />}
    </div>
  );
}

function ConfigForm({ cfg, onClose, onSaved }: { cfg: FiscalConfig; onClose: () => void; onSaved: (c: FiscalConfig) => void }) {
  const [c, setC] = useState(cfg);
  async function save() { await api2.fiscalConfigSet(c); onSaved(c); }
  return (
    <Modal title="Configuración fiscal" onClose={onClose} footer={<><button className="btn g" onClick={onClose}>Cancelar</button><button className="btn p" onClick={save}>Guardar</button></>}>
      <div className="stack">
        <Field label="IVA de tus facturas" hint="Se usa en los ingresos que no tengan otro tipo indicado.">
          <select className="input" value={c.income_vat} onChange={(e) => setC({ ...c, income_vat: Number(e.target.value) })}>
            {[21, 10, 4, 0].map((v) => <option key={v} value={v}>{v} %</option>)}
          </select>
        </Field>
        <Field label="Retención de IRPF en tus facturas" hint="Si facturas a empresas como profesional, suelen retenerte un 15 % (7 % los primeros años).">
          <select className="input" value={c.income_retention} onChange={(e) => setC({ ...c, income_retention: Number(e.target.value) })}>
            {[0, 7, 15].map((v) => <option key={v} value={v}>{v} %</option>)}
          </select>
        </Field>
        <Field label="IVA por defecto de los gastos" hint="Para gastos de autónomo sin tipo de IVA indicado. Mejor indicarlo en cada movimiento.">
          <select className="input" value={c.expense_vat} onChange={(e) => setC({ ...c, expense_vat: Number(e.target.value) })}>
            {[0, 21, 10, 4].map((v) => <option key={v} value={v}>{v === 0 ? "Sin IVA" : `${v} %`}</option>)}
          </select>
        </Field>
      </div>
    </Modal>
  );
}
