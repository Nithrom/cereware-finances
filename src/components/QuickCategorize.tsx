import { useEffect, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { Download, Sparkles } from "lucide-react";
import { api2, catApi, errMsg, type Category, type UncatGroup } from "../lib/api";
import { money } from "../lib/format";
import { CategoryPicker, Modal, Toast } from "./ui";

/** Categoriza los movimientos sin categoría por grupos de comercio; cada asignación crea una regla. */
export function QuickCategorize({ cats, onClose }: { cats: Category[]; onClose: () => void }) {
  const [groups, setGroups] = useState<UncatGroup[] | null>(null);
  const [picker, setPicker] = useState<string | null>(null);
  const [toast, setToast] = useState<string | null>(null);
  const load = () => catApi.groups().then(setGroups).catch((e) => setToast(errMsg(e)));
  useEffect(() => { load(); }, []);

  const total = groups?.reduce((s, g) => s + g.count, 0) ?? 0;
  const id = (g: UncatGroup) => `${g.key}|${g.income}`;

  async function assign(g: UncatGroup, cat: string | null) {
    setPicker(null);
    if (!cat) return;
    try {
      const n = await catApi.categorizeGroup(g.key, g.income, cat);
      setToast(`${n} movimientos categorizados · regla "${g.key}" guardada`);
      load();
    } catch (e) { setToast(errMsg(e)); }
  }
  async function exportCsv() {
    const dest = await save({ defaultPath: "movimientos-sin-categoria.csv", filters: [{ name: "CSV", extensions: ["csv"] }] });
    if (!dest) return;
    try { const n = await api2.exportCsv(dest, { kind: "uncategorized" }); setToast(`${n} movimientos exportados`); } catch (e) { setToast(errMsg(e)); }
  }

  return (
    <Modal title="Categorización rápida" onClose={onClose} width={760}
      footer={<><button className="btn g" onClick={exportCsv}><Download size={16} />Exportar sin categoría (CSV)</button><span className="grow" /><button className="btn p" onClick={onClose}>Hecho</button></>}>
      <div className="stack">
        <div className="muted small"><Sparkles size={13} /> {total} movimientos sin categoría agrupados por comercio. Al elegir una categoría se aplica a todo el grupo y se crea una regla para el futuro.</div>
        <div className="quick-list">
          {groups?.map((g) => (
            <div key={id(g)}>
            <div className="row">
              <span className="count-badge num">{g.count}</span>
              <div className="t"><b className="ellipsis">{g.key === "(otros)" ? "Otros (sin comercio reconocible)" : g.key}</b><span className="ellipsis">{g.sample}</span></div>
              <div className={`a num ${g.income ? "inc" : "exp"}`}>{money(g.total_cents, { sign: true })}</div>
              <button className="cat-pill none" disabled={g.key === "(otros)"} onClick={() => setPicker(picker === id(g) ? null : id(g))}>Elegir categoría</button>
            </div>
            {picker === id(g) && <div className="inline-picker"><CategoryPicker categories={cats} sign={g.income ? 1 : -1} allowNone={false} value={null} onChange={(c) => assign(g, c)} onClose={() => setPicker(null)} /></div>}
            </div>
          ))}
          {groups && groups.length === 0 && <div className="empty"><h2>¡Todo categorizado! 🍒</h2></div>}
        </div>
      </div>
      {toast && <Toast msg={toast} onDone={() => setToast(null)} />}
    </Modal>
  );
}
