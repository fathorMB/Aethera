import { describe, expect, it } from "vitest";
import type { Profile } from "../api";
import { setPath } from "../format";
import { ADVANCED, baseText, ESSENTIAL, gpuMode, groupSummary, leverPaths, modifiedPaths, parse, toText } from "./leve";

/** I campi che la pagina Avvio della v1 mostrava: nella v2 nessuno deve sparire. */
const V1_FIELDS = [
  "model.file", "model.repo", "model.quant", "model.size_gb", "model.sha256",
  "runtime.kind", "runtime.backend", "runtime.build",
  "server.host", "server.port", "server.ctx", "server.n_parallel", "server.n_gpu_layers", "server.fit",
  "server.fit_target", "server.flash_attn", "server.cache_type_k", "server.cache_type_v", "server.load_mode",
  "server.ubatch", "server.batch", "server.threads", "server.threads_batch", "server.n_cpu_moe",
  "server.tensor_overrides", "server.lazy_mode", "server.chat_template_file", "server.metrics", "server.jinja",
  "server.slot_save", "server.extra_args",
  "speculative.type", "speculative.draft_n_max", "speculative.draft_n_min", "speculative.draft_p_min",
  "speculative.draft_model",
  "cache.cache_reuse", "cache.ctx_checkpoints", "cache.checkpoint_min_step", "cache.cache_ram", "cache.kv_unified",
];

const G1: Profile = {
  schema_version: 1,
  name: "G1",
  gate: "G1",
  notes: null,
  model: { repo: "Qwen/Qwen3.6-35B-A3B-GGUF", file: "Qwen3.6-35B-A3B-Q4_K_M.gguf", sha256: null, size_gb: 22.3, quant: "Q4_K_M" },
  runtime: { kind: "llama.cpp", backend: "vulkan", build: "b10809" },
  server: {
    host: "127.0.0.1", port: 8080, ctx: 65536, n_parallel: 1, n_gpu_layers: 999, flash_attn: "on",
    cache_type_k: "f16", cache_type_v: "f16", ubatch: 512, batch: 2048, load_mode: "auto",
    metrics: true, jinja: true, slot_save: true, extra_args: [],
  },
  speculative: { type: "draft-mtp", draft_n_min: 2, draft_n_max: 4, draft_p_min: 0.75 },
  cache: {},
};

describe("Field: le leve di Avvio", () => {
  it("dodici essenziali, come nel mockup approvato", () => {
    expect(ESSENTIAL).toHaveLength(12);
  });

  it("ogni campo della v1 sta in una leva sola, più le note del profilo", () => {
    const all = [...ESSENTIAL, ...ADVANCED].flatMap(leverPaths);
    expect(new Set(all).size).toBe(all.length);
    expect([...all].sort()).toEqual([...V1_FIELDS, "notes"].sort());
  });

  it("le leve marcate cache sono le stesse della v1", () => {
    const cached = [...ESSENTIAL, ...ADVANCED].filter((l) => l.cache).flatMap(leverPaths).sort();
    expect(cached).toEqual(
      ["server.ctx", "server.n_parallel", "cache.cache_reuse", "cache.ctx_checkpoints", "cache.checkpoint_min_step", "cache.cache_ram", "cache.kv_unified"].sort(),
    );
  });

  it("ogni verdetto porta la sua fonte", () => {
    for (const l of [...ESSENTIAL, ...ADVANCED].filter((x) => x.verdict)) expect(l.verdictTitle).toMatch(/M-08 T-\d\d/);
  });

  it("parse: assente non è zero, un numero sbagliato non passa", () => {
    expect(parse("optint", "")).toEqual({ ok: true, value: null });
    expect(parse("int", "")).toEqual({ ok: false });
    expect(parse("int", "12a")).toEqual({ ok: false });
    expect(parse("int", " 2048 ")).toEqual({ ok: true, value: 2048 });
    expect(parse("optfloat", "0,75")).toEqual({ ok: true, value: 0.75 });
    expect(parse("optbool", "")).toEqual({ ok: true, value: null });
    expect(parse("optbool", "off")).toEqual({ ok: true, value: false });
    expect(parse("bool", "on")).toEqual({ ok: true, value: true });
    expect(parse("list", "  a=CPU   b=GPU ")).toEqual({ ok: true, value: ["a=CPU", "b=GPU"] });
    expect(parse("opttext", "  ")).toEqual({ ok: true, value: null });
  });

  it("toText: booleani on/off, liste separate da spazi, assente vuoto", () => {
    expect(toText(true)).toBe("on");
    expect(toText(["a", "b"])).toBe("a b");
    expect(toText(null)).toBe("");
    expect(toText(0)).toBe("0");
  });

  it("una leva è modificata se lo è una delle sue vie, e mostra il valore di prima", () => {
    const batch = ESSENTIAL.find((l) => l.id === "batch")!;
    const edited = setPath(G1, "server.ubatch", 2048);
    expect(modifiedPaths(batch, G1, G1)).toEqual([]);
    expect(modifiedPaths(batch, G1, edited)).toEqual(["server.ubatch"]);
    expect(baseText(batch, G1, edited)).toBe("512");
    // Un profilo nuovo non ha base: niente è «modificato».
    expect(modifiedPaths(batch, null, edited)).toEqual([]);
  });

  it("vuoto e assente sono lo stesso valore", () => {
    const extra = ADVANCED.find((l) => l.id === "extra")!;
    expect(modifiedPaths(extra, G1, setPath(G1, "server.extra_args", undefined))).toEqual([]);
  });

  it("il riassunto di Avanzate conta le leve modificate e i verdetti", () => {
    const edited = setPath(setPath(G1, "server.n_cpu_moe", 16), "server.threads", 12);
    const s = groupSummary(ADVANCED, G1, edited);
    expect(s.modified).toBe(2);
    expect(s.verdicts).toBe(3);
    expect(s.levers).toBe(ADVANCED.length);
  });

  it("layer sulla GPU: 999 tutti, assente decide fit", () => {
    expect(gpuMode(999)).toBe("all");
    expect(gpuMode(null)).toBe("fit");
    expect(gpuMode(undefined)).toBe("fit");
    expect(gpuMode(40)).toBe("num");
  });
});
