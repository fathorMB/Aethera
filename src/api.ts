import { invoke } from "@tauri-apps/api/core";

export interface Issue {
  field: string;
  message: string;
}

export interface Sampling {
  temperature?: number | null;
  top_p?: number | null;
  top_k?: number | null;
  min_p?: number | null;
  presence_penalty?: number | null;
  repeat_penalty?: number | null;
  source?: string | null;
  verified?: string | null;
}

export interface Profile {
  schema_version: number;
  name: string;
  gate?: string | null;
  notes?: string | null;
  model: { repo?: string | null; file: string; sha256?: string | null; size_gb?: number | null; quant?: string | null };
  runtime: { kind: string; backend: string; build: string };
  server: {
    host: string;
    port: number;
    ctx: number;
    n_parallel: number;
    n_gpu_layers: number;
    flash_attn: string;
    cache_type_k: string;
    cache_type_v: string;
    ubatch: number;
    batch: number;
    load_mode: string;
    threads?: number | null;
    threads_batch?: number | null;
    metrics: boolean;
    jinja: boolean;
    slot_save: boolean;
    extra_args?: string[];
  };
  speculative: {
    type: string;
    draft_n_max?: number | null;
    draft_n_min?: number | null;
    draft_p_min?: number | null;
    draft_model?: string | null;
  };
  cache: { cache_reuse?: number | null; ctx_checkpoints?: number | null };
  client?: { context_window: number; reserved_output_tokens: number } | null;
  sampling_by_mode?: Record<string, Sampling>;
}

export interface ProfileEntry {
  name: string;
  file: string;
  profile: Profile | null;
  issues: Issue[];
}

export interface Override {
  field: string;
  base?: string;
  value?: string;
}

export interface ArgGroup {
  flag: string;
  value: string | null;
  base: string | null;
  status: "same" | "changed" | "added" | "removed";
}

export interface Preview {
  issues: Issue[];
  overrides: Override[];
  invalidates_cache: boolean;
  blockers: string[];
  args: ArgGroup[];
  line: string;
  binary: string | null;
  model_path: string | null;
  model_size_gb: number | null;
  estimate: Estimate | null;
}

export interface BuildEntry {
  id: string;
  path: string;
}

export interface MachineConfig {
  schema_version: number;
  name: string;
  models_dir?: string | null;
  ram_margin_gib: number;
  build?: BuildEntry[];
}

export interface ResolvedBuild {
  id: string;
  dir: string;
  binary: string;
  source: string;
}

export interface SystemReport {
  hostname: string | null;
  os: string | null;
  cpu: string | null;
  ram_total_gib: number | null;
  ram_available_gib: number | null;
  gpus: { name: string; dedicated_gib: number | null }[];
}

export type ExitBehavior = "ask" | "stop" | "leave";

export interface Overview {
  data_root: string | null;
  /** Perché la radice scelta non è utilizzabile: disco staccato, cartella sparita, sola lettura. */
  data_root_error: string | null;
  machine: MachineConfig | null;
  machine_error: string | null;
  system: SystemReport;
  exit_behavior: ExitBehavior;
  builds: ResolvedBuild[];
  builds_missing: BuildEntry[];
  endpoint: string | null;
  endpoint_error: string | null;
}

export interface RunInfo {
  run_id: string;
  profile: string;
  base: string;
  base_url: string;
  started_at: string;
  pid: number;
  build: string;
  command_line: string;
  log_path: string;
  manifest_path: string;
  ctx_declared: number;
  n_parallel: number;
  overrides: Override[];
  invalidates_cache: boolean;
}

export interface Finished {
  run: RunInfo;
  code: number | null;
  by_user: boolean;
  left_running: boolean;
  ended_at: string;
}

export interface LockView {
  id: string;
  client: string;
  label: string | null;
  since: string;
  expires_in_s: number;
}

export interface Usage {
  in_use: boolean;
  reasons: string[];
  protected: boolean;
  slot_processing: boolean | null;
  last_request_s: number | null;
  locks: LockView[];
}

export interface Summary {
  requests: number;
  window: number;
  prefill_median: number | null;
  decode_median: number | null;
  decode_p10: number | null;
  decode_p90: number | null;
  acceptance: number | null;
  cache_share: number | null;
  cache_series: (number | null)[];
  decode_series: (number | null)[];
  last_prompt: number | null;
  draft_n: number;
  draft_accepted: number;
  prompt_processed: number;
  prompt_cached: number;
  generated: number;
}

export interface Counters {
  prompt_tokens: number;
  cached_tokens: number;
  predicted_tokens: number;
}

export interface MemoryBefore {
  device?: string | null;
  vram_total_mib?: number | null;
  vram_free_mib?: number | null;
  ram_available_gib?: number | null;
}

export interface MemoryAfter {
  at: string;
  after_ms: number;
  vram_dedicated_gib?: number | null;
  vram_shared_gib?: number | null;
  working_set_gib?: number | null;
  ram_available_gib?: number | null;
  double_copy?: boolean | null;
  ram_margin_gib: number;
  margin_ok?: boolean | null;
}

export interface MemorySection {
  before?: MemoryBefore | null;
  after_load?: MemoryAfter | null;
}

export interface Orphan {
  pid: number;
  port: number;
  base_url: string;
  process: string;
  alias: string | null;
  ctx: number | null;
  slot_processing: boolean | null;
  left_by_aethera: string | null;
}

export type ReadyStatus = {
  state: "ready";
  run: RunInfo;
  uptime_s: number;
  load_ms: number;
  ctx_served: number | null;
  alias_served: string | null;
  divergences: string[];
  usage: Usage;
  telemetry: Summary;
  counters: Counters | null;
  memory: MemorySection | null;
  degraded: string[];
};

export type EngineStatus =
  | { state: "off"; last: Finished | null }
  | { state: "loading"; run: RunInfo; elapsed_s: number; health: number | null; usage: Usage }
  | ReadyStatus
  | { state: "exited"; finished: Finished }
  | { state: "orphan"; orphan: Orphan; last: Finished | null };

export interface ClientSnippets {
  toml: string;
  env: string;
  powershell: string;
}

export interface TelemetryRecord {
  at: string;
  task: number;
  prompt_n: number;
  prompt_ms: number;
  prompt_tps?: number | null;
  cache_n?: number | null;
  gen_n: number;
  gen_ms: number;
  decode_tps?: number | null;
  draft_n?: number | null;
  draft_accepted?: number | null;
}

export interface RunRow {
  id: string;
  started: string;
  ended: string | null;
  running: boolean;
  machine: string;
  profile: string;
  overrides: Override[];
  invalidates_cache: boolean;
  build: string;
  commit: string | null;
  uptime_s: number | null;
  load_ms: number | null;
  ctx_declared: number;
  ctx_served: number | null;
  load_mode: string;
  speculative: string;
  summary: Summary;
  vram_dedicated_gib: number | null;
  ram_available_after_gib: number | null;
  double_copy: boolean | null;
  margin_ok: boolean | null;
  exit_code: number | null;
  by_user: boolean | null;
  left_running: boolean | null;
  degraded: string[];
}

export interface RunDetail {
  row: RunRow;
  manifest_text: string;
  records: TelemetryRecord[];
}

export interface Comparison {
  a: RunRow;
  b: RunRow;
  profile_diff: Override[];
  conditions: { label: string; a: string | null; b: string | null }[];
}

// --- Catalogo (M-04) ---

export interface ModelInfo {
  arch: string | null;
  name: string | null;
  block_count: number | null;
  context_train: number | null;
  embedding_length: number | null;
  head_count: number | null;
  head_count_kv: number | null;
  key_length: number | null;
  value_length: number | null;
  expert_count: number | null;
  expert_used_count: number | null;
  mtp_layers: number | null;
  full_attention_interval: number | null;
  ssm_conv_kernel: number | null;
  ssm_state_size: number | null;
  ssm_group_count: number | null;
  ssm_inner_size: number | null;
  dominant_type: string | null;
  file_type: number | null;
  mtp_types: string[];
  vocab_size: number | null;
  tensor_count: number;
}

export interface Estimate {
  weights_bytes: number | null;
  kv_bytes: number | null;
  state_bytes: number | null;
  compute_bytes: number | null;
  compute_from: string | null;
  total_bytes: number | null;
  /** Senza buffer di calcolo misurato il totale è un minimo, non una previsione. */
  total_is_lower_bound: boolean;
  full_attention_blocks: number | null;
  notes: string[];
}

export type ModelState = "verified" | "present" | "mismatch" | "downloading" | "downloadable" | "missing";

export interface ModelRow {
  id: string;
  repo: string | null;
  file: string;
  quant: string | null;
  size_gb: number | null;
  size_bytes: number | null;
  path: string | null;
  state: ModelState;
  sha256: string | null;
  sha256_verified: string | null;
  verified_at: string | null;
  info: ModelInfo | null;
  estimate: Estimate | null;
  estimate_ctx: number | null;
  estimate_profile: string | null;
  hard_links: number | null;
  part_bytes: number | null;
  profiles: string[];
  sampling_by_mode: Record<string, Sampling>;
  /** File presenti che pesano quanto questa voce dichiara: forse è lei, rinominata. */
  renamed_candidates: string[];
}

export interface CatalogView {
  rows: ModelRow[];
  error: string | null;
}

/** Una riga di log che spiega un guasto, con il consiglio quando è riconosciuta. */
export interface Reason {
  line: string;
  hint: string | null;
}

export interface MachineText {
  path: string;
  text: string | null;
  error: string | null;
}

export interface RemovalPlan {
  file: string;
  path: string | null;
  hard_links: number | null;
  warnings: string[];
}

export type TaskKind = "verify" | "download" | "install" | "copy";
export type TaskState = "running" | "done" | "failed" | "cancelled";

export interface TaskView {
  id: string;
  kind: TaskKind;
  target: string;
  state: TaskState;
  done: number;
  total: number | null;
  message: string | null;
  started: string;
  ended: string | null;
}

export interface Device {
  id: string;
  name: string;
  total_mib: number;
  free_mib: number;
}

export interface Asset {
  name: string;
  size: number;
  digest: string | null;
  url: string;
  backend: string;
}

export interface Release {
  tag: string;
  published: string | null;
  assets: Asset[];
}

export interface BuildsView {
  installed: ResolvedBuild[];
  used_by: Record<string, number>;
  releases: Release[];
  releases_error: string | null;
}

/** Che cosa comporta registrare un .gguf che sta fuori dalla cartella dei pesi. */
export interface AdoptPlan {
  source: string;
  file: string;
  target: string;
  bytes: number;
  action: "nothing" | "link" | "copy";
  already: string | null;
  notes: string[];
  blocker: string | null;
  free_disk: number | null;
}

export interface ImportOutcome {
  task: string | null;
  message: string;
  rows: ModelRow[];
}

export interface DeletionPlan {
  name: string;
  file: string;
  runs: number;
  warnings: string[];
}

export interface CardReport {
  notes: string[];
  rows: ModelRow[];
}

export const profileTemplate = (modelFile: string | null) => invoke<Profile>("profile_template", { modelFile });
export const profileDuplicate = (name: string, newName: string) =>
  invoke<string>("profile_duplicate", { name, newName });
export const profileRename = (name: string, newName: string) => invoke<string>("profile_rename", { name, newName });
export const profileDeletionPlan = (name: string) => invoke<DeletionPlan>("profile_deletion_plan", { name });
export const profileDelete = (name: string) => invoke<void>("profile_delete", { name });
export const catalogSampling = (file: string) => invoke<Record<string, Sampling>>("catalog_sampling", { file });
export const catalogImportPlan = (path: string) => invoke<AdoptPlan>("catalog_import_plan", { path });
export const catalogImport = (path: string, confirmCopy: boolean) =>
  invoke<ImportOutcome>("catalog_import", { path, confirmCopy });
export const catalogModelcard = (id: string, replace: boolean) =>
  invoke<CardReport>("catalog_modelcard", { id, replace });
export const buildsImportDir = (path: string) => invoke<BuildsView>("builds_import_dir", { path });

export const catalogList = () => invoke<CatalogView>("catalog_list");
export const catalogRelink = (id: string, file: string) => invoke<ModelRow[]>("catalog_relink", { id, file });
export const machineText = () => invoke<MachineText>("machine_text");
export const machineReset = () => invoke<Overview>("machine_reset");
export const engineFailure = () => invoke<Reason[]>("engine_failure");
export const catalogVerify = (id: string) => invoke<string>("catalog_verify", { id });
export const catalogAdd = (repo: string, file: string) => invoke<ModelRow[]>("catalog_add", { repo, file });
export const catalogDownload = (id: string) => invoke<string>("catalog_download", { id });
export const catalogReread = (id: string) => invoke<ModelRow[]>("catalog_reread", { id });
export const catalogRemovalPlan = (id: string) => invoke<RemovalPlan>("catalog_removal_plan", { id });
export const catalogRemove = (id: string, deleteFile: boolean, confirm: boolean) =>
  invoke<ModelRow[]>("catalog_remove", { id, deleteFile, confirm });
export const buildsList = (refresh: boolean) => invoke<BuildsView>("builds_list", { refresh });
export const buildsInstall = (tag: string, backend: string) => invoke<string>("builds_install", { tag, backend });
export const buildDevices = (id: string) => invoke<Device[]>("build_devices", { id });
export const tasksList = () => invoke<TaskView[]>("tasks_list");
export const taskCancel = (id: string) => invoke<void>("task_cancel", { id });
export const tasksClear = () => invoke<number>("tasks_clear");

export interface Step {
  id: string;
  title: string;
  what: string;
  done: boolean;
  detail: string | null;
  page: string;
}

export interface Setup {
  steps: Step[];
  complete: boolean;
}

export const setup = () => invoke<Setup>("setup");
export const overview = () => invoke<Overview>("overview");
export const setDataRoot = (path: string) => invoke<Overview>("set_data_root", { path });
export const saveMachine = (machine: MachineConfig) => invoke<Overview>("save_machine", { machine });
export const setExitBehavior = (behavior: ExitBehavior) => invoke<void>("set_exit_behavior", { behavior });
export const listProfiles = () => invoke<ProfileEntry[]>("list_profiles");
export const preview = (base: string, edited: Profile) => invoke<Preview>("preview", { base, edited });
export const saveProfile = (profile: Profile, replace: string | null) =>
  invoke<string>("save_profile", { profile, replace });
export const importMinis = (path: string, build: string) =>
  invoke<{ name: string; notes: string[] }>("import_minis", { path, build });
export const engineStart = (base: string, edited: Profile) => invoke<RunInfo>("engine_start", { base, edited });
export const engineStop = () => invoke<void>("engine_stop");
export const engineRestart = () => invoke<RunInfo>("engine_restart");
export const engineProtect = (on: boolean) => invoke<Usage>("engine_protect", { on });
export const engineStatus = () => invoke<EngineStatus>("engine_status");
export const engineLog = (lines: number) => invoke<string>("engine_log", { lines });
export const orphanTerminate = (pid: number) => invoke<void>("orphan_terminate", { pid });
export const clientSnippets = () => invoke<ClientSnippets | null>("client_snippets");
export const runsList = () => invoke<RunRow[]>("runs_list");
export const runDetail = (id: string) => invoke<RunDetail>("run_detail", { id });
export const runsCompare = (a: string, b: string) => invoke<Comparison>("runs_compare", { a, b });
export const appExit = (stop: boolean, remember: boolean, cancelTasks = false) =>
  invoke<void>("app_exit", { stop, remember, cancelTasks });

/** Il run corrente o l'ultimo, qualunque sia lo stato. */
export function runOf(s: EngineStatus | null): RunInfo | null {
  if (!s) return null;
  if (s.state === "loading" || s.state === "ready") return s.run;
  if (s.state === "exited") return s.finished.run;
  return s.last?.run ?? null;
}

export const isEngineOn = (s: EngineStatus | null) => s?.state === "loading" || s?.state === "ready";

export function usageOf(s: EngineStatus | null): Usage | null {
  return s?.state === "loading" || s?.state === "ready" ? s.usage : null;
}

export const inUse = (s: EngineStatus | null) => usageOf(s)?.in_use ?? false;
