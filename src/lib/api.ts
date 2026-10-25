import { invoke as rawInvoke } from "@tauri-apps/api/core";

/** Tras cualquier escritura, sincroniza a los pocos segundos (agrupando cambios seguidos). */
const WRITE = /^(?!sync_|vault_|settings_)\w+_(create|update|delete|set|upsert|contribute|commit|restore|import|finish|active|split)$|^categorize_group$|^bank_sync/;
let syncTimer: number | undefined;
function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const p = rawInvoke<T>(cmd, args);
  if (WRITE.test(cmd)) p.then(() => {
    window.clearTimeout(syncTimer);
    syncTimer = window.setTimeout(() => import("./sync").then((m) => m.syncNow()), 4000);
  }, () => {});
  return p;
}

export type VaultStatus = { exists: boolean; unlocked: boolean };
export type Account = {
  id: string; name: string; institution: string; kind: string; currency: string;
  balance_cents: number; provider: string; consent_expires_at: string | null;
  is_business: boolean; color: string | null;
};
export type AccountInput = {
  name: string; institution: string; kind?: string; balance_cents?: number; is_business?: boolean; color?: string | null;
};
export type Category = { id: string; parent_id: string | null; name: string; kind: string; color: string; icon: string };
export type DashboardSummary = {
  total_balance_cents: number; month_income_cents: number; month_expense_cents: number;
  accounts: number; transactions: number;
};
export type Mapping = {
  header_row: number | null; date: number | null; value_date: number | null; description: number | null;
  detail: number | null; amount: number | null; debit: number | null; credit: number | null; balance: number | null;
  fee: number | null; state: number | null; currency: number | null; invert: boolean;
};
export type ParsedTx = {
  date: string; value_date: string | null; description: string; amount_cents: number;
  balance_cents: number | null; currency: string; external_id: string | null;
};
export type Preview = {
  format: "csv" | "excel" | "ofx" | "n43"; bank_guess: string | null; columns: string[]; mapping: Mapping | null;
  sample: ParsedTx[]; total: number; skipped: number; date_from: string | null; date_to: string | null;
  last_balance_cents: number | null; account_hint: string | null;
};
export type ImportResult = { inserted: number; duplicates: number; categorized: number; transfers: number };
export type TxFilter = {
  search?: string; account_id?: string; category_id?: string;
  kind?: "income" | "expense" | "uncategorized" | "business" | "transfer"; from?: string; to?: string;
  limit?: number; offset?: number;
};
export type TxRow = {
  id: string; account_id: string; account_name: string; institution: string; date: string; amount_cents: number;
  description: string; notes: string | null; category_id: string | null; category_name: string | null;
  category_color: string | null; category_icon: string | null; category_source: string | null;
  is_business: boolean; vat_rate: number | null; is_transfer: boolean;
};
export type TxPage = { items: TxRow[]; total: number; income_cents: number; expense_cents: number };
export type TxPatch = { category_id?: string | null; notes?: string; is_business?: boolean; vat_rate?: number; learn?: boolean };
export type TxInput = { account_id: string; date: string; amount_cents: number; description: string; category_id?: string | null; notes?: string };
export type RuleRow = { id: string; match_value: string; match_op: string; category_id: string | null; category_name: string | null; learned: boolean; enabled: boolean };

export const api = {
  vaultStatus: () => invoke<VaultStatus>("vault_status"),
  vaultCreate: (password: string) => invoke<void>("vault_create", { password }),
  vaultUnlock: (password: string) => invoke<void>("vault_unlock", { password }),
  vaultLock: () => invoke<void>("vault_lock"),
  settingsGet: (key: string) => invoke<string | null>("settings_get", { key }),
  settingsSet: (key: string, value: string) => invoke<void>("settings_set", { key, value }),
  accounts: () => invoke<Account[]>("accounts_list"),
  accountCreate: (input: AccountInput) => invoke<string>("account_create", { input }),
  accountUpdate: (id: string, input: AccountInput) => invoke<void>("account_update", { id, input }),
  accountDelete: (id: string) => invoke<void>("account_delete", { id }),
  categories: () => invoke<Category[]>("categories_list"),
  dashboard: () => invoke<DashboardSummary>("dashboard_summary"),
  importPreview: (path: string, mapping?: Mapping | null) => invoke<Preview>("import_preview", { path, mapping: mapping ?? null }),
  importCommit: (path: string, accountId: string, mapping: Mapping | null, useBalance: boolean) =>
    invoke<ImportResult>("import_commit", { path, accountId, mapping, useBalance }),
  txList: (filter: TxFilter) => invoke<TxPage>("tx_list", { filter }),
  txUpdate: (id: string, patch: TxPatch) => invoke<{ learned_applied: number }>("tx_update", { id, patch }),
  txCreate: (input: TxInput) => invoke<string>("tx_create", { input }),
  txDelete: (id: string, adjustBalance: boolean) => invoke<void>("tx_delete", { id, adjustBalance }),
  rules: () => invoke<RuleRow[]>("rules_list"),
  ruleCreate: (matchValue: string, categoryId: string) => invoke<number>("rule_create", { matchValue, categoryId }),
  ruleDelete: (id: string) => invoke<void>("rule_delete", { id }),
};

/** Mensaje legible de un error devuelto por el backend. */
export const errMsg = (e: unknown) => (typeof e === "string" ? e : e instanceof Error ? e.message : "Error inesperado");

// ---------------------------------------------------------------- análisis y planificación
export type CatSlice = { id: string; name: string; color: string; icon: string; cents: number };
export type MonthFlow = { month: string; income_cents: number; expense_cents: number };
export type Dashboard = {
  total_balance_cents: number; accounts: number; transactions: number; uncategorized: number; month: string;
  month_income_cents: number; month_expense_cents: number; avg_expense_cents: number; avg_income_cents: number;
  balance_history: { month: string; value_cents: number }[]; flows: MonthFlow[]; by_category: CatSlice[];
};
export type BudgetRow = { category_id: string; name: string; color: string; icon: string; budget_cents: number | null; spent_cents: number; avg_cents: number };
export type Recurring = {
  id: string; name: string; account_id: string | null; account_name: string | null; category_id: string | null;
  category_color: string | null; category_icon: string | null; amount_cents: number; prev_amount_cents: number | null;
  interval: "weekly" | "monthly" | "quarterly" | "yearly"; next_date: string; source: "manual" | "detected";
  is_subscription: boolean; active: boolean; yearly_cents: number;
};
export type RecurringInput = {
  name: string; account_id: string | null; category_id: string | null; amount_cents: number;
  interval: Recurring["interval"]; next_date: string; is_subscription: boolean;
};
export type Goal = { id: string; name: string; target_cents: number; saved_cents: number; target_date: string | null; color: string; monthly_needed_cents: number | null };
export type GoalInput = { name: string; target_cents: number; saved_cents?: number; target_date?: string | null; color?: string };
export type Forecast = {
  start_cents: number; daily_variable_cents: number; points: [string, number][];
  events: { date: string; name: string; amount_cents: number; recurring_id: string }[];
  min_cents: number; min_date: string; end_cents: number;
};
export type BusinessYear = {
  year: number; months: MonthFlow[]; income_cents: number; expense_cents: number;
  by_category: CatSlice[]; clients: { name: string; cents: number; count: number }[];
};
export type CategoryInput = { name: string; parent_id: string | null; kind: string; color: string; icon: string };

export const api2 = {
  dashboard: () => invoke<Dashboard>("dashboard_full"),
  budgets: (month: string) => invoke<BudgetRow[]>("budgets_list", { month }),
  budgetSet: (categoryId: string, month: string | null, amountCents: number | null) => invoke<void>("budget_set", { categoryId, month, amountCents }),
  reportCategories: (from: string, to: string, kind: "income" | "expense") => invoke<CatSlice[]>("report_categories", { from, to, kind }),
  reportFlows: (months: number) => invoke<MonthFlow[]>("report_flows", { months }),
  recurring: () => invoke<Recurring[]>("recurring_list"),
  recurringUpsert: (id: string | null, input: RecurringInput) => invoke<string>("recurring_upsert", { id, input }),
  recurringSetActive: (id: string, active: boolean) => invoke<void>("recurring_set_active", { id, active }),
  recurringDelete: (id: string) => invoke<void>("recurring_delete", { id }),
  goals: () => invoke<Goal[]>("goals_list"),
  goalUpsert: (id: string | null, input: GoalInput) => invoke<void>("goal_upsert", { id, input }),
  goalContribute: (id: string, amountCents: number) => invoke<void>("goal_contribute", { id, amountCents }),
  goalDelete: (id: string) => invoke<void>("goal_delete", { id }),
  forecast: (days: number, includeVariable: boolean) => invoke<Forecast>("forecast", { days, includeVariable }),
  businessYear: (year: number) => invoke<BusinessYear>("business_year", { year }),
  changePassword: (current: string, newPassword: string) => invoke<void>("vault_change_password", { current, newPassword }),
  backupExport: (dest: string) => invoke<void>("backup_export", { dest }),
  backupRestore: (src: string, password: string) => invoke<void>("backup_restore", { src, password }),
  exportCsv: (dest: string, filter: TxFilter) => invoke<number>("export_csv", { dest, filter }),
  categoryUpsert: (id: string | null, input: CategoryInput) => invoke<string>("category_upsert", { id, input }),
  categoryDelete: (id: string) => invoke<void>("category_delete", { id }),
};

// ---------------------------------------------------------------- conexión bancaria
export type BankConfigView = { app_id: string; has_key: boolean; redirect_url: string };
export type Aspsp = { name: string; country: string; logo: string | null; max_consent_days: number | null };
export type SyncResult = { account: string; inserted: number; error: string | null };
export const bankApi = {
  config: () => invoke<BankConfigView | null>("bank_config_get"),
  configSet: (appId: string, keyPath: string | null, redirectUrl: string) => invoke<void>("bank_config_set", { appId, keyPath, redirectUrl }),
  aspsps: (country: string) => invoke<Aspsp[]>("bank_aspsps", { country }),
  start: (name: string, country: string, days: number) => invoke<{ url: string; state: string; auto_capture: boolean; redirect_url: string }>("bank_connect_start", { name, country, days }),
  poll: () => invoke<string | null>("bank_connect_poll"),
  finish: (codeOrUrl: string, institution: string) => invoke<{ accounts: string[]; imported: number }>("bank_connect_finish", { codeOrUrl, institution }),
  sync: () => invoke<SyncResult[]>("bank_sync"),
  linked: () => invoke<{ id: string; label: string }[]>("bank_linked"),
  syncAccount: (id: string) => invoke<SyncResult>("bank_sync_account", { id }),
};

// ---------------------------------------------------------------- categorización rápida
export type UncatGroup = { key: string; sample: string; count: number; total_cents: number; income: boolean };
export const catApi = {
  groups: () => invoke<UncatGroup[]>("uncategorized_groups"),
  categorizeGroup: (key: string, income: boolean, categoryId: string) => invoke<number>("categorize_group", { key, income, categoryId }),
  rulesImport: (path: string) => invoke<[number, number, string[]]>("rules_import", { path }),
};

// ---------------------------------------------------------------- sincronización
export type SyncStatus = { enabled: boolean; server: string | null; device: string; pending: number; last_sync: string | null };
export const syncApi = {
  status: () => invoke<SyncStatus>("sync_status"),
  code: () => invoke<string>("sync_code"),
  now: () => invoke<{ pushed: number; pulled: number }>("sync_now"),
  enable: (server: string) => invoke<string>("sync_enable", { server }),
  join: (code: string) => invoke<{ pushed: number; pulled: number }>("sync_join", { code }),
  disable: () => invoke<void>("sync_disable"),
};
