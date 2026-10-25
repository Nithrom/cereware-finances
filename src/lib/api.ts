import { invoke } from "@tauri-apps/api/core";

export type VaultStatus = { exists: boolean; unlocked: boolean };
export type Account = {
  id: string; name: string; institution: string; kind: string; currency: string;
  balance_cents: number; provider: string; consent_expires_at: string | null;
  is_business: boolean; color: string | null;
};
export type Category = { id: string; parent_id: string | null; name: string; kind: string; color: string; icon: string };
export type DashboardSummary = {
  total_balance_cents: number; month_income_cents: number; month_expense_cents: number;
  accounts: number; transactions: number;
};

export const api = {
  vaultStatus: () => invoke<VaultStatus>("vault_status"),
  vaultCreate: (password: string) => invoke<void>("vault_create", { password }),
  vaultUnlock: (password: string) => invoke<void>("vault_unlock", { password }),
  vaultLock: () => invoke<void>("vault_lock"),
  settingsGet: (key: string) => invoke<string | null>("settings_get", { key }),
  settingsSet: (key: string, value: string) => invoke<void>("settings_set", { key, value }),
  accounts: () => invoke<Account[]>("accounts_list"),
  categories: () => invoke<Category[]>("categories_list"),
  dashboard: () => invoke<DashboardSummary>("dashboard_summary"),
};
