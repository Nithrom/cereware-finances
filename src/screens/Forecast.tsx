import { useEffect, useState } from "react";
import { AlertTriangle, CalendarClock } from "lucide-react";
import { api2, errMsg, type Forecast as F } from "../lib/api";
import { fmtDate, money } from "../lib/format";
import { LineArea } from "../components/charts";
import type { Nav } from "../App";

export function Forecast({ onNav }: { onNav: Nav }) {
  const [days, setDays] = useState(90);
  const [variable, setVariable] = useState(true);
  const [f, setF] = useState<F | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [threshold, setThreshold] = useState(() => { try { return Number(localStorage.getItem("cwf-low") ?? "20000"); } catch { return 20000; } });
  useEffect(() => { api2.forecast(days, variable).then(setF).catch((e) => setErr(errMsg(e))); }, [days, variable]);
  useEffect(() => { try { localStorage.setItem("cwf-low", String(threshold)); } catch { /* */ } }, [threshold]);

  if (err) return <div className="page"><div className="banner warn">{err}</div></div>;
  if (!f) return null;
  const step = Math.max(1, Math.round(f.points.length / 12));
  const pts = f.points.filter((_, i) => i % step === 0 || i === f.points.length - 1);
  const low = f.min_cents < threshold;

  return (
    <div className="page">
      <header className="top">
        <div><h1>Previsión</h1><div className="muted">Proyección de tu saldo total con los pagos periódicos{variable ? " y tu gasto habitual" : ""}.</div></div>
        <div className="seg">
          {[30, 60, 90, 180].map((d) => <button key={d} className={days === d ? "on" : ""} onClick={() => setDays(d)}>{d} días</button>)}
        </div>
      </header>

      {low && (
        <div className="banner warn"><AlertTriangle size={16} />
          <span><b>Tu saldo bajaría a {money(f.min_cents)} el {fmtDate(f.min_date)}.</b> Está por debajo de tu umbral de {money(threshold)}.</span>
        </div>
      )}

      <div className="kpi4">
        <div className="card"><div className="muted">Hoy</div><div className="v num">{money(f.start_cents)}</div></div>
        <div className="card"><div className="muted">En {days} días</div><div className={`v num ${f.end_cents < f.start_cents ? "exp" : "inc"}`}>{money(f.end_cents)}</div></div>
        <div className="card"><div className="muted">Punto más bajo</div><div className={`v num ${low ? "warn-text" : ""}`}>{money(f.min_cents)}</div><span className="pill">{fmtDate(f.min_date)}</span></div>
        <div className="card"><div className="muted">Gasto variable estimado</div><div className="v num">{money(f.daily_variable_cents * 30)}</div><span className="pill">al mes</span></div>
      </div>

      <div className="card">
        <h3>Saldo previsto
          <label className="check small"><input type="checkbox" checked={variable} onChange={(e) => setVariable(e.target.checked)} />Incluir gasto variable medio</label>
        </h3>
        <LineArea points={pts.map(([d, v]) => ({ label: fmtDate(d), value: v, tip: fmtDate(d) }))} labelEvery={2} height={240} />
        <div className="row-gap small muted">Umbral de aviso:
          <input className="input sm num" style={{ width: 110 }} defaultValue={(threshold / 100).toString()} onBlur={(e) => setThreshold(Math.round(Number(e.target.value.replace(",", ".")) * 100) || 0)} /> €
        </div>
      </div>

      <div className="card">
        <h3>Movimientos previstos <button className="link small" onClick={() => onNav("suscripciones")}>Gestionar recurrentes</button></h3>
        {f.events.length === 0 && <div className="muted small row-gap"><CalendarClock size={15} />No hay pagos periódicos. Se detectan solos al importar varios meses o puedes añadirlos en Suscripciones.</div>}
        {f.events.slice(0, 40).map((e, i) => (
          <div className="row" key={i}>
            <div className="date-badge"><b>{new Date(e.date + "T12:00").getDate()}</b><span>{fmtDate(e.date).split(" ")[1]}</span></div>
            <div className="t"><b>{e.name}</b></div>
            <div className={`a num ${e.amount_cents > 0 ? "inc" : "exp"}`}>{money(e.amount_cents, { sign: true })}</div>
          </div>
        ))}
      </div>
    </div>
  );
}
