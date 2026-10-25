import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { android } from "./android";

export type Update = { version: string; notes: string | null; install: (onProgress?: (pct: number | null) => void) => Promise<void> };

/** Busca una versión nueva en el servidor de CereWare. Windows: plugin updater firmado. Android: APK firmado. */
export async function checkUpdate(): Promise<Update | null> {
  if (android) {
    const r = await invoke<{ version: string | null; notes: string | null; url: string | null }>("update_check");
    if (!r.version || !r.url) return null;
    const url = r.url;
    return {
      version: r.version, notes: r.notes,
      install: async (p) => {
        p?.(null);
        const path = await invoke<string>("update_download", { url });
        if (!android!.installApk(path)) throw new Error("No se pudo abrir el instalador");
      },
    };
  }
  const { check } = await import("@tauri-apps/plugin-updater");
  const u = await check();
  if (!u) return null;
  return {
    version: u.version, notes: u.body ?? null,
    install: async (p) => {
      let total = 0, done = 0;
      await u.downloadAndInstall((e) => {
        if (e.event === "Started") total = e.data.contentLength ?? 0;
        if (e.event === "Progress") { done += e.data.chunkLength; p?.(total ? Math.round((done / total) * 100) : null); }
      });
      const { relaunch } = await import("@tauri-apps/plugin-process");
      await relaunch();
    },
  };
}

export const appVersion = () => getVersion();
