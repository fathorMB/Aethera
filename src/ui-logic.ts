/**
 * Logica dei componenti della v2 (Alerts, Kpi, Stack, Chips), separata dal disegno perché si
 * possa provare senza DOM. Nessuna regola nuova: soglie e testi sono quelli di M-09 e del backend.
 */
import type { BuildProvenance, EngineStatus, MemoryAfter, Provenance, Reference } from "./api";
import { clock, fixed, num } from "./format";

export type Tone = "" | "ok" | "warn" | "err" | "busy" | "acc";

// --- Stack ---------------------------------------------------------------------------------

export interface StackPart {
  key: string;
  label: string;
  /** Assente = sconosciuto: il pezzo non si disegna, e la legenda lo dice. */
  value: number | null;
  cls: string;
}

export interface StackSegment {
  key: string;
  cls: string;
  pct: number;
}

/**
 * Da pezzi e totale alle larghezze della barra. Un pezzo sconosciuto non diventa zero: non si
 * disegna. Senza totale non si disegna niente, perché una proporzione inventata è peggio di
 * nessuna. La somma non supera mai il 100 %.
 */
export function stackSegments(parts: StackPart[], total: number | null | undefined): StackSegment[] {
  if (total == null || !Number.isFinite(total) || total <= 0) return [];
  const out: StackSegment[] = [];
  let used = 0;
  for (const p of parts) {
    if (p.value == null || !Number.isFinite(p.value) || p.value <= 0) continue;
    const pct = Math.min((p.value / total) * 100, 100 - used);
    if (pct <= 0) break;
    used += pct;
    out.push({ key: p.key, cls: p.cls, pct });
  }
  return out;
}

/** «dedicata 23,13 · condivisa sconosciuta …»: il title della barra, con tutti i pezzi. */
export function stackTitle(parts: StackPart[], digits = 2, unit = "GiB"): string {
  return parts.map((p) => `${p.label} ${p.value == null ? "sconosciuto" : `${fixed(p.value, digits)} ${unit}`}`).join(" · ");
}

/**
 * La memoria dopo il caricamento su tutta la memoria della macchina: VGM (memoria dedicata della
 * GPU) più la RAM che Windows vede. La VRAM condivisa sta dentro la RAM di Windows, quindi si
 * toglie da «Windows e altri processi» per non contarla due volte.
 */
export function memoryParts(
  after: MemoryAfter | null | undefined,
  vgmGib: number | null | undefined,
  ramTotalGib: number | null | undefined,
): { parts: StackPart[]; total: number | null } {
  const ded = after?.vram_dedicated_gib ?? null;
  const sha = after?.vram_shared_gib ?? null;
  const avail = after?.ram_available_gib ?? null;
  const vgm = vgmGib ?? null;
  const ram = ramTotalGib ?? null;
  const vfree = vgm != null && ded != null ? Math.max(0, vgm - ded) : null;
  const win = ram != null && avail != null ? Math.max(0, ram - avail - (sha ?? 0)) : null;
  return {
    total: vgm != null && ram != null ? vgm + ram : null,
    parts: [
      { key: "ded", label: "VRAM dedicata", value: ded, cls: "ded" },
      { key: "vfr", label: "VRAM non usata", value: vfree, cls: "vfr" },
      { key: "sha", label: "VRAM condivisa", value: sha, cls: "sha" },
      { key: "win", label: "RAM di Windows e altri processi", value: win, cls: "win" },
      { key: "fre", label: "RAM disponibile", value: avail, cls: "fre" },
    ],
  };
}

// --- Kpi -----------------------------------------------------------------------------------

/** Soglie della quota riusata: le stesse delle barre della tabella e della scintilla (M-09). */
export const REUSE_OK = 0.9;
export const REUSE_LOW = 0.5;
/** Il 70 % del backend (telemetry::DEGRADED_DECODE_RATIO). */
export const DEGRADED_DECODE_RATIO = 0.7;

export function shareTone(share: number | null | undefined): Tone {
  if (share == null) return "";
  return share >= REUSE_OK ? "ok" : share >= REUSE_LOW ? "warn" : "err";
}

export interface Judgement {
  tone: Tone;
  text: string;
}

export function reuseJudgement(share: number | null | undefined): Judgement {
  const tone = shareTone(share);
  switch (tone) {
    case "ok":
      return { tone, text: "i client rimandano indietro la conversazione: bene" };
    case "warn":
      return { tone, text: "riuso parziale: una parte del prompt si rielabora" };
    case "err":
      return { tone, text: "sotto il 50 %: il client cambia il prompt prima della coda" };
    default:
      return { tone, text: "nessuna richiesta attribuibile" };
  }
}

/**
 * Il giudizio sul decode: solo contro la mediana di riferimento degli avvii con le stesse
 * condizioni. Senza riferimento non c'è niente con cui confrontare, e lo si dice.
 */
export function decodeJudgement(median: number | null | undefined, reference: Reference | null | undefined): Judgement {
  if (median == null) return { tone: "", text: "nessuna richiesta misurata" };
  if (!reference) return { tone: "", text: "nessun riferimento con queste condizioni" };
  const ref = `${fixed(reference.decode_median, 1)} tok/s su ${reference.runs} ${reference.runs === 1 ? "avvio" : "avvii"}`;
  if (median < reference.decode_median * DEGRADED_DECODE_RATIO) {
    return { tone: "err", text: `sotto il 70 % del riferimento (${ref})` };
  }
  return { tone: "", text: `nella norma: riferimento ${ref}` };
}

export const seconds = (ms: number | null | undefined) => (ms == null ? null : `${fixed(ms / 1000, 1)} s`);

// --- Alerts --------------------------------------------------------------------------------

export type AlertAction = "restart" | "benchmark" | "budget";

export interface AlertSpec {
  key: string;
  tone: Tone;
  title: string;
  detail: string;
  action?: AlertAction;
}

/**
 * Gli avvisi della pagina Motore, in una fascia sola: degradato, divergenze, condizioni cambiate,
 * ultima compattazione. I testi sono quelli delle note della v1, spostati.
 */
export function engineAlerts(status: EngineStatus | null | undefined): AlertSpec[] {
  if (status?.state !== "ready") return [];
  const out: AlertSpec[] = [];
  status.degraded.forEach((msg, i) =>
    out.push({
      key: `deg${i}`,
      tone: "warn",
      title: "Degradato",
      detail: `${msg}. «Riavvia con la stessa riga» quando il motore è libero.`,
      action: "restart",
    }),
  );
  status.divergences.forEach((msg, i) =>
    out.push({ key: `div${i}`, tone: "warn", title: "Divergenza", detail: `${msg} — si registra, non si corregge` }),
  );
  if (status.conditions_changed.length) {
    const ref = status.reference;
    out.push({
      key: "cond",
      tone: "warn",
      title: "Condizioni cambiate dall'avvio precedente",
      detail:
        `${status.conditions_changed.join(" · ")}. La soglia «degradato» non confronta questo avvio con quelli di prima: ` +
        (ref
          ? `la mediana di riferimento riparte (${fixed(ref.decode_median, 1)} tok/s, ${ref.runs} ${ref.runs === 1 ? "avvio" : "avvii"} con queste condizioni finora).`
          : "la mediana di riferimento riparte da qui (nessun avvio con queste condizioni finora)."),
      action: "benchmark",
    });
  }
  const t = status.telemetry;
  const last = t.compactions[t.compactions.length - 1];
  if (last) {
    const many =
      t.compactions.length > 1
        ? ` Nelle ultime ${t.window} richieste: ${t.compactions.length} compattazioni, ${seconds(t.compactions.reduce((a, x) => a + x.cost_ms, 0))} in tutto.`
        : "";
    out.push({
      key: "cmp",
      tone: "busy",
      title: `Compattazione alle ${clock(last.at)}`,
      detail:
        `${last.client ? `di ${last.client}: ` : ""}${num(last.reprocessed)} token rielaborati fra riassunto e contesto ricostruito, ` +
        `${seconds(last.cost_ms)} che la conversazione non avrebbe pagato continuando.${many} ` +
        "Più contesto servito le rende più rare: vedi Budget di contesto nelle Impostazioni.",
      action: "budget",
    });
  }
  return out;
}

// --- Chips ---------------------------------------------------------------------------------

export interface ChipItem {
  id: string;
  label: string;
  count?: number;
}

/** Conteggio per chiave, nell'ordine in cui le chiavi compaiono. */
export function countBy<T>(items: T[], key: (x: T) => string): { id: string; count: number }[] {
  const m = new Map<string, number>();
  for (const x of items) m.set(key(x), (m.get(key(x)) ?? 0) + 1);
  return [...m].map(([id, count]) => ({ id, count }));
}

// --- Benchmark -----------------------------------------------------------------------------

/** Δ percentuale di A su B; sotto il 3 % non si colora, perché è rumore di misura. */
export function delta(a: number | null | undefined, b: number | null | undefined, digits = 0): { text: string | null; dir: "" | "up" | "dn" } {
  if (a == null || b == null || b === 0) return { text: null, dir: "" };
  const d = ((a - b) / b) * 100;
  return { text: `${d >= 0 ? "+" : ""}${fixed(d, digits)} %`, dir: Math.abs(d) < 3 ? "" : d > 0 ? "up" : "dn" };
}

/** Per le grandezze in cui di più è peggio (VRAM, tempo di caricamento) il colore si inverte. */
export function flip(dir: "" | "up" | "dn"): "" | "up" | "dn" {
  return dir === "up" ? "dn" : dir === "dn" ? "up" : "";
}

// --- Build ---------------------------------------------------------------------------------

/** Backend di llama.cpp riconosciuti dentro l'id di una build (machine::BACKENDS). */
export const BACKENDS = ["vulkan", "cuda", "hip", "sycl", "musa", "cann", "opencl", "metal", "blas", "cpu"];

/**
 * Da `b10809-win-vulkan-x64` a («b10809», «vulkan»), come machine::split_build_id: la build porta
 * la serie del fork se c'è (`b10991+moro1`), perché una build patchata si chiede per nome.
 */
export function splitBuildId(id: string): { build: string | null; backend: string | null } {
  const parts = id.split("-");
  const build = parts.find((p) => /^b\d+(\+moro\d+)?$/.test(p)) ?? null;
  const backend = parts.map((p) => p.toLowerCase()).find((p) => BACKENDS.includes(p)) ?? null;
  return { build, backend };
}

// --- Serie di patch (M-14) -----------------------------------------------------------------

/** La serie di una build scaricata da ggml-org, senza file di provenienza (provenance::UPSTREAM_SERIES). */
export const UPSTREAM_SERIES = "ggml-org";

/**
 * unknown = non registrata (mai trattata come upstream); upstream = scaricata da ggml-org;
 * plain = tag liscio compilato qui (`moro0`); patched = serie con patch; unreadable = il file di
 * provenienza c'è ma non si legge.
 */
export type SeriesKind = "unknown" | "upstream" | "plain" | "patched" | "unreadable";

export interface SeriesBranch {
  ramo: string;
  /** I primi caratteri del commit, come li registra il backend; null se la serie non lo porta. */
  commit: string | null;
}

export interface SeriesInfo {
  kind: SeriesKind;
  /** `moro1`, `ggml-org`, `moro?`; null se sconosciuta. */
  name: string | null;
  branches: SeriesBranch[];
}

/** Da `moro1 patch/int8-coopmat@abcdef012` (Provenance::series nel backend) ai suoi pezzi. */
export function parseSeries(series: string | null | undefined): SeriesInfo {
  const text = series?.trim();
  if (!text) return { kind: "unknown", name: null, branches: [] };
  if (text === UPSTREAM_SERIES) return { kind: "upstream", name: UPSTREAM_SERIES, branches: [] };
  const [name, ...rest] = text.split(/\s+/);
  if (name.startsWith("moro?")) return { kind: "unreadable", name: "moro?", branches: [] };
  const branches = rest.map((piece) => {
    const at = piece.lastIndexOf("@");
    return at > 0 ? { ramo: piece.slice(0, at), commit: piece.slice(at + 1) || null } : { ramo: piece, commit: null };
  });
  return { kind: branches.length ? "patched" : "plain", name, branches };
}

/**
 * La serie in poche lettere, da mettere accanto alla build: `moro1 int8-coopmat`. Una build di
 * ggml-org non porta niente accanto (null); una serie sconosciuta si dice, perché non è upstream.
 */
export function seriesShort(series: string | null | undefined): string | null {
  const s = parseSeries(series);
  switch (s.kind) {
    case "upstream":
      return null;
    case "unknown":
      return "serie sconosciuta";
    case "unreadable":
      return "moro? illeggibile";
    default:
      return [s.name, ...s.branches.map((b) => b.ramo.replace(/^patch\//, ""))].join(" ");
  }
}

/** La spiegazione lunga della serie corta, per il title. */
export function seriesTitle(series: string | null | undefined): string {
  const s = parseSeries(series);
  switch (s.kind) {
    case "upstream":
      return "build scaricata da ggml-org, senza patch";
    case "unknown":
      return "serie di patch sconosciuta: l'avvio è precedente a quando Aethera registra le condizioni";
    case "unreadable":
      return "accanto a llama-server c'è un file di provenienza che non si legge: non si sa che serie sia";
    case "plain":
      return `${s.name}: il tag liscio compilato su questa macchina, senza patch`;
    default:
      return `${s.name}: ${s.branches.map((b) => `${b.ramo}${b.commit ? ` @ ${b.commit}` : ""}`).join(" · ")}`;
  }
}

/**
 * La build senza la serie in coda, quando la serie la ripete: `b10991+moro1` con serie `moro1 …`
 * diventa `b10991`, così accanto si legge `b10991 · moro1 int8-coopmat` e non due volte `moro1`.
 */
export function buildBase(build: string, series: string | null | undefined): string {
  const s = parseSeries(series);
  if (s.kind !== "plain" && s.kind !== "patched") return build;
  return build.endsWith(`+${s.name}`) ? build.slice(0, -(s.name!.length + 1)) : build;
}

/**
 * Due avvii con serie diverse? «same» solo se tutte e due sono note e uguali, commit dei rami
 * compresi: una serie ricompilata con una patch cambiata non è la stessa. Se una manca o non si
 * legge, non si sa.
 */
export function seriesDiffer(a: string | null | undefined, b: string | null | undefined): "same" | "different" | "unknown" {
  const known = (k: SeriesKind) => k !== "unknown" && k !== "unreadable";
  if (!known(parseSeries(a).kind) || !known(parseSeries(b).kind)) return "unknown";
  const norm = (s: string) => s.trim().split(/\s+/).join(" ");
  return norm(a!) === norm(b!) ? "same" : "different";
}

/** L'avviso del confronto quando le due build non portano la stessa serie; null se è la stessa. */
export function seriesAlert(a: string | null | undefined, b: string | null | undefined): AlertSpec | null {
  const verdict = seriesDiffer(a, b);
  if (verdict === "same") return null;
  const names = `A ${seriesShort(a) ?? UPSTREAM_SERIES} · B ${seriesShort(b) ?? UPSTREAM_SERIES}`;
  if (verdict === "unknown") {
    return {
      key: "serie",
      tone: "warn",
      title: "Serie di patch sconosciuta",
      detail: `${names}. Non si sa se le due build portano le stesse patch: il confronto potrebbe misurare anche una patch.`,
    };
  }
  if (parseSeries(a).kind !== "patched" && parseSeries(b).kind !== "patched") {
    return {
      key: "serie",
      tone: "warn",
      title: "Build compilate diversamente",
      detail: `${names}. Nessuna delle due porta patch, ma una è compilata qui e l'altra scaricata: il confronto misura anche la compilazione.`,
    };
  }
  return {
    key: "serie",
    tone: "warn",
    title: "Build con patch diverse",
    detail: `${names}. Il confronto misura anche la patch, non solo il profilo.`,
  };
}

/** Lo stesso avviso in una riga, per la fascia delle due spunte; la spiegazione sta nel title. */
export function seriesBadge(a: string | null | undefined, b: string | null | undefined): string | null {
  const alert = seriesAlert(a, b);
  if (!alert) return null;
  const title = alert.title.charAt(0).toLowerCase() + alert.title.slice(1);
  return alert.title === "Build con patch diverse" ? `${title}: il confronto misura anche la patch` : title;
}

// --- Provenienza delle build (Impostazioni) ------------------------------------------------

export type ProvenanceKind = "fork" | "upstream" | "missing" | "unreadable" | "unknown";

export interface ProvenanceView {
  kind: ProvenanceKind;
  tone: Tone;
  /** Etichetta corta della riga. */
  label: string;
  /** Spiegazione lunga, per il title. */
  help: string;
}

/** La serie come la registra il backend fra le condizioni (Provenance::series). */
export function provenanceSeries(p: Provenance): string {
  return [`moro${p.serie}`, ...(p.patch ?? []).map((b) => `${b.ramo}@${b.commit.slice(0, 9)}`)].join(" ");
}

/**
 * Che cosa dire della provenienza di una build. `entry` assente = Aethera non ha letto la cartella
 * (llama-server manca): sconosciuta, non upstream. Senza file la build è quella scaricata da
 * ggml-org, a meno che l'id dichiari una serie del fork (`b10991+moro1-vulkan`): allora la
 * provenienza dovrebbe esserci e manca.
 */
export function provenanceView(id: string, entry: BuildProvenance | null | undefined): ProvenanceView {
  if (!entry) {
    return {
      kind: "unknown",
      tone: "",
      label: "provenienza sconosciuta",
      help: "Aethera non ha trovato llama-server in questa cartella, quindi non ha cercato il file di provenienza.",
    };
  }
  if (entry.error) {
    return {
      kind: "unreadable",
      tone: "err",
      label: "provenienza illeggibile",
      help: `${entry.error} — finché non si legge non si sa che serie sia, e un profilo non può usare questa build.`,
    };
  }
  if (entry.provenance) {
    const p = entry.provenance;
    return {
      kind: "fork",
      tone: "acc",
      label: `${p.base} · ${seriesShort(provenanceSeries(p))}`,
      help: "Build compilata su questa macchina: provenienza.toml accanto a llama-server dice da che tag viene e che patch porta. Un profilo la usa solo se la chiede per nome.",
    };
  }
  if (/\+moro/.test(id)) {
    return {
      kind: "missing",
      tone: "warn",
      label: "provenienza assente",
      help: "L'id dichiara una serie del fork ma accanto a llama-server non c'è provenienza.toml: Aethera non la tratta come build del fork, e un profilo che la chiede per nome non parte.",
    };
  }
  return {
    kind: "upstream",
    tone: "",
    label: "build scaricata da ggml-org",
    help: "Accanto a llama-server non c'è provenienza.toml: Aethera la tratta come una build di ggml-org, senza patch.",
  };
}

/** Lo stesso percorso scritto da Windows in due modi: barre e maiuscole non fanno un'altra cartella. */
export function samePath(a: string, b: string): boolean {
  const norm = (p: string) => p.replace(/\\/g, "/").replace(/\/+$/, "").toLowerCase();
  return norm(a) === norm(b);
}
