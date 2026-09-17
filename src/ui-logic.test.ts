import { describe, expect, it } from "vitest";
import type { EngineStatus, Provenance, ReadyStatus, Summary } from "./api";
import {
  buildBase,
  parseSeries,
  provenanceSeries,
  provenanceView,
  samePath,
  seriesAlert,
  seriesBadge,
  seriesDiffer,
  seriesShort,
  seriesTitle,
  countBy,
  decodeJudgement,
  delta,
  engineAlerts,
  flip,
  memoryParts,
  reuseJudgement,
  shareTone,
  splitBuildId,
  stackSegments,
  stackTitle,
} from "./ui-logic";

const summary = (over: Partial<Summary> = {}): Summary => ({
  requests: 18, window: 18, prefill_median: 418, decode_median: 33.1, decode_p10: 29.4, decode_p90: 34,
  acceptance: 0.71, cache_share: 0.93, cache_series: [], decode_series: [], last_prompt: 9412,
  draft_n: 0, draft_accepted: 0, prompt_processed: 0, prompt_cached: 0, generated: 0, turns: [],
  compactions: [], cache_sources: ["log"], ...over,
});

const ready = (over: Partial<ReadyStatus> = {}): EngineStatus => ({
  state: "ready",
  run: {
    run_id: "r-1", profile: "G1", base: "G1", base_url: "http://127.0.0.1:8080", started_at: "2026-09-16T17:33:42",
    pid: 1, build: "b10991", command_line: "", log_path: "", manifest_path: "", ctx_declared: 65536, n_parallel: 1,
    overrides: [], invalidates_cache: false,
  },
  uptime_s: 60, load_ms: 12400, ctx_served: 65536, alias_served: "G1", divergences: [],
  usage: { in_use: false, reasons: [], protected: false, slot_processing: false, last_request_s: null, locks: [] },
  telemetry: summary(), counters: null, memory: null, degraded: [], conditions: null, conditions_changed: [],
  reference: null, ...over,
});

describe("Stack", () => {
  const parts = [
    { key: "a", label: "a", value: 24, cls: "ded" },
    { key: "b", label: "b", value: null, cls: "sha" },
    { key: "c", label: "c", value: 0, cls: "win" },
    { key: "d", label: "d", value: 48, cls: "fre" },
  ];

  it("i pezzi sconosciuti o nulli non si disegnano", () => {
    expect(stackSegments(parts, 96)).toEqual([
      { key: "a", cls: "ded", pct: 25 },
      { key: "d", cls: "fre", pct: 50 },
    ]);
  });

  it("senza totale non si disegna niente", () => {
    expect(stackSegments(parts, null)).toEqual([]);
    expect(stackSegments(parts, 0)).toEqual([]);
  });

  it("la somma non supera il 100 %", () => {
    const segs = stackSegments([{ key: "x", label: "x", value: 80, cls: "ded" }, { key: "y", label: "y", value: 80, cls: "win" }], 100);
    expect(segs.map((s) => s.pct)).toEqual([80, 20]);
  });

  it("il title dice anche lo sconosciuto", () => {
    expect(stackTitle(parts.slice(0, 2))).toBe("a 24,00 GiB · b sconosciuto");
  });

  it("memoria: VGM più RAM di Windows, la condivisa non si conta due volte", () => {
    const m = memoryParts(
      { at: "", after_ms: 6300, vram_dedicated_gib: 23.13, vram_shared_gib: 0.08, ram_available_gib: 24, ram_margin_gib: 8 },
      48,
      47.65,
    );
    expect(m.total).toBeCloseTo(95.65);
    const v = Object.fromEntries(m.parts.map((p) => [p.key, p.value]));
    expect(v.vfr).toBeCloseTo(24.87);
    expect(v.win).toBeCloseTo(23.57);
    expect(v.fre).toBe(24);
  });

  it("memoria: senza VGM il totale è sconosciuto", () => {
    const m = memoryParts({ at: "", after_ms: 0, vram_dedicated_gib: 23, ram_margin_gib: 8 }, null, 47.65);
    expect(m.total).toBeNull();
    expect(m.parts.find((p) => p.key === "vfr")!.value).toBeNull();
    expect(m.parts.find((p) => p.key === "fre")!.value).toBeNull();
  });
});

describe("Kpi", () => {
  it("quota riusata: verde dal 90 %, rossa sotto il 50 %", () => {
    expect(shareTone(0.93)).toBe("ok");
    expect(shareTone(0.9)).toBe("ok");
    expect(shareTone(0.6)).toBe("warn");
    expect(shareTone(0.49)).toBe("err");
    expect(shareTone(null)).toBe("");
    expect(reuseJudgement(null).text).toBe("nessuna richiesta attribuibile");
  });

  it("decode: giudicato solo contro il riferimento delle stesse condizioni", () => {
    expect(decodeJudgement(null, null).tone).toBe("");
    expect(decodeJudgement(33.1, null).text).toBe("nessun riferimento con queste condizioni");
    expect(decodeJudgement(33.1, { decode_median: 33, runs: 3 })).toEqual({ tone: "", text: "nella norma: riferimento 33,0 tok/s su 3 avvii" });
    expect(decodeJudgement(20, { decode_median: 33, runs: 1 })).toEqual({ tone: "err", text: "sotto il 70 % del riferimento (33,0 tok/s su 1 avvio)" });
  });
});

describe("Alerts", () => {
  it("un motore spento o orfano non ha avvisi qui", () => {
    expect(engineAlerts(null)).toEqual([]);
    expect(engineAlerts({ state: "off", last: null })).toEqual([]);
  });

  it("degradato, divergenze, condizioni e compattazione, nell'ordine e con i testi di prima", () => {
    const a = engineAlerts(
      ready({
        degraded: ["acceso da più di 24 h: prima di una misura conviene riavviare"],
        divergences: ["contesto servito 32768, dichiarato 65536"],
        conditions_changed: ["driver GPU 32.0.22042.1 → 32.0.31041.1004"],
        reference: { decode_median: 33, runs: 3 },
        telemetry: summary({
          compactions: [{ at: "2026-09-16T17:52:31", tasks: [1, 2], reprocessed: 26795, cost_ms: 121000, client: "claude-code" }],
        }),
      }),
    );
    expect(a.map((x) => x.key)).toEqual(["deg0", "div0", "cond", "cmp"]);
    expect(a[0].detail).toContain("«Riavvia con la stessa riga» quando il motore è libero.");
    expect(a[0].action).toBe("restart");
    expect(a[1].detail).toBe("contesto servito 32768, dichiarato 65536 — si registra, non si corregge");
    expect(a[2].detail).toContain("la mediana di riferimento riparte (33,0 tok/s, 3 avvii con queste condizioni finora).");
    expect(a[3].title).toBe("Compattazione alle 16-09 17:52:31");
    expect(a[3].detail).toBe(
      "di claude-code: 26.795 token rielaborati fra riassunto e contesto ricostruito, 121,0 s che la conversazione non avrebbe pagato continuando. Più contesto servito le rende più rare: vedi Budget di contesto nelle Impostazioni.",
    );
    expect(a[3].action).toBe("budget");
  });

  it("condizioni cambiate senza riferimento, e più compattazioni sommate", () => {
    const a = engineAlerts(
      ready({
        conditions_changed: ["alimentazione"],
        telemetry: summary({
          window: 20,
          compactions: [
            { at: "2026-09-16T17:00:00", tasks: [], reprocessed: 100, cost_ms: 1000, client: null },
            { at: "2026-09-16T17:10:00", tasks: [], reprocessed: 200, cost_ms: 2500, client: null },
          ],
        }),
      }),
    );
    expect(a[0].detail).toContain("riparte da qui (nessun avvio con queste condizioni finora).");
    expect(a[1].detail.startsWith("200 token")).toBe(true);
    expect(a[1].detail).toContain("Nelle ultime 20 richieste: 2 compattazioni, 3,5 s in tutto.");
  });
});

describe("Chips, Benchmark, build", () => {
  it("conta per chiave nell'ordine di comparsa", () => {
    expect(countBy(["G1", "G3", "G1"], (x) => x)).toEqual([
      { id: "G1", count: 2 },
      { id: "G3", count: 1 },
    ]);
  });

  it("Δ sotto il 3 % non si colora; la VRAM si legge al contrario", () => {
    expect(delta(418, 344)).toEqual({ text: "+22 %", dir: "up" });
    expect(delta(101, 100)).toEqual({ text: "+1 %", dir: "" });
    expect(delta(1, null)).toEqual({ text: null, dir: "" });
    expect(delta(1, 0)).toEqual({ text: null, dir: "" });
    expect(flip(delta(23.13, 22.68, 1).dir)).toBe("");
    expect(flip(delta(30, 20).dir)).toBe("dn");
  });

  it("id di build come machine::split_build_id", () => {
    expect(splitBuildId("b10809-win-vulkan-x64")).toEqual({ build: "b10809", backend: "vulkan" });
    expect(splitBuildId("b10985-vulkan")).toEqual({ build: "b10985", backend: "vulkan" });
    expect(splitBuildId("llama-cpu")).toEqual({ build: null, backend: "cpu" });
    // Una build del fork si propone con la sua serie: un profilo la chiede per nome.
    expect(splitBuildId("b10991+moro1-win-vulkan-x64")).toEqual({ build: "b10991+moro1", backend: "vulkan" });
    expect(splitBuildId("b10991+altro-vulkan")).toEqual({ build: null, backend: "vulkan" });
  });
});

describe("serie di patch (M-14)", () => {
  const MORO1 = "moro1 patch/int8-coopmat@abcdef012";

  it("si legge nei suoi pezzi, e quella che manca resta sconosciuta", () => {
    expect(parseSeries(MORO1)).toEqual({ kind: "patched", name: "moro1", branches: [{ ramo: "patch/int8-coopmat", commit: "abcdef012" }] });
    expect(parseSeries("moro0")).toEqual({ kind: "plain", name: "moro0", branches: [] });
    expect(parseSeries("ggml-org").kind).toBe("upstream");
    expect(parseSeries("moro? (provenienza illeggibile)")).toEqual({ kind: "unreadable", name: "moro?", branches: [] });
    for (const v of [null, undefined, "", "  "]) expect(parseSeries(v).kind).toBe("unknown");
    expect(parseSeries("moro2 patch/a@111 patch/b").branches).toEqual([
      { ramo: "patch/a", commit: "111" },
      { ramo: "patch/b", commit: null },
    ]);
  });

  it("forma corta accanto alla build: niente per ggml-org, mai niente per una sconosciuta", () => {
    expect(seriesShort(MORO1)).toBe("moro1 int8-coopmat");
    expect(seriesShort("moro2 patch/int8-coopmat@aaa patch/mtp@bbb")).toBe("moro2 int8-coopmat mtp");
    expect(seriesShort("moro0")).toBe("moro0");
    expect(seriesShort("ggml-org")).toBeNull();
    expect(seriesShort(null)).toBe("serie sconosciuta");
    expect(seriesShort("moro? (provenienza illeggibile)")).toBe("moro? illeggibile");
    expect(seriesTitle(MORO1)).toBe("moro1: patch/int8-coopmat @ abcdef012");
    expect(seriesTitle(null)).toContain("sconosciuta");
  });

  it("la build non ripete la serie che le sta accanto", () => {
    expect(buildBase("b10991+moro1", MORO1)).toBe("b10991");
    expect(buildBase("b10991+moro0", "moro0")).toBe("b10991");
    expect(buildBase("b10991", "ggml-org")).toBe("b10991");
    // Se la serie non si conosce, o non è quella scritta nella build, la build resta com'è.
    expect(buildBase("b10991+moro1", null)).toBe("b10991+moro1");
    expect(buildBase("b10991+moro2", MORO1)).toBe("b10991+moro2");
  });

  it("due serie sono la stessa solo se note e uguali, commit compresi", () => {
    expect(seriesDiffer(MORO1, MORO1)).toBe("same");
    expect(seriesDiffer("ggml-org", "ggml-org")).toBe("same");
    expect(seriesDiffer(MORO1, "ggml-org")).toBe("different");
    expect(seriesDiffer("moro0", "ggml-org")).toBe("different");
    expect(seriesDiffer(MORO1, "moro1 patch/int8-coopmat@999999999")).toBe("different");
    expect(seriesDiffer(MORO1, null)).toBe("unknown");
    expect(seriesDiffer(null, null)).toBe("unknown");
    expect(seriesDiffer("moro? (provenienza illeggibile)", "moro? (provenienza illeggibile)")).toBe("unknown");
  });

  it("l'avviso del confronto dice che cosa si sta misurando", () => {
    expect(seriesAlert(MORO1, MORO1)).toBeNull();
    const patched = seriesAlert(MORO1, "ggml-org");
    expect(patched).toMatchObject({ tone: "warn", title: "Build con patch diverse" });
    expect(patched?.detail).toContain("A moro1 int8-coopmat · B ggml-org");
    expect(patched?.detail).toContain("misura anche la patch");
    expect(seriesAlert("moro0", "ggml-org")?.title).toBe("Build compilate diversamente");
    // Una serie sconosciuta non passa per ggml-org: l'avviso c'è, e dice che non si sa.
    const unknown = seriesAlert(null, "ggml-org");
    expect(unknown?.title).toBe("Serie di patch sconosciuta");
    expect(unknown?.detail).toContain("A serie sconosciuta · B ggml-org");
    // Nella fascia delle due spunte, in una riga.
    expect(seriesBadge(MORO1, "ggml-org")).toBe("build con patch diverse: il confronto misura anche la patch");
    expect(seriesBadge(null, "ggml-org")).toBe("serie di patch sconosciuta");
    expect(seriesBadge("moro0", "moro0")).toBeNull();
  });
});

describe("provenienza delle build (Impostazioni)", () => {
  const prov: Provenance = {
    schema_version: 1, id: "b10991+moro1-vulkan", base: "b10991", commit_base: "930e2fa5995789ef", serie: 1, backend: "vulkan",
    commit: "0123456789abcdef", data: "2026-09-17T02:00:00+02:00", durata_build_s: 640,
    patch: [{ ramo: "patch/int8-coopmat", commit: "abcdef0123456789abcdef", commit_della_patch: ["aaaa vulkan: int8"] }],
  };

  it("la serie è la stessa stringa che il backend mette fra le condizioni", () => {
    expect(provenanceSeries(prov)).toBe("moro1 patch/int8-coopmat@abcdef012");
    expect(provenanceSeries({ ...prov, serie: 0, patch: undefined })).toBe("moro0");
  });

  it("i cinque casi restano distinti", () => {
    const dir = "D:\\llm\\builds\\x";
    expect(provenanceView("b10991+moro1-vulkan", { dir, provenance: prov, error: null })).toMatchObject({
      kind: "fork", label: "b10991 · moro1 int8-coopmat",
    });
    expect(provenanceView("b10991-vulkan", { dir, provenance: null, error: null })).toMatchObject({
      kind: "upstream", label: "build scaricata da ggml-org",
    });
    // L'id promette una serie ma il file non c'è: non è «scaricata da ggml-org».
    expect(provenanceView("b10991+moro1-vulkan", { dir, provenance: null, error: null })).toMatchObject({
      kind: "missing", label: "provenienza assente", tone: "warn",
    });
    const broken = provenanceView("b10991-vulkan", { dir, provenance: null, error: "provenienza.toml: riga 3" });
    expect(broken).toMatchObject({ kind: "unreadable", tone: "err" });
    expect(broken.help).toContain("riga 3");
    expect(provenanceView("b10991-vulkan", undefined).kind).toBe("unknown");
  });

  it("lo stesso percorso scritto in due modi è la stessa cartella", () => {
    expect(samePath("D:\\LLM\\builds\\x\\", "d:/llm/builds/x")).toBe(true);
    expect(samePath("D:\\llm\\builds\\x", "D:\\llm\\builds\\y")).toBe(false);
  });
});
