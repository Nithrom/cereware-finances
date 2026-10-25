import { useCallback, useEffect, useState } from "react";
import { Plus, TrendingUp, EyeOff, Eye, Sparkles, Pencil } from "lucide-react";
import { api, api2, errMsg, type Account, type Category, type Recurring, type RecurringInput } from "../lib/api";
import { fmtDate, isoToday, money, parseMoney } from "../lib/format";
import { CatIcon, CategoryPicker, Field, Modal, Toast } from "../components/ui";

const INTERVAL: Record<string, string> = { weekly: "Semanal", monthly: "Mensual", quarterly: "Trimestral", yearly: "Anual" };

export function Subscriptions() {
  const [items, setItems] = useState<Recurring[]>([]);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [cats, setCats] = useState<Category[]>([]);
  const [edit, setEdit] = useState<Recurring | "new" | null>(null);
  const [showInactive, setShowInactive] = useState(false);
  const [toast, setToast] = useState<string | null>(null);
  const load = useCallback(() => api2.recurring().then(setItems).catch((e) => setToast(errMsg(e))), []);
  useEffect(() => { load(); api.accounts().then(setAccounts); api.categories().then(setCats); }, [load]);

  const active = items.filter((r) => r.active);
  const subs = active.filter((r) => r.is_subscription && r.amount_cents < 0);
  const bills = active.filter((r) => !r.is_subscription && r.amount_cents < 0);
  const incomes = active.filter((r) => r.amount_cents > 0);
  const yearly = [...subs, ...bills].reduce((s, r) => s + r.yearly_cents, 0);
  const subsYearly = subs.reduce((s, r) => s + r.yearly_cents, 0);
  const increases = active.filter((r) => r.prev_amount_cents !== null && Math.abs(r.amount_cents) > Math.abs(r.prev_amount_cents!));

  async function toggle(r: Recurring) {
    await api2.recurringSetActive(r.id, !r.active);
    setToast(r.active ? `${r.name} oculto` : `${r.name} restaurado`);
    load();
  }

  const Section = ({ title, list }: { title: string; list: Recurring[] }) => list.length ? (
    <div className="card">
      <h3>{title} <small className="num">{money(Math.abs(list.reduce((s, r) => s + r.yearly_cents, 0)) / 12)}/mes</small></h3>
      {list.map((r) => (
        <div className="row" key={r.id}>
          <CatIcon icon={r.category_icon} color={r.category_color} />
          <div className="t">
            <b>{r.name} {r.source === "detected" && <Sparkles size={12} className="muted" aria-label="Detectado automáticamente" />}</b>
            <span>{INTERVAL[r.interval]} · próximo {fmtDate(r.next_date)}{r.account_name ? ` · ${r.account_name}` : ""}</span>
          </div>
          {r.prev_amount_cents !== null && Math.abs(r.amount_cents) > Math.abs(r.prev_amount_cents) && (
            <span className="pill warn"><TrendingUp size={12} /> antes {money(Math.abs(r.prev_amount_cents))}</span>
          )}
          <div className="r"><div className={`a num ${r.amount_cents > 0 ? "inc" : ""}`}>{money(r.amount_cents, { sign: true })}</div><div className="muted small num">{money(Math.abs(r.yearly_cents))}/año</div></div>
          <button className="icon-btn" onClick={() => setEdit(r)} aria-label="Editar"><Pencil size={15} /></button>
          <button className="icon-btn" onClick={() => toggle(r)} aria-label="Ocultar"><EyeOff size={15} /></button>
        </div>
      ))}
    </div>
  ) : null;

  return (
    <div className="page">
      <header className="top">
        <div><h1>Suscripciones y recibos</h1><div className="muted">Detectados automáticamente a partir de tus movimientos.</div></div>
        <button className="btn p" onClick={() => setEdit("new")}><Plus size={16} />Añadir</button>
      </header>

      <div className="kpi3">
        <div className="card"><div className="muted">Gasto fijo mensual</div><div className="v num">{money(Math.abs(yearly) / 12)}</div></div>
        <div className="card"><div className="muted">Suscripciones al año</div><div className="v num">{money(Math.abs(subsYearly))}</div><span className="pill">{subs.length} activas</span></div>
        <div className="card"><div className="muted">Subidas de precio</div><div className={`v num ${increases.length ? "warn-text" : ""}`}>{increases.length}</div></div>
      </div>

      {active.length === 0 && (
        <div className="card empty"><Sparkles size={26} className="muted" /><h2>Aún no hay pagos periódicos</h2><p className="muted">Se detectan solos cuando hay al menos 3 cargos parecidos con la misma frecuencia. También puedes añadirlos a mano (alquiler, IVA trimestral, nómina…).</p></div>
      )}
      <Section title="Suscripciones" list={subs} />
      <Section title="Recibos y pagos fijos" list={bills} />
      <Section title="Ingresos periódicos" list={incomes} />

      {items.some((r) => !r.active) && (
        <div>
          <button className="link" onClick={() => setShowInactive(!showInactive)}>{showInactive ? "Ocultar descartados" : `Ver descartados (${items.filter((r) => !r.active).length})`}</button>
          {showInactive && (
            <div className="card">
              {items.filter((r) => !r.active).map((r) => (
                <div className="row" key={r.id}>
                  <div className="t"><b className="muted">{r.name}</b><span>{INTERVAL[r.interval]} · {money(r.amount_cents)}</span></div>
                  <button className="btn g sm" onClick={() => toggle(r)}><Eye size={14} />Restaurar</button>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {edit && <RecurringForm item={edit === "new" ? null : edit} accounts={accounts} cats={cats} onClose={() => setEdit(null)}
        onSaved={() => { setEdit(null); load(); setToast("Guardado"); }} />}
      {toast && <Toast msg={toast} onDone={() => setToast(null)} />}
    </div>
  );
}

function RecurringForm({ item, accounts, cats, onClose, onSaved }: { item: Recurring | null; accounts: Account[]; cats: Category[]; onClose: () => void; onSaved: () => void }) {
  const [f, setF] = useState({
    name: item?.name ?? "", amount: item ? (Math.abs(item.amount_cents) / 100).toString().replace(".", ",") : "",
    type: item && item.amount_cents > 0 ? "income" : "expense", interval: item?.interval ?? "monthly",
    next_date: item?.next_date ?? isoToday(), account_id: (item?.account_id ?? accounts[0]?.id ?? null) as string | null,
    category_id: (item?.category_id ?? null) as string | null, is_subscription: item?.is_subscription ?? false,
  });
  const [picker, setPicker] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const c = cats.find((x) => x.id === f.category_id);
  async function save() {
    const v = parseMoney(f.amount);
    if (!v) return setErr("Importe no válido");
    const input: RecurringInput = {
      name: f.name, account_id: f.account_id, category_id: f.category_id, amount_cents: f.type === "expense" ? -Math.abs(v) : Math.abs(v),
      interval: f.interval as RecurringInput["interval"], next_date: f.next_date, is_subscription: f.is_subscription,
    };
    try { await api2.recurringUpsert(item?.id ?? null, input); onSaved(); } catch (e) { setErr(errMsg(e)); }
  }
  async function del() { if (item) { await api2.recurringDelete(item.id); onSaved(); } }
  return (
    <Modal title={item ? "Editar pago periódico" : "Nuevo pago periódico"} onClose={onClose}
      footer={<>{item && <button className="btn g" onClick={del}>Eliminar</button>}<span className="grow" /><button className="btn g" onClick={onClose}>Cancelar</button><button className="btn p" onClick={save}>Guardar</button></>}>
      <div className="stack">
        <div className="seg wide">
          <button className={f.type === "expense" ? "on" : ""} onClick={() => setF({ ...f, type: "expense" })}>Pago</button>
          <button className={f.type === "income" ? "on" : ""} onClick={() => setF({ ...f, type: "income" })}>Ingreso</button>
        </div>
        <Field label="Nombre"><input className="input" autoFocus value={f.name} onChange={(e) => setF({ ...f, name: e.target.value })} placeholder="Netflix, alquiler, IVA trimestral…" /></Field>
        <div className="row2">
          <Field label="Importe (€)"><input className="input num" value={f.amount} onChange={(e) => setF({ ...f, amount: e.target.value })} /></Field>
          <Field label="Frecuencia">
            <select className="input" value={f.interval} onChange={(e) => setF({ ...f, interval: e.target.value as Recurring["interval"] })}>
              {Object.entries(INTERVAL).map(([k, v]) => <option key={k} value={k}>{v}</option>)}
            </select>
          </Field>
        </div>
        <div className="row2">
          <Field label="Próxima fecha"><input type="date" className="input" value={f.next_date} onChange={(e) => setF({ ...f, next_date: e.target.value })} /></Field>
          <Field label="Cuenta">
            <select className="input" value={f.account_id ?? ""} onChange={(e) => setF({ ...f, account_id: e.target.value || null })}>
              <option value="">—</option>
              {accounts.map((a) => <option key={a.id} value={a.id}>{a.institution} · {a.name}</option>)}
            </select>
          </Field>
        </div>
        <Field label="Categoría">
          <div className="pill-wrap">
            <button className="input left" onClick={() => setPicker(!picker)}><CatIcon icon={c?.icon} color={c?.color} size={22} />{c?.name ?? "Sin categoría"}</button>
            {picker && <CategoryPicker categories={cats} value={f.category_id} onChange={(id) => { setF({ ...f, category_id: id }); setPicker(false); }} onClose={() => setPicker(false)} />}
          </div>
        </Field>
        <label className="check"><input type="checkbox" checked={f.is_subscription} onChange={(e) => setF({ ...f, is_subscription: e.target.checked })} />Es una suscripción (streaming, software…)</label>
        {err && <div className="form-error">{err}</div>}
      </div>
    </Modal>
  );
}
