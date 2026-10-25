import { invoke } from "@tauri-apps/api/core";
import { api } from "./api";

/**
 * Desbloqueo con huella/cara (solo móvil): la contraseña del almacén se guarda en el
 * Keystore de Android, cifrada con una clave que exige autenticación biométrica.
 */
const FLAG = "cwf-bio";
const KEY = "cwf-vault-password";

export const bioEnabled = () => { try { return localStorage.getItem(FLAG) === "1"; } catch { return false; } };
const setFlag = (v: boolean) => { try { v ? localStorage.setItem(FLAG, "1") : localStorage.removeItem(FLAG); } catch { /* */ } };

export async function bioEnable(password: string) {
  await invoke("plugin:keystore|store", {
    payload: { key: KEY, value: password, prompt: { title: "Activar desbloqueo con huella", subtitle: "Confirma con tu huella o tu cara", cancelLabel: "Cancelar" } },
  });
  setFlag(true);
}

export async function bioUnlock(): Promise<void> {
  const r = await invoke<{ value: string | null }>("plugin:keystore|retrieve", {
    payload: { key: KEY, prompt: { title: "Desbloquear CereWare Finances", subtitle: "Usa tu huella o tu cara", cancelLabel: "Usar contraseña" } },
  });
  if (!r?.value) { setFlag(false); throw new Error("No hay desbloqueo biométrico guardado"); }
  await api.vaultUnlock(r.value);
}

export async function bioDisable() {
  try { await invoke("plugin:keystore|remove", { payload: { key: KEY } }); } catch { /* */ }
  setFlag(false);
}
