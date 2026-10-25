import { useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { FileUp, CheckCircle2, AlertTriangle } from "lucide-react";
import { api, errMsg, type Account, type ImportResult, type Mapping, type Preview } from "../lib/api";
import { fmtDate, money } from "../lib/format";
import { Field, Modal } from "./ui";

const FIELDS: { key: keyof Mapping; label: string }[] = [
  { key: "date", label: "Fecha" },
  { key: "description", label: "Concepto" },
  { key: "detail", label: "Concepto adicional" },
  { key: "amount", label: "Importe" },
  { key: "debit", label: "Cargo (gastos)" },
  { key: "credit", label: "Abono (ingresos)" },
  { key: "balance", label: "Saldo" },
];
const FORMAT: Record<string, string> = { csv: "CSV", excel: "Excel", ofx: "OFX", n43: "Norma 43" };

type Props = { accounts: Account[]; defaultAccountId?: string; onClose: () => void; onDone: (r: ImportResult) => void };

export function ImportWizard({ accounts, defaultAccountId, onClose, onDone }: Props) {
  const [path, setPath] = useState<string | null>(null);
  const [preview, setPreview] = useState<Preview | null>(null);
  const [mapping, setMapping] = useState<Mapping | null>(null);
  const [accountId, setAccountId] = useState<string>(defaultAccountId ?? accounts[0]?.id ?? "__new");
  const [newName, setNewName] = useState("Cuenta principal");
  const [newBank, setNewBank] = useState("");
  const [useBalance, setUseBalance] = useState(true);
  const [showMap, setShowMap] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<ImportResult | null>(null);

  async function pick() {
    const f = await open({
      multiple: false,
      filters: [{ name: "Extractos", extensions: ["csv", "txt", "xlsx", "xls", "ods", "ofx", "qfx", "n43", "aeb", "q43"] }],
    });
    if (typeof f === "string") setPath(f);
  }

  useEffect(() => {
    if (!path) return;
    setError(null);
    setBusy(true);
    api.importPreview(path, mapping)
      .then((p) => {
        setPreview(p);
        if (!mapping && p.mapping) setMapping(p.mapping);
        if (p.bank_guess && !newBank) setNewBank(p.bank_guess.split(" /")[0]);
      })
      .catch((e) => { setError(errMsg(e)); setShowMap(true); })
      .finally(() => setBusy(false));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [path, mapping]);

  const sums = useMemo(() => {
    const s = preview?.sample ?? [];
    return { inc: s.filter((t) => t.amount_cents > 0).length, exp: s.filter((t) => t.amount_cents < 0).length };
  }, [preview]);

  async function commit() {
    if (!path) return;
    setBusy(true);
    setError(null);
    try {
      let id = accountId;
      if (id === "__new") {
        if (!newBank.trim()) throw "Indica el banco de la cuenta nueva";
        id = await api.accountCreate({ name: newName, institution: newBank });
      }
      const r = await api.importCommit(path, id, preview?.format === "csv" || preview?.format === "excel" ? mapping : null, useBalance);
      setResult(r);
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setBusy(false);
    }
  }

  if (result) {
    return (
      <Modal title="Importación completada" onClose={() => onDone(result)}
        footer={<button className="btn p" onClick={() => onDone(result)}>Ver movimientos</button>}>
        <div className="result">
          <CheckCircle2 size={40} className="inc" />
          <div className="result-grid">
            <div><b className="num">{result.inserted}</b><span>nuevos</span></div>
            <div><b className="num">{result.duplicates}</b><span>ya existían</span></div>
            <div><b className="num">{result.categorized}</b><span>categorizados</span></div>
            <div><b className="num">{result.transfers}</b><span>transferencias</span></div>
          </div>
        </div>
      </Modal>
    );
  }

  const isTable = preview?.format === "csv" || preview?.format === "excel" || (!preview && error);
  const canImport = !!preview && preview.total > 0 && !busy;

  return (
    <Modal title="Importar extracto" onClose={onClose} width={860}
      footer={path ? (
        <>
          <span className="muted grow">{preview ? `${preview.total} movimientos${preview.skipped ? ` · ${preview.skipped} filas ignoradas` : ""}` : ""}</span>
          <button className="btn g" onClick={pick}>Otro fichero</button>
          <button className="btn p" onClick={commit} disabled={!canImport}>{busy ? "Procesando…" : "Importar"}</button>
        </>
      ) : undefined}>
      {!path ? (
        <button className="dropzone" onClick={pick}>
          <FileUp size={32} />
          <b>Elige el extracto de tu banco</b>
          <span className="muted">CSV, Excel (xls/xlsx), OFX o Norma 43. Descárgalo desde la web o app de tu banco.</span>
        </button>
      ) : (
        <div className="stack">
          <div className="file-line">
            <FileUp size={16} /><span className="ellipsis">{path.split(/[\\/]/).pop()}</span>
            {preview && <span className="chip">{FORMAT[preview.format]}</span>}
            {preview?.bank_guess && <span className="chip ok">{preview.bank_guess}</span>}
            {preview?.date_from && <span className="muted">{fmtDate(preview.date_from)} – {fmtDate(preview.date_to!)}</span>}
          </div>

          {error && <div className="banner warn"><AlertTriangle size={16} />{error}</div>}

          <div className="row2">
            <Field label="Importar en la cuenta">
              <select className="input" value={accountId} onChange={(e) => setAccountId(e.target.value)}>
                {accounts.map((a) => <option key={a.id} value={a.id}>{a.institution} · {a.name}</option>)}
                <option value="__new">+ Cuenta nueva…</option>
              </select>
            </Field>
            {accountId === "__new" && (
              <div className="row2">
                <Field label="Banco"><input className="input" value={newBank} onChange={(e) => setNewBank(e.target.value)} placeholder="Openbank" /></Field>
                <Field label="Nombre"><input className="input" value={newName} onChange={(e) => setNewName(e.target.value)} /></Field>
              </div>
            )}
          </div>

          {preview?.last_balance_cents != null && (
            <label className="check">
              <input type="checkbox" checked={useBalance} onChange={(e) => setUseBalance(e.target.checked)} />
              Actualizar el saldo de la cuenta a <b className="num">{money(preview.last_balance_cents)}</b> (saldo más reciente del extracto)
            </label>
          )}

          {isTable && (
            <div>
              <button className="link" onClick={() => setShowMap((s) => !s)}>{showMap ? "Ocultar columnas" : "Ajustar columnas"}</button>
              {showMap && preview && mapping && (
                <div className="map-grid">
                  {FIELDS.map((f) => (
                    <Field key={f.key} label={f.label}>
                      <select className="input sm" value={(mapping[f.key] as number | null) ?? ""}
                        onChange={(e) => setMapping({ ...mapping, [f.key]: e.target.value === "" ? null : Number(e.target.value) })}>
                        <option value="">—</option>
                        {preview.columns.map((c, i) => <option key={i} value={i}>{c || `Columna ${i + 1}`}</option>)}
                      </select>
                    </Field>
                  ))}
                  <label className="check"><input type="checkbox" checked={mapping.invert} onChange={(e) => setMapping({ ...mapping, invert: e.target.checked })} />Invertir signo</label>
                </div>
              )}
            </div>
          )}

          {preview && preview.sample.length > 0 && (
            <div className="table-wrap">
              <table className="table">
                <thead><tr><th>Fecha</th><th>Concepto</th><th className="r">Importe</th><th className="r">Saldo</th></tr></thead>
                <tbody>
                  {preview.sample.slice(0, 12).map((t, i) => (
                    <tr key={i}>
                      <td className="nowrap">{fmtDate(t.date)}</td>
                      <td className="ellipsis" style={{ maxWidth: 420 }}>{t.description}</td>
                      <td className={`r num ${t.amount_cents > 0 ? "inc" : "exp"}`}>{money(t.amount_cents, { sign: true })}</td>
                      <td className="r num muted">{t.balance_cents != null ? money(t.balance_cents) : ""}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
              <div className="muted small pad">Vista previa: {sums.inc} ingresos y {sums.exp} gastos en las primeras filas.</div>
            </div>
          )}
        </div>
      )}
    </Modal>
  );
}
