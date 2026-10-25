import { useCallback, useEffect, useMemo, useState } from "react";
import { Plus, Search, Upload, Briefcase, Trash2, ArrowLeftRight, Sparkles } from "lucide-react";
import { api, errMsg, type Account, type Category, type TxFilter, type TxPage, type TxRow } from "../lib/api";
import { fmtDate, isoToday, money, parseMoney } from "../lib/format";
import { CatIcon, CategoryPicker, Field, Modal, Toast } from "../components/ui";
import { ImportWizard } from "../components/ImportWizard";
import { QuickCategorize } from "../components/QuickCategorize";

const KIND_CHIPS: { id: TxFilter["kind"]; label: string }[] = [
  { id: undefined, label: "Todos" },
  { id: "expense", label: "Gastos" },
  { id: "income", label: "Ingresos" },
  { id: "uncategorized", label: "Sin categoría" },
  { id: "business", label: "Autónomo" },
  { id: "transfer", label: "Transferencias" },
];
const PAGE = 100;

export function Transactions({ preset, onChanged }: { preset?: TxFilter; onChanged: () => void }) {
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [cats, setCats] = useState<Category[]>([]);
  const [filter, setFilter] = useState<TxFilter>(preset ?? {});
  const [search, setSearch] = useState("");
  const [page, setPage] = useState<TxPage | null>(null);
  const [limit, setLimit] = useState(PAGE);
  const [picker, setPicker] = useState<string | null>(null);
  const [learn, setLearn] = useState(true);
  const [detail, setDetail] = useState<TxRow | null>(null);
  const [creating, setCreating] = useState(false);
  const [importing, setImporting] = useState(false);
  const [toast, setToast] = useState<string | null>(null);
  const [quick, setQuick] = useState(false);
  const [showFilters, setShowFilters] = useState(false);
  const activeFilters = [filter.account_id, filter.category_id, filter.from, filter.to].filter(Boolean).length;

  useEffect(() => {
    api.accounts().then(setAccounts);
    api.categories().then(setCats);
  }, []);
  useEffect(() => { const t = setTimeout(() => setFilter((f) => ({ ...f, search: search || undefined })), 250); return () => clearTimeout(t); }, [search]);
  const load = useCallback(() => api.txList({ ...filter, limit }).then(setPage).catch((e) => setToast(errMsg(e))), [filter, limit]);
  useEffect(() => { load(); }, [load]);

  const catById = useMemo(() => Object.fromEntries(cats.map((c) => [c.id, c])), [cats]);
  const changed = () => { load(); onChanged(); };

  async function setCategory(t: TxRow, id: string | null) {
    setPicker(null);
    try {
      const r = await api.txUpdate(t.id, { category_id: id, learn: learn && id !== null });
      setToast(r.learned_applied > 0 ? `Categoría aplicada también a ${r.learned_applied} movimientos similares` : "Categoría actualizada");
      changed();
    } catch (e) { setToast(errMsg(e)); }
  }

  // Agrupa por día
  const groups = useMemo(() => {
    const g: { date: string; items: TxRow[]; sum: number }[] = [];
    for (const t of page?.items ?? []) {
      const last = g[g.length - 1];
      if (last && last.date === t.date) { last.items.push(t); last.sum += t.amount_cents; }
      else g.push({ date: t.date, items: [t], sum: t.amount_cents });
    }
    return g;
  }, [page]);

  return (
    <div className="page">
      <header className="top">
        <div><h1>Movimientos</h1>
          {page && <div className="muted num">{page.total} movimientos · <span className="inc">{money(page.income_cents, { sign: true })}</span> · <span className="exp">{money(page.expense_cents)}</span></div>}
        </div>
        <div className="row-gap">
          <button className="btn g" onClick={() => setQuick(true)} disabled={!accounts.length}><Sparkles size={16} />Categorización rápida</button>
          <button className="btn g" onClick={() => setImporting(true)} disabled={!accounts.length}><Upload size={16} />Importar</button>
          <button className="btn p" onClick={() => setCreating(true)} disabled={!accounts.length}><Plus size={16} />Añadir</button>
        </div>
      </header>

      <div className={`filters ${showFilters ? "open" : ""}`}>
        <div className="search"><Search size={16} /><input placeholder="Buscar concepto, nota o categoría" value={search} onChange={(e) => setSearch(e.target.value)} enterKeyHint="search" /></div>
        <button className="btn g filters-toggle" onClick={() => setShowFilters(!showFilters)}>Filtros{activeFilters ? ` (${activeFilters})` : ""}</button>
        <select className="input sm" value={filter.account_id ?? ""} onChange={(e) => setFilter({ ...filter, account_id: e.target.value || undefined })}>
          <option value="">Todas las cuentas</option>
          {accounts.map((a) => <option key={a.id} value={a.id}>{a.institution} · {a.name}</option>)}
        </select>
        <select className="input sm" value={filter.category_id ?? ""} onChange={(e) => setFilter({ ...filter, category_id: e.target.value || undefined })}>
          <option value="">Todas las categorías</option>
          {cats.filter((c) => !c.parent_id).map((p) => (
            <optgroup key={p.id} label={p.name}>
              <option value={p.id}>{p.name} (todo)</option>
              {cats.filter((c) => c.parent_id === p.id).map((c) => <option key={c.id} value={c.id}>{c.name}</option>)}
            </optgroup>
          ))}
        </select>
        <input type="date" className="input sm" value={filter.from ?? ""} onChange={(e) => setFilter({ ...filter, from: e.target.value || undefined })} aria-label="Desde" />
        <input type="date" className="input sm" value={filter.to ?? ""} onChange={(e) => setFilter({ ...filter, to: e.target.value || undefined })} aria-label="Hasta" />
      </div>
      <div className="chips">
        {KIND_CHIPS.map((k) => (
          <button key={k.label} className={`chip-btn ${filter.kind === k.id ? "on" : ""}`} onClick={() => setFilter({ ...filter, kind: k.id })}>{k.label}</button>
        ))}
        <label className="check small ml"><input type="checkbox" checked={learn} onChange={(e) => setLearn(e.target.checked)} /><Sparkles size={14} />Aprender al categorizar</label>
      </div>

      <div className="card tx-list">
        {groups.map((g) => (
          <div key={g.date}>
            <div className="day"><span>{fmtDate(g.date)}</span><span className="num">{money(g.sum, { sign: true })}</span></div>
            {g.items.map((t) => {
              const cat = t.category_id ? catById[t.category_id] : null;
              return (
                <div key={t.id} className="tx" onClick={() => setDetail(t)}>
                  <CatIcon icon={t.is_transfer ? "arrow-left-right" : cat?.icon} color={cat?.color} />
                  <div className="tx-main">
                    <b className="ellipsis">{t.description}</b>
                    <span className="muted small ellipsis">{t.institution} · {t.account_name}{t.notes ? ` · ${t.notes}` : ""}</span>
                  </div>
                  {t.is_business && <Briefcase size={15} className="muted" aria-label="Autónomo" />}
                  <div className="pill-wrap" onClick={(e) => e.stopPropagation()}>
                    <button className={`cat-pill ${cat ? "" : "none"}`} onClick={() => setPicker(picker === t.id ? null : t.id)}
                      style={cat ? { color: cat.color, background: `color-mix(in srgb, ${cat.color} 14%, transparent)` } : undefined}>
                      {cat?.name ?? "Categorizar"}
                    </button>
                    {picker === t.id && <CategoryPicker categories={cats} sign={t.amount_cents} value={t.category_id} onChange={(id) => setCategory(t, id)} onClose={() => setPicker(null)} />}
                  </div>
                  <div className={`amt num ${t.is_transfer ? "muted" : t.amount_cents > 0 ? "inc" : "exp"}`}>{money(t.amount_cents, { sign: true })}</div>
                </div>
              );
            })}
          </div>
        ))}
        {page && page.items.length === 0 && (
          <div className="empty"><h2>No hay movimientos</h2><p className="muted">{accounts.length ? "Prueba con otros filtros o importa un extracto." : "Primero crea una cuenta o importa un extracto desde Cuentas y bancos."}</p></div>
        )}
        {page && page.items.length < page.total && (
          <div className="more"><button className="btn g" onClick={() => setLimit(limit + PAGE)}>Cargar más ({page.total - page.items.length})</button></div>
        )}
      </div>

      {detail && <TxDetail tx={detail} cats={cats} onClose={() => setDetail(null)} onSaved={(m) => { setDetail(null); setToast(m); changed(); }} />}
      {creating && <TxForm accounts={accounts} cats={cats} onClose={() => setCreating(false)} onSaved={() => { setCreating(false); setToast("Movimiento añadido"); changed(); }} />}
      {importing && <ImportWizard accounts={accounts} defaultAccountId={filter.account_id} onClose={() => setImporting(false)} onDone={() => { setImporting(false); changed(); }} />}
      {quick && <QuickCategorize cats={cats} onClose={() => { setQuick(false); changed(); }} />}
      {toast && <Toast msg={toast} onDone={() => setToast(null)} />}
    </div>
  );
}


function TxDetail({ tx, cats, onClose, onSaved }: { tx: TxRow; cats: Category[]; onClose: () => void; onSaved: (m: string) => void }) {
  const [notes, setNotes] = useState(tx.notes ?? "");
  const [business, setBusiness] = useState(tx.is_business);
  const [cat, setCat] = useState(tx.category_id);
  const [picker, setPicker] = useState(false);
  const [confirm, setConfirm] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const c = cats.find((x) => x.id === cat);

  async function save() {
    try {
      await api.txUpdate(tx.id, {
        notes, is_business: business,
        ...(cat !== tx.category_id ? { category_id: cat, learn: cat !== null } : {}),
      });
      onSaved("Movimiento guardado");
    } catch (e) { setErr(errMsg(e)); }
  }
  async function del() {
    try { await api.txDelete(tx.id, true); onSaved("Movimiento eliminado"); } catch (e) { setErr(errMsg(e)); }
  }

  return (
    <Modal title="Movimiento" onClose={onClose}
      footer={<>
        {confirm ? <button className="btn danger" onClick={del}><Trash2 size={16} />Confirmar borrado</button>
          : <button className="btn g" onClick={() => setConfirm(true)}><Trash2 size={16} />Eliminar</button>}
        <span className="grow" />
        <button className="btn g" onClick={onClose}>Cancelar</button>
        <button className="btn p" onClick={save}>Guardar</button>
      </>}>
      <div className="stack">
        <div className="tx-head">
          <div className={`big num ${tx.amount_cents > 0 ? "inc" : "exp"}`}>{money(tx.amount_cents, { sign: true })}</div>
          <div><b>{tx.description}</b><div className="muted small">{fmtDate(tx.date)} · {tx.institution} · {tx.account_name}</div></div>
          {tx.is_transfer && <span className="chip"><ArrowLeftRight size={12} /> Transferencia interna</span>}
        </div>
        <Field label="Categoría">
          <div className="pill-wrap">
            <button className="input left" onClick={() => setPicker(!picker)}>
              <CatIcon icon={c?.icon} color={c?.color} size={22} />{c?.name ?? "Sin categoría"}
            </button>
            {picker && <CategoryPicker categories={cats} sign={tx.amount_cents} value={cat} onChange={(id) => { setCat(id); setPicker(false); }} onClose={() => setPicker(false)} />}
          </div>
        </Field>
        <Field label="Notas"><textarea className="input" rows={2} value={notes} onChange={(e) => setNotes(e.target.value)} placeholder="Añade una nota…" /></Field>
        <label className="check"><input type="checkbox" checked={business} onChange={(e) => setBusiness(e.target.checked)} />Movimiento de autónomo</label>
        {tx.category_source === "rule" && <div className="muted small"><Sparkles size={12} /> Categorizado automáticamente por una regla.</div>}
        {err && <div className="form-error">{err}</div>}
      </div>
    </Modal>
  );
}

function TxForm({ accounts, cats, onClose, onSaved }: { accounts: Account[]; cats: Category[]; onClose: () => void; onSaved: () => void }) {
  const [f, setF] = useState({ account_id: accounts[0]?.id ?? "", date: isoToday(), amount: "", type: "expense", description: "", category_id: null as string | null });
  const [picker, setPicker] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const c = cats.find((x) => x.id === f.category_id);
  async function save() {
    const v = parseMoney(f.amount);
    if (!v) return setErr("Importe no válido");
    try {
      await api.txCreate({ account_id: f.account_id, date: f.date, amount_cents: f.type === "expense" ? -Math.abs(v) : Math.abs(v), description: f.description, category_id: f.category_id });
      onSaved();
    } catch (e) { setErr(errMsg(e)); }
  }
  return (
    <Modal title="Añadir movimiento" onClose={onClose}
      footer={<><button className="btn g" onClick={onClose}>Cancelar</button><button className="btn p" onClick={save}>Añadir</button></>}>
      <div className="stack">
        <div className="seg wide">
          <button className={f.type === "expense" ? "on" : ""} onClick={() => setF({ ...f, type: "expense" })}>Gasto</button>
          <button className={f.type === "income" ? "on" : ""} onClick={() => setF({ ...f, type: "income" })}>Ingreso</button>
        </div>
        <div className="row2">
          <Field label="Importe (€)"><input className="input num" autoFocus value={f.amount} onChange={(e) => setF({ ...f, amount: e.target.value })} placeholder="0,00" /></Field>
          <Field label="Fecha"><input type="date" className="input" value={f.date} onChange={(e) => setF({ ...f, date: e.target.value })} /></Field>
        </div>
        <Field label="Concepto"><input className="input" value={f.description} onChange={(e) => setF({ ...f, description: e.target.value })} placeholder="Ej. Cena con amigos" /></Field>
        <div className="row2">
          <Field label="Cuenta">
            <select className="input" value={f.account_id} onChange={(e) => setF({ ...f, account_id: e.target.value })}>
              {accounts.map((a) => <option key={a.id} value={a.id}>{a.institution} · {a.name}</option>)}
            </select>
          </Field>
          <Field label="Categoría">
            <div className="pill-wrap">
              <button className="input left" onClick={() => setPicker(!picker)}><CatIcon icon={c?.icon} color={c?.color} size={22} />{c?.name ?? "Sin categoría"}</button>
              {picker && <CategoryPicker categories={cats} sign={f.type === "income" ? 1 : -1} value={f.category_id} onChange={(id) => { setF({ ...f, category_id: id }); setPicker(false); }} onClose={() => setPicker(false)} />}
            </div>
          </Field>
        </div>
        {err && <div className="form-error">{err}</div>}
      </div>
    </Modal>
  );
}
