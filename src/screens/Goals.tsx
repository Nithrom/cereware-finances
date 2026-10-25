import { useCallback, useEffect, useState } from "react";
import { Plus, PiggyBank, Pencil } from "lucide-react";
import { api2, errMsg, type Goal } from "../lib/api";
import { fmtDate, money, parseMoney } from "../lib/format";
import { Field, Modal, Toast } from "../components/ui";
import { Ring } from "../components/charts";

const COLORS = ["#E5383B", "#7F1D1D", "#6C8EF5", "#2BB3A3", "#F5A524", "#A78BFA", "#F472B6", "#38BDF8"];

export function Goals() {
  const [goals, setGoals] = useState<Goal[]>([]);
  const [edit, setEdit] = useState<Goal | "new" | null>(null);
  const [contrib, setContrib] = useState<Goal | null>(null);
  const [toast, setToast] = useState<string | null>(null);
  const load = useCallback(() => api2.goals().then(setGoals).catch((e) => setToast(errMsg(e))), []);
  useEffect(() => { load(); }, [load]);
  const saved = goals.reduce((s, g) => s + g.saved_cents, 0);
  const target = goals.reduce((s, g) => s + g.target_cents, 0);
  const monthly = goals.reduce((s, g) => s + (g.monthly_needed_cents ?? 0), 0);

  return (
    <div className="page">
      <header className="top">
        <div><h1>Huchas</h1><div className="muted">Objetivos de ahorro. El dinero sigue en tus cuentas; aquí lo apartas mentalmente.</div></div>
        <button className="btn p" onClick={() => setEdit("new")}><Plus size={16} />Nueva hucha</button>
      </header>
      {goals.length > 0 && (
        <div className="kpi3">
          <div className="card"><div className="muted">Ahorrado</div><div className="v num">{money(saved)}</div></div>
          <div className="card"><div className="muted">Objetivo total</div><div className="v num">{money(target)}</div></div>
          <div className="card"><div className="muted">Aportación mensual necesaria</div><div className="v num">{money(monthly)}</div></div>
        </div>
      )}
      <div className="acc-grid">
        {goals.map((g) => {
          const done = g.saved_cents >= g.target_cents;
          return (
            <div className="card goal-card" key={g.id}>
              <div className="acc-top">
                <Ring value={g.saved_cents} max={g.target_cents} color={g.color} size={64} />
                <div className="grow"><b>{g.name}</b><div className="muted small">{g.target_date ? `Para el ${fmtDate(g.target_date)}` : "Sin fecha"}</div></div>
                <button className="icon-btn" onClick={() => setEdit(g)} aria-label="Editar"><Pencil size={15} /></button>
              </div>
              <div className="num"><b className="lg">{money(g.saved_cents)}</b> <span className="muted">de {money(g.target_cents)}</span></div>
              <div className="muted small">{done ? "¡Objetivo conseguido! 🍒" : g.monthly_needed_cents ? `Aporta ${money(g.monthly_needed_cents)} al mes para llegar a tiempo` : `Faltan ${money(g.target_cents - g.saved_cents)}`}</div>
              <div className="row-gap">
                <button className="btn p sm" onClick={() => setContrib(g)}>Aportar</button>
              </div>
            </div>
          );
        })}
        {goals.length === 0 && (
          <div className="card empty" style={{ gridColumn: "1/-1" }}><PiggyBank size={28} className="muted" /><h2>Crea tu primera hucha</h2><p className="muted">Un viaje, un PC nuevo, un colchón para imprevistos… Te diremos cuánto apartar cada mes.</p></div>
        )}
      </div>
      {edit && <GoalForm goal={edit === "new" ? null : edit} onClose={() => setEdit(null)} onSaved={() => { setEdit(null); load(); }} />}
      {contrib && <ContributeForm goal={contrib} onClose={() => setContrib(null)} onSaved={(m) => { setContrib(null); setToast(m); load(); }} />}
      {toast && <Toast msg={toast} onDone={() => setToast(null)} />}
    </div>
  );
}

function GoalForm({ goal, onClose, onSaved }: { goal: Goal | null; onClose: () => void; onSaved: () => void }) {
  const [f, setF] = useState({
    name: goal?.name ?? "", target: goal ? (goal.target_cents / 100).toString().replace(".", ",") : "",
    saved: goal ? (goal.saved_cents / 100).toString().replace(".", ",") : "0", date: goal?.target_date ?? "", color: goal?.color ?? COLORS[0],
  });
  const [err, setErr] = useState<string | null>(null);
  async function save() {
    const t = parseMoney(f.target), s = parseMoney(f.saved) ?? 0;
    if (!t) return setErr("Objetivo no válido");
    try { await api2.goalUpsert(goal?.id ?? null, { name: f.name, target_cents: t, saved_cents: s, target_date: f.date || null, color: f.color }); onSaved(); }
    catch (e) { setErr(errMsg(e)); }
  }
  async function del() { if (goal) { await api2.goalDelete(goal.id); onSaved(); } }
  return (
    <Modal title={goal ? "Editar hucha" : "Nueva hucha"} onClose={onClose}
      footer={<>{goal && <button className="btn g" onClick={del}>Eliminar</button>}<span className="grow" /><button className="btn g" onClick={onClose}>Cancelar</button><button className="btn p" onClick={save}>Guardar</button></>}>
      <div className="stack">
        <Field label="Nombre"><input className="input" autoFocus value={f.name} onChange={(e) => setF({ ...f, name: e.target.value })} placeholder="PC nuevo" /></Field>
        <div className="row2">
          <Field label="Objetivo (€)"><input className="input num" value={f.target} onChange={(e) => setF({ ...f, target: e.target.value })} /></Field>
          <Field label="Ya ahorrado (€)"><input className="input num" value={f.saved} onChange={(e) => setF({ ...f, saved: e.target.value })} /></Field>
        </div>
        <Field label="Fecha objetivo (opcional)"><input type="date" className="input" value={f.date} onChange={(e) => setF({ ...f, date: e.target.value })} /></Field>
        <Field label="Color"><div className="swatches">{COLORS.map((c) => <button key={c} className={`swatch ${f.color === c ? "on" : ""}`} style={{ background: c }} onClick={() => setF({ ...f, color: c })} aria-label={c} />)}</div></Field>
        {err && <div className="form-error">{err}</div>}
      </div>
    </Modal>
  );
}

function ContributeForm({ goal, onClose, onSaved }: { goal: Goal; onClose: () => void; onSaved: (m: string) => void }) {
  const [v, setV] = useState(goal.monthly_needed_cents ? (goal.monthly_needed_cents / 100).toString().replace(".", ",") : "");
  const [withdraw, setWithdraw] = useState(false);
  async function save() {
    const c = parseMoney(v);
    if (!c) return;
    await api2.goalContribute(goal.id, withdraw ? -Math.abs(c) : Math.abs(c));
    onSaved(withdraw ? `Retirados ${money(c)} de ${goal.name}` : `Aportados ${money(c)} a ${goal.name}`);
  }
  return (
    <Modal title={goal.name} onClose={onClose} width={400}
      footer={<><button className="btn g" onClick={onClose}>Cancelar</button><button className="btn p" onClick={save}>{withdraw ? "Retirar" : "Aportar"}</button></>}>
      <div className="stack">
        <div className="seg wide">
          <button className={!withdraw ? "on" : ""} onClick={() => setWithdraw(false)}>Aportar</button>
          <button className={withdraw ? "on" : ""} onClick={() => setWithdraw(true)}>Retirar</button>
        </div>
        <Field label="Importe (€)"><input className="input num" autoFocus value={v} onChange={(e) => setV(e.target.value)} onKeyDown={(e) => e.key === "Enter" && save()} /></Field>
      </div>
    </Modal>
  );
}
