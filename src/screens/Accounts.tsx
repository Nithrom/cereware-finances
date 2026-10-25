import { useCallback, useEffect, useState } from "react";
import { Plus, Upload, Landmark, Pencil, Trash2, Link2 } from "lucide-react";
import { api, errMsg, type Account, type AccountInput } from "../lib/api";
import { money, parseMoney } from "../lib/format";
import { Field, Modal, Toast } from "../components/ui";
import { ImportWizard } from "../components/ImportWizard";

const KINDS: Record<string, string> = { checking: "Cuenta corriente", savings: "Ahorro", card: "Tarjeta", cash: "Efectivo", investment: "Inversión" };
const PROVIDER: Record<string, string> = { manual: "Manual", import: "Extractos", enablebanking: "Conectada" };

export function Accounts({ onImported }: { onImported: () => void }) {
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [edit, setEdit] = useState<Account | "new" | null>(null);
  const [importFor, setImportFor] = useState<string | null | undefined>(undefined);
  const [toast, setToast] = useState<string | null>(null);
  const load = useCallback(() => api.accounts().then(setAccounts).catch(console.error), []);
  useEffect(() => { load(); }, [load]);

  const total = accounts.reduce((s, a) => s + a.balance_cents, 0);

  return (
    <div className="page">
      <header className="top">
        <div><h1>Cuentas y bancos</h1><div className="muted">{accounts.length} cuentas · <span className="num">{money(total)}</span></div></div>
        <div className="row-gap">
          <button className="btn g" onClick={() => setEdit("new")}><Plus size={16} />Cuenta manual</button>
          <button className="btn p" onClick={() => setImportFor(null)}><Upload size={16} />Importar extracto</button>
        </div>
      </header>

      <section className="card connect">
        <Link2 size={22} />
        <div className="grow"><b>Conexión directa con tu banco</b><div className="muted small">Sincronización automática de solo lectura (PSD2) con Openbank, BBVA, Revolut y más. Disponible próximamente; mientras tanto, importa extractos.</div></div>
        <button className="btn g" disabled>Conectar banco</button>
      </section>

      <div className="acc-grid">
        {accounts.map((a) => (
          <div key={a.id} className="card acc">
            <div className="acc-top">
              <span className="acc-logo" style={{ background: a.color || "var(--primary)" }}><Landmark size={18} /></span>
              <div className="grow"><b>{a.name}</b><div className="muted small">{a.institution} · {KINDS[a.kind] ?? a.kind}{a.is_business ? " · Autónomo" : ""}</div></div>
              <button className="icon-btn" onClick={() => setEdit(a)} aria-label="Editar"><Pencil size={16} /></button>
            </div>
            <div className="acc-bal num">{money(a.balance_cents)}</div>
            <div className="acc-foot">
              <span className="chip">{PROVIDER[a.provider] ?? a.provider}</span>
              <button className="link" onClick={() => setImportFor(a.id)}>Importar extracto</button>
            </div>
          </div>
        ))}
        {accounts.length === 0 && (
          <div className="card empty" style={{ gridColumn: "1/-1" }}>
            <h2>Aún no tienes cuentas</h2>
            <p className="muted">Importa un extracto (se crea la cuenta automáticamente) o añade una cuenta manual.</p>
          </div>
        )}
      </div>

      {edit && (
        <AccountForm account={edit === "new" ? null : edit} onClose={() => setEdit(null)}
          onSaved={(m) => { setEdit(null); setToast(m); load(); onImported(); }} />
      )}
      {importFor !== undefined && (
        <ImportWizard accounts={accounts} defaultAccountId={importFor ?? undefined} onClose={() => setImportFor(undefined)}
          onDone={() => { setImportFor(undefined); load(); onImported(); }} />
      )}
      {toast && <Toast msg={toast} onDone={() => setToast(null)} />}
    </div>
  );
}

const COLORS = ["#7F1D1D", "#E5383B", "#6C8EF5", "#2BB3A3", "#F5A524", "#A78BFA", "#F472B6", "#38BDF8", "#1F1A1C"];

function AccountForm({ account, onClose, onSaved }: { account: Account | null; onClose: () => void; onSaved: (msg: string) => void }) {
  const [f, setF] = useState({
    name: account?.name ?? "", institution: account?.institution ?? "", kind: account?.kind ?? "checking",
    balance: account ? (account.balance_cents / 100).toFixed(2).replace(".", ",") : "0,00",
    is_business: account?.is_business ?? false, color: account?.color ?? COLORS[0],
  });
  const [err, setErr] = useState<string | null>(null);
  const [confirmDel, setConfirmDel] = useState(false);

  async function save() {
    const bal = parseMoney(f.balance);
    if (bal === null) return setErr("Saldo no válido");
    const input: AccountInput = { name: f.name, institution: f.institution, kind: f.kind, balance_cents: bal, is_business: f.is_business, color: f.color };
    try {
      if (account) await api.accountUpdate(account.id, input);
      else await api.accountCreate(input);
      onSaved(account ? "Cuenta actualizada" : "Cuenta creada");
    } catch (e) { setErr(errMsg(e)); }
  }
  async function del() {
    if (!account) return;
    try { await api.accountDelete(account.id); onSaved("Cuenta eliminada"); } catch (e) { setErr(errMsg(e)); }
  }

  return (
    <Modal title={account ? "Editar cuenta" : "Nueva cuenta manual"} onClose={onClose}
      footer={<>
        {account && (confirmDel
          ? <button className="btn danger" onClick={del}><Trash2 size={16} />Borrar cuenta y sus movimientos</button>
          : <button className="btn g" onClick={() => setConfirmDel(true)}><Trash2 size={16} />Eliminar</button>)}
        <span className="grow" />
        <button className="btn g" onClick={onClose}>Cancelar</button>
        <button className="btn p" onClick={save}>Guardar</button>
      </>}>
      <div className="stack">
        <div className="row2">
          <Field label="Banco"><input className="input" value={f.institution} onChange={(e) => setF({ ...f, institution: e.target.value })} placeholder="Openbank" autoFocus /></Field>
          <Field label="Nombre"><input className="input" value={f.name} onChange={(e) => setF({ ...f, name: e.target.value })} placeholder="Cuenta nómina" /></Field>
        </div>
        <div className="row2">
          <Field label="Tipo">
            <select className="input" value={f.kind} onChange={(e) => setF({ ...f, kind: e.target.value })}>
              {Object.entries(KINDS).map(([k, v]) => <option key={k} value={k}>{v}</option>)}
            </select>
          </Field>
          <Field label="Saldo actual (€)"><input className="input num" value={f.balance} onChange={(e) => setF({ ...f, balance: e.target.value })} /></Field>
        </div>
        <Field label="Color">
          <div className="swatches">{COLORS.map((c) => (
            <button key={c} className={`swatch ${f.color === c ? "on" : ""}`} style={{ background: c }} onClick={() => setF({ ...f, color: c })} aria-label={c} />
          ))}</div>
        </Field>
        <label className="check"><input type="checkbox" checked={f.is_business} onChange={(e) => setF({ ...f, is_business: e.target.checked })} />Cuenta de autónomo (sus movimientos cuentan como profesionales)</label>
        {err && <div className="form-error">{err}</div>}
      </div>
    </Modal>
  );
}
