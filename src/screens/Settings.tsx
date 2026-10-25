import { useEffect, useState } from "react";
import { save, open } from "@tauri-apps/plugin-dialog";
import { ShieldCheck, Download, Upload, KeyRound, Tags, Wand2, Trash2, Plus, Landmark } from "lucide-react";
import { bankApi, type BankConfigView } from "../lib/api";
import { BankConfigForm } from "../components/BankConnect";
import { SyncSection } from "../components/SyncPanel";
import { bioDisable, bioEnable, bioEnabled } from "../lib/biometric";
import { invoke } from "@tauri-apps/api/core";
import { api, api2, catApi, errMsg, type Category, type RuleRow } from "../lib/api";
import { CatIcon, CategoryPicker, Field, Modal, Toast } from "../components/ui";
import { getAutolock, setAutolock } from "../lib/autolock";
import { autoScale, getScale, setScale, type Scale } from "../lib/uiscale";
import { Monitor } from "lucide-react";

export function Settings({ onRestored, mobile = false }: { onRestored: () => void; mobile?: boolean }) {
  const [toast, setToast] = useState<string | null>(null);
  const [pw, setPw] = useState(false);
  const [restore, setRestore] = useState<string | null>(null);
  const [lockMin, setLockMin] = useState(getAutolock());
  const [scale, setScaleState] = useState<Scale>(getScale());
  const [bio, setBio] = useState(bioEnabled());
  const [bioPw, setBioPw] = useState(false);
  const [rules, setRules] = useState<RuleRow[]>([]);
  const [cats, setCats] = useState<Category[]>([]);
  const [showSys, setShowSys] = useState(false);
  const [newRule, setNewRule] = useState<{ text: string; cat: string | null } | null>(null);
  const [catEdit, setCatEdit] = useState<Category | "new" | null>(null);
  const [bankCfg, setBankCfg] = useState<BankConfigView | null>(null);
  const [bankEdit, setBankEdit] = useState(false);
  useEffect(() => { bankApi.config().then(setBankCfg); }, []);

  const loadRules = () => api.rules().then(setRules);
  const loadCats = () => api.categories().then(setCats);
  useEffect(() => { loadRules(); loadCats(); }, []);

  async function backup() {
    const d = new Date().toISOString().slice(0, 10);
    const dest = await save({ defaultPath: `cereware-finances-${d}.cwbak`, filters: [{ name: "Copia de CereWare Finances", extensions: ["cwbak"] }] });
    if (!dest) return;
    try { await api2.backupExport(dest); setToast("Copia de seguridad guardada (cifrada con tu contraseña)"); } catch (e) { setToast(errMsg(e)); }
  }
  async function pickRestore() {
    const f = await open({ multiple: false, filters: [{ name: "Copia de CereWare Finances", extensions: ["cwbak", "cwf"] }] });
    if (typeof f === "string") setRestore(f);
  }

  async function importRules() {
    const f = await open({ multiple: false, filters: [{ name: "Reglas (CSV)", extensions: ["csv", "txt"] }] });
    if (typeof f !== "string") return;
    try {
      const [added, changed, unknown] = await catApi.rulesImport(f);
      loadRules();
      setToast(`${added} reglas importadas · ${changed} movimientos categorizados${unknown.length ? ` · ${unknown.length} con categoría desconocida` : ""}`);
    } catch (e) { setToast(errMsg(e)); }
  }

  const learned = rules.filter((r) => r.learned);
  const system = rules.filter((r) => !r.learned);

  return (
    <div className="page narrow">
      <header className="top"><h1>Ajustes</h1></header>

      <section className="card settings-sec">
        <h3><ShieldCheck size={18} />Seguridad</h3>
        <div className="set-row">
          <div><b>Contraseña</b><div className="muted small">Cifra todos tus datos. Si la olvidas no se pueden recuperar.</div></div>
          <button className="btn g" onClick={() => setPw(true)}><KeyRound size={16} />Cambiar</button>
        </div>
        {mobile && (
          <div className="set-row">
            <div><b>Desbloqueo con huella o cara</b><div className="muted small">La contraseña se guarda cifrada en el almacén seguro del móvil.</div></div>
            <button className="btn g" onClick={async () => { if (bio) { await bioDisable(); setBio(false); setToast("Desbloqueo biométrico desactivado"); } else setBioPw(true); }}>{bio ? "Desactivar" : "Activar"}</button>
          </div>
        )}
        <div className="set-row">
          <div><b>Bloqueo automático</b><div className="muted small">Bloquea la app tras un tiempo sin usarla.</div></div>
          <select className="input sm" style={{ width: 160 }} value={lockMin} onChange={(e) => { const v = Number(e.target.value); setLockMin(v); setAutolock(v); }}>
            {[0, 5, 10, 15, 30, 60].map((m) => <option key={m} value={m}>{m === 0 ? "Nunca" : `${m} minutos`}</option>)}
          </select>
        </div>
      </section>

      <section className="card settings-sec">
        <h3><Monitor size={18} />Apariencia</h3>
        <div className="set-row">
          <div><b>Tamaño de la interfaz</b><div className="muted small">Automático según tu pantalla ({Math.round(autoScale() * 100)} % en esta).</div></div>
          <select className="input sm" style={{ width: 160 }} value={String(scale)} onChange={(e) => { const v: Scale = e.target.value === "auto" ? "auto" : Number(e.target.value); setScaleState(v); setScale(v); }}>
            <option value="auto">Automático</option>
            {[0.9, 1, 1.1, 1.25, 1.5].map((z) => <option key={z} value={String(z)}>{Math.round(z * 100)} %</option>)}
          </select>
        </div>
      </section>

      <section className="card settings-sec">
        <h3><Download size={18} />Copias de seguridad</h3>
        <div className="set-row">
          <div><b>Exportar copia</b><div className="muted small">Fichero .cwbak cifrado. Guárdalo en un USB o en la nube.</div></div>
          <button className="btn g" onClick={backup}><Download size={16} />Exportar</button>
        </div>
        <div className="set-row">
          <div><b>Restaurar copia</b><div className="muted small">Sustituye todos los datos actuales por los de la copia.</div></div>
          <button className="btn g" onClick={pickRestore}><Upload size={16} />Restaurar</button>
        </div>
      </section>

      <SyncSection onToast={setToast} mobile={mobile} />

      <section className="card settings-sec">
        <h3><Landmark size={18} />Conexión bancaria</h3>
        <div className="set-row">
          <div><b>Enable Banking</b><div className="muted small">{bankCfg?.has_key ? `Configurado · ${bankCfg.app_id.slice(0, 8)}…` : "Sin configurar. Necesario para sincronizar con tu banco."}</div></div>
          <button className="btn g" onClick={() => setBankEdit(true)}><KeyRound size={16} />{bankCfg?.has_key ? "Editar" : "Configurar"}</button>
        </div>
      </section>

      <section className="card settings-sec">
        <h3><Tags size={18} />Categorías <button className="btn g sm ml" onClick={() => setCatEdit("new")}><Plus size={14} />Nueva</button></h3>
        <div className="cat-grid">
          {cats.filter((c) => !c.parent_id).map((p) => (
            <div key={p.id} className="cat-group">
              <button className="cat-chip parent" onClick={() => setCatEdit(p)}><CatIcon icon={p.icon} color={p.color} size={24} />{p.name}</button>
              {cats.filter((c) => c.parent_id === p.id).map((c) => <button key={c.id} className="cat-chip" onClick={() => setCatEdit(c)}>{c.name}</button>)}
            </div>
          ))}
        </div>
      </section>

      <section className="card settings-sec">
        <h3><Wand2 size={18} />Reglas de categorización
          <button className="btn g sm ml" onClick={importRules}><Upload size={14} />Importar</button>
          <button className="btn g sm" onClick={() => setNewRule({ text: "", cat: null })}><Plus size={14} />Nueva regla</button></h3>
        <div className="muted small">Las reglas aprendidas se crean al categorizar un movimiento con "Aprender" activado. Tienen prioridad sobre las del sistema.</div>
        {learned.length === 0 && <div className="muted small pad">Aún no hay reglas aprendidas.</div>}
        {learned.map((r) => (
          <div className="row" key={r.id}>
            <div className="t"><b>"{r.match_value}"</b><span>→ {r.category_name ?? "—"}</span></div>
            <button className="icon-btn" onClick={async () => { await api.ruleDelete(r.id); loadRules(); }} aria-label="Eliminar"><Trash2 size={15} /></button>
          </div>
        ))}
        <button className="link" onClick={() => setShowSys(!showSys)}>{showSys ? "Ocultar" : "Ver"} reglas del sistema ({system.length})</button>
        {showSys && <div className="sys-rules">{system.map((r) => <span key={r.id} className="chip">{r.match_value} → {r.category_name}</span>)}</div>}
      </section>

      <div className="muted small center">CereWare Finances 0.1.0 · Tus datos solo se guardan en este equipo.</div>

      {pw && <PasswordForm onClose={() => setPw(false)} onDone={async (newPw) => {
        setPw(false); setToast("Contraseña cambiada");
        if (mobile && bioEnabled()) { try { await bioEnable(newPw); } catch { await bioDisable(); setBio(false); } }
      }} />}
      {bioPw && <BioForm onClose={() => setBioPw(false)} onDone={() => { setBioPw(false); setBio(true); setToast("Desbloqueo con huella activado"); }} />}
      {restore && <RestoreForm path={restore} onClose={() => setRestore(null)} onDone={() => { setRestore(null); onRestored(); }} />}
      {newRule && (
        <Modal title="Nueva regla" onClose={() => setNewRule(null)} footer={<><button className="btn g" onClick={() => setNewRule(null)}>Cancelar</button>
          <button className="btn p" disabled={!newRule.cat || newRule.text.trim().length < 2} onClick={async () => {
            try { const n = await api.ruleCreate(newRule.text, newRule.cat!); setNewRule(null); loadRules(); setToast(`Regla creada · ${n} movimientos recategorizados`); } catch (e) { setToast(errMsg(e)); }
          }}>Crear</button></>}>
          <div className="stack">
            <Field label="Si el concepto contiene las palabras" hint="Sin distinguir mayúsculas ni acentos. Todas las palabras deben aparecer.">
              <input className="input" autoFocus value={newRule.text} onChange={(e) => setNewRule({ ...newRule, text: e.target.value })} placeholder="PELUQUERIA LOLA" />
            </Field>
            <Field label="Asignar la categoría">
              <RulePicker cats={cats} value={newRule.cat} onChange={(c) => setNewRule({ ...newRule, cat: c })} />
            </Field>
          </div>
        </Modal>
      )}
      {catEdit && <CategoryForm cat={catEdit === "new" ? null : catEdit} cats={cats} onClose={() => setCatEdit(null)} onSaved={() => { setCatEdit(null); loadCats(); }} />}
      {bankEdit && <BankConfigForm cfg={bankCfg} onClose={() => setBankEdit(false)} onSaved={() => { setBankEdit(false); bankApi.config().then(setBankCfg); setToast("Credenciales guardadas"); }} />}
      {toast && <Toast msg={toast} onDone={() => setToast(null)} />}
    </div>
  );
}

function RulePicker({ cats, value, onChange }: { cats: Category[]; value: string | null; onChange: (id: string | null) => void }) {
  const [open, setOpen] = useState(false);
  const c = cats.find((x) => x.id === value);
  return (
    <div className="pill-wrap">
      <button className="input left" onClick={() => setOpen(!open)}><CatIcon icon={c?.icon} color={c?.color} size={22} />{c?.name ?? "Elige categoría"}</button>
      {open && <CategoryPicker categories={cats} value={value} allowNone={false} onChange={(id) => { onChange(id); setOpen(false); }} onClose={() => setOpen(false)} />}
    </div>
  );
}

function BioForm({ onClose, onDone }: { onClose: () => void; onDone: () => void }) {
  const [pw, setPw] = useState("");
  const [err, setErr] = useState<string | null>(null);
  async function go() {
    try {
      if (!(await invoke<boolean>("vault_verify", { password: pw }))) return setErr("Contraseña incorrecta");
      await bioEnable(pw); onDone();
    }
    catch (e) { setErr(errMsg(e)); }
  }
  return (
    <Modal title="Activar huella o cara" onClose={onClose} width={420} footer={<><button className="btn g" onClick={onClose}>Cancelar</button><button className="btn p" onClick={go} disabled={!pw}>Activar</button></>}>
      <div className="stack">
        <Field label="Confirma tu contraseña"><input type="password" className="input" autoFocus value={pw} onChange={(e) => setPw(e.target.value)} /></Field>
        {err && <div className="form-error">{err}</div>}
      </div>
    </Modal>
  );
}

function PasswordForm({ onClose, onDone }: { onClose: () => void; onDone: (newPassword: string) => void }) {
  const [f, setF] = useState({ cur: "", n1: "", n2: "" });
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  async function go() {
    if (f.n1 !== f.n2) return setErr("Las contraseñas nuevas no coinciden");
    setBusy(true);
    try { await api2.changePassword(f.cur, f.n1); onDone(f.n1); } catch (e) { setErr(errMsg(e)); setBusy(false); }
  }
  return (
    <Modal title="Cambiar contraseña" onClose={onClose} width={420} footer={<><button className="btn g" onClick={onClose}>Cancelar</button><button className="btn p" onClick={go} disabled={busy}>{busy ? "Cifrando…" : "Cambiar"}</button></>}>
      <div className="stack">
        <Field label="Contraseña actual"><input type="password" className="input" autoFocus value={f.cur} onChange={(e) => setF({ ...f, cur: e.target.value })} /></Field>
        <Field label="Nueva contraseña" hint="Mínimo 8 caracteres."><input type="password" className="input" value={f.n1} onChange={(e) => setF({ ...f, n1: e.target.value })} /></Field>
        <Field label="Repite la nueva"><input type="password" className="input" value={f.n2} onChange={(e) => setF({ ...f, n2: e.target.value })} /></Field>
        {err && <div className="form-error">{err}</div>}
      </div>
    </Modal>
  );
}

function RestoreForm({ path, onClose, onDone }: { path: string; onClose: () => void; onDone: () => void }) {
  const [pw, setPw] = useState("");
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  async function go() {
    setBusy(true);
    try { await api2.backupRestore(path, pw); onDone(); } catch (e) { setErr(errMsg(e)); setBusy(false); }
  }
  return (
    <Modal title="Restaurar copia" onClose={onClose} width={440} footer={<><button className="btn g" onClick={onClose}>Cancelar</button><button className="btn danger" onClick={go} disabled={busy || !pw}>{busy ? "Restaurando…" : "Restaurar y sustituir"}</button></>}>
      <div className="stack">
        <div className="banner warn">Se sustituirán todos los datos actuales. Se guarda una copia del estado actual junto al almacén (.bak).</div>
        <div className="muted small ellipsis">{path}</div>
        <Field label="Contraseña de la copia"><input type="password" className="input" autoFocus value={pw} onChange={(e) => setPw(e.target.value)} /></Field>
        {err && <div className="form-error">{err}</div>}
      </div>
    </Modal>
  );
}

const CAT_COLORS = ["#6C8EF5", "#A78BFA", "#F5A524", "#2BB3A3", "#F472B6", "#FB923C", "#38BDF8", "#94A3B8", "#15803D", "#E5383B", "#7F1D1D"];
const CAT_ICONS = ["home", "shopping-cart", "utensils", "car", "gamepad-2", "shopping-bag", "heart-pulse", "briefcase", "plane", "wallet", "receipt", "package", "cpu", "shirt", "zap", "wifi", "repeat", "circle-ellipsis"];

function CategoryForm({ cat, cats, onClose, onSaved }: { cat: Category | null; cats: Category[]; onClose: () => void; onSaved: () => void }) {
  const [f, setF] = useState({ name: cat?.name ?? "", parent_id: cat?.parent_id ?? null as string | null, kind: cat?.kind ?? "expense", color: cat?.color ?? CAT_COLORS[0], icon: cat?.icon ?? "circle-ellipsis" });
  const [err, setErr] = useState<string | null>(null);
  async function go() {
    const parent = cats.find((c) => c.id === f.parent_id);
    try { await api2.categoryUpsert(cat?.id ?? null, { ...f, kind: parent?.kind ?? f.kind, color: parent && !cat ? parent.color : f.color }); onSaved(); } catch (e) { setErr(errMsg(e)); }
  }
  async function del() { if (cat) { await api2.categoryDelete(cat.id); onSaved(); } }
  return (
    <Modal title={cat ? "Editar categoría" : "Nueva categoría"} onClose={onClose}
      footer={<>{cat && <button className="btn g" onClick={del}><Trash2 size={15} />Eliminar</button>}<span className="grow" /><button className="btn g" onClick={onClose}>Cancelar</button><button className="btn p" onClick={go}>Guardar</button></>}>
      <div className="stack">
        <Field label="Nombre"><input className="input" autoFocus value={f.name} onChange={(e) => setF({ ...f, name: e.target.value })} /></Field>
        {!cat && (
          <div className="row2">
            <Field label="Dentro de">
              <select className="input" value={f.parent_id ?? ""} onChange={(e) => setF({ ...f, parent_id: e.target.value || null })}>
                <option value="">— Categoría principal —</option>
                {cats.filter((c) => !c.parent_id).map((c) => <option key={c.id} value={c.id}>{c.name}</option>)}
              </select>
            </Field>
            {!f.parent_id && (
              <Field label="Tipo">
                <select className="input" value={f.kind} onChange={(e) => setF({ ...f, kind: e.target.value })}>
                  <option value="expense">Gasto</option><option value="income">Ingreso</option>
                </select>
              </Field>
            )}
          </div>
        )}
        <Field label="Color"><div className="swatches">{CAT_COLORS.map((c) => <button key={c} className={`swatch ${f.color === c ? "on" : ""}`} style={{ background: c }} onClick={() => setF({ ...f, color: c })} aria-label={c} />)}</div></Field>
        <Field label="Icono"><div className="swatches">{CAT_ICONS.map((i) => <button key={i} className={`icon-pick ${f.icon === i ? "on" : ""}`} onClick={() => setF({ ...f, icon: i })}><CatIcon icon={i} color={f.color} size={30} /></button>)}</div></Field>
        {err && <div className="form-error">{err}</div>}
      </div>
    </Modal>
  );
}
