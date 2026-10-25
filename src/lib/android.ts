import { api2 } from "./api";
import { money } from "./format";

/** Puente nativo (solo existe en la app Android). */
type Bridge = {
  installApk(path: string): boolean;
  saveWidget(balance: string, spent: string, label: string, updated: string): void;
  clearWidget(): void;
};
export const android: Bridge | undefined = (window as unknown as { CwfAndroid?: Bridge }).CwfAndroid;

/** Actualiza el resumen del widget de escritorio con el saldo y el gasto del mes. */
export async function refreshWidget() {
  if (!android) return;
  try {
    const d = await api2.dashboard();
    const label = new Date().toLocaleDateString("es-ES", { month: "long" });
    const updated = new Date().toLocaleTimeString("es-ES", { hour: "2-digit", minute: "2-digit" });
    android.saveWidget(money(d.total_balance_cents), money(Math.abs(d.month_expense_cents)), label, updated);
  } catch { /* sin almacén abierto */ }
}
