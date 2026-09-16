/**
 * Le leve di un profilo come le mostra la pagina Avvio della v2: dodici essenziali con l'etichetta
 * in italiano, le altre in «Avanzate». Ogni campo del profilo sta in una leva sola: nessuno sparisce.
 */
import type { Profile } from "../api";
import { getPath, sameValue, show } from "../format";

export type Kind = "text" | "opttext" | "int" | "optint" | "optfloat" | "bool" | "optbool" | "select" | "optselect" | "list";

export interface Input {
  path: string;
  kind: Kind;
  options?: string[];
  /** Larghezza del controllo quando ce ne sono più d'uno affiancati. */
  width?: string;
  placeholder?: string;
}

export interface Lever {
  id: string;
  label: string;
  /** Il nome della leva nel profilo, in piccolo sotto l'etichetta. */
  lever: string;
  inputs: Input[];
  cache?: boolean;
  hint?: string;
  /** Che cosa ne ha concluso M-08 su questa macchina: informa, non impedisce. */
  verdict?: string;
  /** La fonte del verdetto, nel title della pillola. */
  verdictTitle?: string;
  /** Controlli che non sono un semplice campo: build dalla macchina, layer sulla GPU. */
  special?: "build" | "gpu";
}

export const M08 = "rapporto in .lmbrain-lite/reports/misure-motore-2026-09.md";

// Le stesse liste del backend (profile.rs).
export const FLASH = ["on", "off", "auto"];
export const CACHE_TYPES = ["f32", "f16", "bf16", "q8_0", "q4_0", "q4_1", "iq4_nl", "q5_0", "q5_1"];
export const LOAD = ["auto", "none", "mmap", "mlock", "mmap+mlock", "dio"];
export const SPEC_TYPES = [
  "none",
  "draft-simple",
  "draft-eagle3",
  "draft-mtp",
  "draft-dflash",
  "draft-dspark",
  "ngram-simple",
  "ngram-map-k",
  "ngram-map-k4v",
  "ngram-mod",
  "ngram-cache",
];

/** Le dodici leve essenziali del mockup approvato. */
export const ESSENTIAL: Lever[] = [
  { id: "model", label: "Modello", lever: "model.file", inputs: [{ path: "model.file", kind: "text" }], hint: "nella cartella pesi" },
  {
    id: "build",
    label: "Build del motore",
    lever: "runtime.build · backend",
    inputs: [
      { path: "runtime.build", kind: "text", placeholder: "build" },
      { path: "runtime.backend", kind: "text", placeholder: "backend" },
    ],
    special: "build",
    hint: "fissata nel profilo",
  },
  { id: "ctx", label: "Contesto", lever: "server.ctx", inputs: [{ path: "server.ctx", kind: "int" }], cache: true, hint: "verificato su /props" },
  {
    id: "par",
    label: "Slot in parallelo",
    lever: "server.n_parallel",
    inputs: [{ path: "server.n_parallel", kind: "int" }],
    cache: true,
    hint: "MTP richiede 1",
  },
  {
    id: "gpu",
    label: "Layer sulla GPU",
    lever: "server.n_gpu_layers",
    inputs: [{ path: "server.n_gpu_layers", kind: "optint", width: "80px" }],
    special: "gpu",
    hint: "vuoto: decide fit",
  },
  { id: "flash", label: "Attenzione flash", lever: "server.flash_attn", inputs: [{ path: "server.flash_attn", kind: "select", options: FLASH }] },
  {
    id: "kv",
    label: "Tipo della cache K / V",
    lever: "cache_type_k · cache_type_v",
    inputs: [
      { path: "server.cache_type_k", kind: "select", options: CACHE_TYPES },
      { path: "server.cache_type_v", kind: "select", options: CACHE_TYPES },
    ],
    verdict: "q8_0: scartata",
    verdictTitle: "M-08 T-06: q8_0 peggiora il decode a tutti i contesti provati (−7,6 % a 32k, −5,5 % a 64k, −3,4 % a 128k) e risparmia poco",
  },
  {
    id: "batch",
    label: "Micro-batch / batch",
    lever: "server.ubatch · batch",
    inputs: [
      { path: "server.ubatch", kind: "int" },
      { path: "server.batch", kind: "int" },
    ],
  },
  {
    id: "load",
    label: "Caricamento dei pesi",
    lever: "server.load_mode",
    inputs: [{ path: "server.load_mode", kind: "select", options: LOAD }],
    verdict: "mmap: scartata",
    verdictTitle: "M-08 T-09: con mmap il processo tiene una doppia copia dei pesi su un modello che riempie la VGM; auto e none vanno bene",
  },
  {
    id: "spec",
    label: "Speculazione",
    lever: "speculative.type · n-min · n-max · p-min",
    inputs: [
      { path: "speculative.type", kind: "select", options: SPEC_TYPES },
      { path: "speculative.draft_n_min", kind: "optint", width: "52px", placeholder: "n-min" },
      { path: "speculative.draft_n_max", kind: "optint", width: "52px", placeholder: "n-max" },
      { path: "speculative.draft_p_min", kind: "optfloat", width: "60px", placeholder: "p-min" },
    ],
    hint: "vuoti: default motore",
  },
  {
    id: "template",
    label: "Template di chat",
    lever: "server.chat_template_file",
    inputs: [{ path: "server.chat_template_file", kind: "opttext", placeholder: "del modello" }],
    hint: "vuoto: del modello · qwen3.6-tollerante.jinja per Claude Code",
  },
  {
    id: "port",
    label: "Indirizzo e porta",
    lever: "server.host · port",
    inputs: [
      { path: "server.host", kind: "text" },
      { path: "server.port", kind: "int", width: "72px" },
    ],
    hint: "porta sempre esplicita",
  },
];

/** Tutto il resto, dietro un clic. */
export const ADVANCED: Lever[] = [
  {
    id: "fit",
    label: "Adattamento alla memoria",
    lever: "server.fit · fit_target",
    inputs: [
      { path: "server.fit", kind: "optselect", options: ["on", "off"], width: "110px" },
      { path: "server.fit_target", kind: "opttext", placeholder: "MiB liberi" },
    ],
    hint: "default motore: on · fit_target in MiB liberi, 1024 o 1024,512",
  },
  {
    id: "threads",
    label: "Thread",
    lever: "threads · threads_batch",
    inputs: [
      { path: "server.threads", kind: "optint", placeholder: "default motore" },
      { path: "server.threads_batch", kind: "optint", placeholder: "default motore" },
    ],
    hint: "default motore",
  },
  {
    id: "moe",
    label: "Esperti sulla CPU",
    lever: "server.n_cpu_moe",
    inputs: [{ path: "server.n_cpu_moe", kind: "optint" }],
    verdict: "M-08: peggiora",
    verdictTitle: "M-08 T-08: sul Coder-Next ogni layer di esperti sulla CPU toglie velocità, in modo monotono",
  },
  {
    id: "ot",
    label: "Tensori su buffer",
    lever: "server.tensor_overrides · -ot",
    inputs: [{ path: "server.tensor_overrides", kind: "list", placeholder: "regex=buffer" }],
    hint: "regex=buffer, separati da spazi",
  },
  {
    id: "lazy",
    label: "Lettura pigra",
    lever: "server.lazy_mode",
    inputs: [{ path: "server.lazy_mode", kind: "optselect", options: ["auto", "on", "off"] }],
    hint: "default motore: auto",
  },
  {
    id: "flags",
    label: "Metriche · jinja · slot su disco",
    lever: "metrics · jinja · slot_save",
    inputs: [
      { path: "server.metrics", kind: "bool" },
      { path: "server.jinja", kind: "bool" },
      { path: "server.slot_save", kind: "bool" },
    ],
    hint: "slot su runs/<id>/slots",
  },
  {
    id: "extra",
    label: "Argomenti extra",
    lever: "server.extra_args",
    inputs: [{ path: "server.extra_args", kind: "list" }],
    hint: "via di fuga",
  },
  {
    id: "draft",
    label: "Modello draft esterno",
    lever: "speculative.draft_model",
    inputs: [{ path: "speculative.draft_model", kind: "opttext" }],
  },
  {
    id: "reuse",
    label: "Riuso della cache",
    lever: "cache.cache_reuse",
    inputs: [{ path: "cache.cache_reuse", kind: "optint" }],
    cache: true,
    verdict: "M-08: scartata",
    verdictTitle: "M-08 T-13: con --cache-reuse 256 i turni costano come senza",
  },
  {
    id: "ckpt",
    label: "Checkpoint dello stato",
    lever: "ctx_checkpoints · checkpoint_min_step · cache_ram · kv_unified",
    inputs: [
      { path: "cache.ctx_checkpoints", kind: "optint", width: "56px" },
      { path: "cache.checkpoint_min_step", kind: "optint", width: "56px" },
      { path: "cache.cache_ram", kind: "optint", width: "64px" },
      { path: "cache.kv_unified", kind: "optbool" },
    ],
    cache: true,
    hint: "cache_ram in MiB · -1 senza limite · 0 spenta",
    verdict: "M-08: scartata",
    verdictTitle: "M-08 T-04: stessi token riusati, turno per turno; nessun effetto sul riuso",
  },
  {
    id: "meta",
    label: "Metadati del modello",
    lever: "model.repo · quant · size_gb · sha256",
    inputs: [
      { path: "model.repo", kind: "opttext", placeholder: "repo" },
      { path: "model.quant", kind: "opttext", width: "80px", placeholder: "quant" },
      { path: "model.size_gb", kind: "optfloat", width: "64px", placeholder: "GB" },
      { path: "model.sha256", kind: "opttext", placeholder: "sha256" },
    ],
    hint: "size_gb in GB decimali",
  },
  { id: "kind", label: "Tipo di runtime", lever: "runtime.kind", inputs: [{ path: "runtime.kind", kind: "text" }] },
  { id: "notes", label: "Note del profilo", lever: "notes", inputs: [{ path: "notes", kind: "opttext" }] },
];

export const leverPaths = (l: Lever) => l.inputs.map((i) => i.path);

export function parse(kind: Kind, raw: string): { ok: true; value: unknown } | { ok: false } {
  const t = raw.trim();
  switch (kind) {
    case "text":
    case "select":
      return { ok: true, value: raw };
    case "bool":
      return { ok: true, value: t === "on" };
    case "optselect":
      return { ok: true, value: t === "" ? null : t };
    case "optbool":
      return { ok: true, value: t === "" ? null : t === "on" };
    case "opttext":
      return { ok: true, value: t === "" ? null : t };
    case "list":
      return { ok: true, value: t === "" ? [] : t.split(/\s+/) };
    case "int":
    case "optint": {
      if (t === "") return kind === "optint" ? { ok: true, value: null } : { ok: false };
      return /^-?\d+$/.test(t) ? { ok: true, value: Number(t) } : { ok: false };
    }
    case "optfloat": {
      if (t === "") return { ok: true, value: null };
      const n = Number(t.replace(",", "."));
      return Number.isFinite(n) ? { ok: true, value: n } : { ok: false };
    }
    default:
      return { ok: false };
  }
}

/** Il valore come testo del controllo: assente è vuoto, i booleani sono on/off. */
export function toText(v: unknown): string {
  if (v == null) return "";
  if (typeof v === "boolean") return v ? "on" : "off";
  return Array.isArray(v) ? v.join(" ") : String(v);
}

/** Le vie della leva che differiscono dal profilo base. Senza base (profilo nuovo) nessuna. */
export function modifiedPaths(l: Lever, base: Profile | null | undefined, edited: Profile): string[] {
  if (!base) return [];
  return leverPaths(l).filter((p) => !sameValue(getPath(base, p), getPath(edited, p)));
}

/** Il valore di prima delle vie modificate, per il barrato. */
export function baseText(l: Lever, base: Profile | null | undefined, edited: Profile): string {
  if (!base) return "";
  return modifiedPaths(l, base, edited)
    .map((p) => show(getPath(base, p)))
    .join(" · ");
}

/** Quante leve di un gruppo sono modificate, e quante portano un verdetto. */
export function groupSummary(levers: Lever[], base: Profile | null | undefined, edited: Profile) {
  return {
    levers: levers.length,
    fields: levers.reduce((a, l) => a + l.inputs.length, 0),
    modified: levers.filter((l) => modifiedPaths(l, base, edited).length > 0).length,
    verdicts: levers.filter((l) => l.verdict).length,
  };
}

/** Modalità dei layer sulla GPU: 999 = tutti, assente = decide fit, altrimenti un numero. */
export function gpuMode(v: unknown): "all" | "num" | "fit" {
  return v == null ? "fit" : v === 999 ? "all" : "num";
}
