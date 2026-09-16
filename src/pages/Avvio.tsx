import { open } from "@tauri-apps/plugin-dialog";
import { createEffect, createMemo, createSignal, For, Match, on, onCleanup, onMount, Show, Switch } from "solid-js";
import * as api from "../api";
import type { EngineStatus, Issue, Overview, Preview, Profile, ProfileEntry } from "../api";
import { AskName, CommandLine, Confirm, copy, Empty, Val } from "../components";
import { fixed, getPath, sameValue, setPath, show } from "../format";

type Kind = "text" | "opttext" | "int" | "optint" | "optfloat" | "bool" | "optbool" | "select" | "optselect" | "list";

interface FieldDef {
  path: string;
  label: string;
  kind: Kind;
  options?: string[];
  cache?: boolean;
  hint?: string;
  /** Che cosa ne ha concluso M-08 su questa macchina: informa, non impedisce. */
  verdict?: string;
}

const M08 = "rapporto in .lmbrain-lite/reports/misure-motore-2026-09.md";

const FLASH = ["on", "off", "auto"];
const CACHE_TYPES = ["f32", "f16", "bf16", "q8_0", "q4_0", "q4_1", "iq4_nl", "q5_0", "q5_1"];
const LOAD = ["auto", "none", "mmap", "mlock", "mmap+mlock", "dio"];

const SECTIONS: { title: string; fields: FieldDef[] }[] = [
  {
    title: "Modello",
    fields: [
      { path: "model.file", label: "file", kind: "text", hint: "nella cartella pesi" },
      { path: "model.repo", label: "repo", kind: "opttext" },
      { path: "model.quant", label: "quant", kind: "opttext" },
      { path: "model.size_gb", label: "size_gb", kind: "optfloat", hint: "GB decimali" },
      { path: "model.sha256", label: "sha256", kind: "opttext" },
    ],
  },
  {
    title: "Motore",
    fields: [
      { path: "runtime.kind", label: "kind", kind: "text" },
      { path: "runtime.backend", label: "backend", kind: "text" },
      { path: "runtime.build", label: "build", kind: "text", hint: "fissata" },
    ],
  },
  {
    title: "Server",
    fields: [
      { path: "server.host", label: "host", kind: "text" },
      { path: "server.port", label: "port", kind: "int", hint: "sempre esplicita" },
      { path: "server.ctx", label: "ctx", kind: "int", cache: true, hint: "verificato su /props" },
      { path: "server.n_parallel", label: "n_parallel", kind: "int", cache: true, hint: "MTP richiede 1" },
      { path: "server.n_gpu_layers", label: "n_gpu_layers", kind: "optint", hint: "vuoto: decide fit" },
      { path: "server.fit", label: "fit", kind: "optselect", options: ["on", "off"], hint: "default motore: on" },
      { path: "server.fit_target", label: "fit_target", kind: "opttext", hint: "MiB liberi · 1024 o 1024,512" },
      { path: "server.flash_attn", label: "flash_attn", kind: "select", options: FLASH },
      { path: "server.cache_type_k", label: "cache_type_k", kind: "select", options: CACHE_TYPES },
      { path: "server.cache_type_v", label: "cache_type_v", kind: "select", options: CACHE_TYPES },
      { path: "server.load_mode", label: "load_mode", kind: "select", options: LOAD },
      { path: "server.ubatch", label: "ubatch", kind: "int" },
      { path: "server.batch", label: "batch", kind: "int" },
      { path: "server.threads", label: "threads", kind: "optint", hint: "default motore" },
      { path: "server.threads_batch", label: "threads_batch", kind: "optint", hint: "default motore" },
      {
        path: "server.n_cpu_moe",
        label: "n_cpu_moe",
        kind: "optint",
        verdict: "M-08: peggiora",
        hint: "M-08 T-08: sul Coder-Next ogni layer di esperti sulla CPU toglie velocità, in modo monotono",
      },
      { path: "server.tensor_overrides", label: "tensor_overrides", kind: "list", hint: "-ot · regex=buffer, separati da spazi" },
      { path: "server.lazy_mode", label: "lazy_mode", kind: "optselect", options: ["auto", "on", "off"], hint: "default motore: auto" },
      {
        path: "server.chat_template_file",
        label: "chat_template_file",
        kind: "opttext",
        hint: "vuoto: del modello · qwen3.6-tollerante.jinja per Claude Code",
      },
      { path: "server.metrics", label: "metrics", kind: "bool" },
      { path: "server.jinja", label: "jinja", kind: "bool" },
      { path: "server.slot_save", label: "slot_save", kind: "bool", hint: "runs/<id>/slots" },
      { path: "server.extra_args", label: "extra_args", kind: "list", hint: "via di fuga" },
    ],
  },
  {
    title: "Speculazione",
    fields: [
      { path: "speculative.type", label: "type", kind: "text", hint: "none · draft-mtp · ngram-mod" },
      { path: "speculative.draft_n_max", label: "draft_n_max", kind: "optint", hint: "default motore" },
      { path: "speculative.draft_n_min", label: "draft_n_min", kind: "optint", hint: "default motore" },
      { path: "speculative.draft_p_min", label: "draft_p_min", kind: "optfloat", hint: "default motore" },
      { path: "speculative.draft_model", label: "draft_model", kind: "opttext" },
    ],
  },
  {
    title: "Cache e checkpoint",
    fields: [
      {
        path: "cache.cache_reuse",
        label: "cache_reuse",
        kind: "optint",
        cache: true,
        verdict: "M-08: scartata",
        hint: "M-08 T-13: con --cache-reuse 256 i turni costano come senza",
      },
      {
        path: "cache.ctx_checkpoints",
        label: "ctx_checkpoints",
        kind: "optint",
        cache: true,
        verdict: "M-08: scartata",
        hint: "M-08 T-04: stessi token riusati, turno per turno",
      },
      {
        path: "cache.checkpoint_min_step",
        label: "checkpoint_min_step",
        kind: "optint",
        cache: true,
        verdict: "M-08: scartata",
        hint: "M-08 T-04: nessun effetto sul riuso",
      },
      {
        path: "cache.cache_ram",
        label: "cache_ram",
        kind: "optint",
        cache: true,
        verdict: "M-08: scartata",
        hint: "MiB · -1 senza limite · 0 spenta. M-08 T-04: nessun effetto sul riuso",
      },
      {
        path: "cache.kv_unified",
        label: "kv_unified",
        kind: "optbool",
        cache: true,
        verdict: "M-08: scartata",
        hint: "M-08 T-04: nessun effetto sul riuso",
      },
    ],
  },
];

function parse(kind: Kind, raw: string): { ok: true; value: unknown } | { ok: false } {
  const t = raw.trim();
  switch (kind) {
    case "text":
    case "select":
      return { ok: true, value: raw };
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

function FieldRow(props: {
  def: FieldDef;
  base: Profile | null;
  edited: Profile;
  issues: Issue[];
  onSet: (path: string, value: unknown) => void;
}) {
  const [bad, setBad] = createSignal(false);
  const value = () => getPath(props.edited, props.def.path);
  const baseValue = () => (props.base ? getPath(props.base, props.def.path) : undefined);
  const modified = () => props.base != null && !sameValue(baseValue(), value());
  const issues = () => props.issues.filter((i) => i.field === props.def.path);
  const text = () => {
    const v = value();
    if (v == null) return "";
    if (typeof v === "boolean") return v ? "on" : "off";
    return Array.isArray(v) ? v.join(" ") : String(v);
  };
  const options = () => (props.def.kind === "optbool" ? ["on", "off"] : (props.def.options ?? []));
  const optional = () => props.def.kind === "optselect" || props.def.kind === "optbool";
  const onInput = (raw: string) => {
    const r = parse(props.def.kind, raw);
    setBad(!r.ok);
    if (r.ok) props.onSet(props.def.path, r.value);
  };

  return (
    <>
      <div class="field" classList={{ mod: modified() }} style={bad() || issues().length ? { outline: "1px solid var(--err)" } : {}}>
        <label>
          {props.def.label}{" "}
          <Show when={props.def.cache}>
            <span class="pill cache">cache</span>
          </Show>
        </label>
        <Show
          when={props.def.kind === "bool"}
          fallback={
            <Show
              when={props.def.kind === "select" || optional()}
              fallback={
                <input
                  value={text()}
                  placeholder={props.def.kind.startsWith("opt") ? "non impostato" : ""}
                  onInput={(e) => onInput(e.currentTarget.value)}
                  spellcheck={false}
                />
              }
            >
              <select value={text()} onChange={(e) => onInput(e.currentTarget.value)}>
                <Show when={optional()}>
                  <option value="">non impostato</option>
                </Show>
                <For each={options()}>{(o) => <option value={o}>{o}</option>}</For>
              </select>
            </Show>
          }
        >
          <select value={value() ? "on" : "off"} onChange={(e) => props.onSet(props.def.path, e.currentTarget.value === "on")}>
            <option value="on">on</option>
            <option value="off">off</option>
          </select>
        </Show>
        <Show
          when={modified()}
          fallback={
            <Show when={props.def.verdict} fallback={<span class="cond">{props.def.hint ?? ""}</span>}>
              <span class="pill x" title={`${props.def.hint ?? ""} — ${M08}`}>
                {props.def.verdict}
              </span>
            </Show>
          }
        >
          <span class="base">
            <s>{show(baseValue())}</s>
          </span>
        </Show>
      </div>
      <For each={issues()}>{(i) => <div class="cond" style={{ color: "var(--err)", "padding-left": "162px" }}>{i.message}</div>}</For>
    </>
  );
}

/** Il dialogo aperto: uno alla volta, e nessuno usa `window.prompt`, che nella WebView non c'è. */
type Ask =
  | { kind: "duplica"; name: string }
  | { kind: "rinomina"; name: string }
  | { kind: "elimina"; name: string; lines: string[] };

export default function Avvio(props: {
  overview: Overview;
  status: EngineStatus | null;
  onStarted: () => void;
  /** Pesi scelti dal Catalogo con «Avvia…»: il profilo che li usa, o uno nuovo su misura. */
  pendingModel?: string | null;
  onPendingHandled?: () => void;
}) {
  const [entries, setEntries] = createSignal<ProfileEntry[]>([]);
  const [selected, setSelected] = createSignal("");
  const [edited, setEdited] = createSignal<Profile | null>(null);
  const [preview, setPreview] = createSignal<Preview | null>(null);
  const [message, setMessage] = createSignal<{ kind: "ok" | "err"; text: string } | null>(null);
  const [newName, setNewName] = createSignal("");
  const [importBuild, setImportBuild] = createSignal(props.overview.builds[0]?.id.split("-")[0] ?? "b10809");
  const [busy, setBusy] = createSignal(false);
  const [ask, setAsk] = createSignal<Ask | null>(null);

  const entry = createMemo(() => entries().find((e) => e.name === selected()) ?? null);
  const base = () => entry()?.profile ?? null;
  const engineOn = () => api.isEngineOn(props.status);

  const select = (name: string, list = entries()) => {
    setSelected(name);
    const e = list.find((x) => x.name === name);
    setEdited(e?.profile ? structuredClone(e.profile) : null);
    setNewName(e ? `${e.name}.variante` : "");
  };

  const reload = async (keep?: string) => {
    try {
      const list = await api.listProfiles();
      setEntries(list);
      const name = keep && list.some((e) => e.name === keep) ? keep : list[0]?.name ?? "";
      select(name, list);
      return list;
    } catch (e) {
      setMessage({ kind: "err", text: String(e) });
      return [] as ProfileEntry[];
    }
  };

  /** Un profilo nuovo esiste solo in finestra finché non è salvato: `selected()` resta vuoto. */
  const creating = () => selected() === "" && edited() != null;

  const newProfile = (modelFile: string | null) =>
    act(async () => {
      const p = await api.profileTemplate(modelFile);
      setSelected("");
      setEdited(p);
      setNewName(p.name);
      setMessage({
        kind: "ok",
        text: `Profilo nuovo «${p.name}»: valori sensati, tutti espliciti. Correggi quello che serve e salvalo — finché non è salvato non si avvia, perché un avvio cita il profilo da cui è partito.`,
      });
    });

  /** I pesi arrivati dal Catalogo: il profilo che già li usa, o uno nuovo costruito su di loro. */
  const applyPending = (file: string, list: ProfileEntry[]) => {
    props.onPendingHandled?.();
    const hit = list.find((e) => e.profile?.model.file.toLowerCase() === file.toLowerCase());
    if (hit) {
      select(hit.name, list);
      setMessage({ kind: "ok", text: `«${file}» è già usato dal profilo ${hit.name}: eccolo.` });
    } else {
      newProfile(file);
    }
  };

  onMount(async () => {
    const list = await reload();
    if (props.pendingModel) applyPending(props.pendingModel, list);
  });

  createEffect(
    on(edited, (p) => {
      if (!p) {
        setPreview(null);
        return;
      }
      const t = setTimeout(async () => {
        try {
          setPreview(await api.preview(selected(), p));
        } catch (e) {
          setMessage({ kind: "err", text: String(e) });
        }
      }, 120);
      onCleanup(() => clearTimeout(t));
    }),
  );

  const set = (path: string, value: unknown) => {
    const p = edited();
    if (p) setEdited(setPath(p, path, value));
  };

  const act = async (fn: () => Promise<void>) => {
    setBusy(true);
    setMessage(null);
    try {
      await fn();
    } catch (e) {
      setMessage({ kind: "err", text: String(e) });
    } finally {
      setBusy(false);
    }
  };

  const start = () =>
    act(async () => {
      const r = await api.engineStart(selected(), edited()!);
      setMessage({ kind: "ok", text: `Avvio ${r.run_id} partito.` });
      props.onStarted();
    });

  const saveNew = () =>
    act(async () => {
      const name = newName().trim();
      const saved = await api.saveProfile({ ...edited()!, name }, null);
      setMessage({ kind: "ok", text: `Salvato come ${saved}.` });
      await reload(saved);
    });

  const update = () =>
    act(async () => {
      const saved = await api.saveProfile(edited()!, selected());
      setMessage({ kind: "ok", text: `Profilo ${saved} aggiornato.` });
      await reload(saved);
    });

  const duplicate = (to: string) =>
    act(async () => {
      const saved = await api.profileDuplicate(selected(), to);
      setAsk(null);
      setMessage({ kind: "ok", text: `«${selected()}» duplicato in ${saved}.` });
      await reload(saved);
    });

  const rename = (to: string) =>
    act(async () => {
      const from = selected();
      const saved = await api.profileRename(from, to);
      setAsk(null);
      setMessage({
        kind: "ok",
        text: `«${from}» ora si chiama ${saved}: cambia anche l'alias servito, quindi i client che lo citano vanno aggiornati. Gli avvii già registrati restano come sono.`,
      });
      await reload(saved);
    });

  const askDelete = () =>
    act(async () => {
      const name = selected();
      const plan = await api.profileDeletionPlan(name);
      setAsk({
        kind: "elimina",
        name,
        lines: [`Il file ${plan.file} viene cancellato.`, ...plan.warnings],
      });
    });

  const remove = () =>
    act(async () => {
      const name = selected();
      await api.profileDelete(name);
      setAsk(null);
      setMessage({ kind: "ok", text: `Profilo ${name} cancellato. Gli avvii che lo citano restano nello storico.` });
      await reload();
    });

  /** Campionamento consigliato che il catalogo conosce per questi pesi, con fonte e data. */
  const takeSampling = () =>
    act(async () => {
      const p = edited()!;
      const found = await api.catalogSampling(p.model.file);
      const modes = Object.keys(found);
      if (!modes.length) {
        setMessage({
          kind: "err",
          text: `Il catalogo non conosce nessun campionamento per «${p.model.file}»: leggi la model card dalla pagina Catalogo.`,
        });
        return;
      }
      setEdited({ ...p, sampling_by_mode: { ...p.sampling_by_mode, ...found } });
      setMessage({ kind: "ok", text: `Campionamento preso dal catalogo: ${modes.join(" · ")}. Salva il profilo per tenerlo.` });
    });

  const importJson = () =>
    act(async () => {
      const picked = await open({ multiple: true, filters: [{ name: "Profili minis-config", extensions: ["json"] }] });
      if (!picked) return;
      const paths = Array.isArray(picked) ? picked : [picked];
      const lines: string[] = [];
      let last: string | undefined;
      let failed = false;
      for (const path of paths) {
        try {
          const r = await api.importMinis(path, importBuild());
          last = r.name;
          lines.push(`✓ ${r.name}`, ...r.notes.map((n) => `   ${n}`));
        } catch (e) {
          failed = true;
          lines.push(`✗ ${path}: ${e}`);
        }
      }
      setMessage({ kind: failed ? "err" : "ok", text: lines.join("\n") });
      await reload(last);
    });

  const binary = () => preview()?.binary ?? "llama-server";
  const canStart = () =>
    !busy() && !engineOn() && !creating() && preview() != null && preview()!.blockers.length === 0;

  return (
    <section>
      <h1>Avvio</h1>
      <p class="sub">
        Scegli un profilo; ogni leva che cambi resta in sovrapposizione (●) e aggiorna la riga di comando. Puoi avviare
        così (il manifest registra profilo + differenze) o salvare come nuovo profilo.
      </p>

      <Show when={message()}>
        {(m) => (
          <pre class={`note ${m().kind === "err" ? "err" : ""} mb`} style={{ "white-space": "pre-wrap", margin: "0 0 12px" }}>
            {m().text}
          </pre>
        )}
      </Show>

      <div class="split">
        <div>
          <h2>Profili</h2>
          <div class="mini mono" style={{ "word-break": "break-all", margin: "-4px 0 8px" }}>
            {props.overview.data_root}\profiles
          </div>
          <div class="row" style={{ margin: "0 0 8px" }}>
            <button class="btn sm primary" disabled={busy()} onClick={() => newProfile(null)}>
              Nuovo profilo…
            </button>
            <button class="btn sm" disabled={busy() || !selected()} onClick={() => setAsk({ kind: "duplica", name: selected() })}>
              Duplica…
            </button>
            <button class="btn sm" disabled={busy() || !selected()} onClick={() => setAsk({ kind: "rinomina", name: selected() })}>
              Rinomina…
            </button>
            <button class="btn sm danger" disabled={busy() || !selected()} onClick={askDelete}>
              Elimina…
            </button>
          </div>
          <div class="list">
            <Show when={creating()}>
              <div class="it on">
                <div class="n">{edited()!.name}</div>
                <div class="m">
                  <span class="pill mod">non ancora salvato</span>
                </div>
              </div>
            </Show>
            <For
              each={entries()}
              fallback={
                <Show when={!creating()}>
                  <div class="cond">
                    Nessun profilo ancora. «Nuovo profilo…» ne scrive uno con valori sensati sul modello che scegli;
                    se vieni da minis-config, «Importa JSON…» li converte.
                  </div>
                </Show>
              }
            >
              {(e) => (
                <div class="it" classList={{ on: e.name === selected() }} onClick={() => select(e.name)}>
                  <div class="n">{e.name}</div>
                  <div class="m">
                    <Show when={e.profile?.gate}>
                      <span class="pill g">{e.profile!.gate}</span>
                    </Show>
                    <Show when={e.profile?.model.size_gb}>
                      <span>{String(e.profile!.model.size_gb).replace(".", ",")} GB</span>
                    </Show>
                    <Show when={e.profile && e.profile.speculative.type !== "none"}>
                      <span>{e.profile!.speculative.type}</span>
                    </Show>
                    <Show when={e.issues.length}>
                      <span class="badge err tight">
                        <i />
                        {e.issues.length} {e.issues.length === 1 ? "errore" : "errori"}
                      </span>
                    </Show>
                    <Show when={e.name === selected() && (preview()?.overrides.length ?? 0) > 0}>
                      <span class="pill mod">
                        {preview()!.overrides.length} {preview()!.overrides.length === 1 ? "modifica" : "modifiche"}
                      </span>
                    </Show>
                  </div>
                </div>
              )}
            </For>
          </div>
          <hr />
          <div class="card mb">
            <h2>
              Stima memoria <span class="r">prima dell'avvio</span>
            </h2>
            <Show when={preview()?.estimate} fallback={<div class="cond">Scegli un profilo per vedere la stima.</div>}>
              {(e) => {
                const gb = (b: number | null) => (b == null ? null : fixed(b / 1e9, 2));
                const vgm = () => props.overview.system.gpus.find((g) => g.dedicated_gib)?.dedicated_gib ?? null;
                const quota = () => {
                  const t = e().total_bytes;
                  const v = vgm();
                  return t && v ? Math.min(100, (t / (v * 1024 ** 3)) * 100) : 0;
                };
                return (
                  <>
                    <dl class="kv">
                      <dt>Pesi</dt>
                      <dd class="num">
                        <Val v={gb(e().weights_bytes)} unit="GB" />
                      </dd>
                      <dt>
                        Cache KV <span class="cond">@{edited()?.server.ctx}</span>
                      </dt>
                      <dd class="num">
                        <Val v={gb(e().kv_bytes)} unit="GB" />
                      </dd>
                      <Show when={e().state_bytes}>
                        <dt>Stato ricorrente</dt>
                        <dd class="num">
                          <Val v={gb(e().state_bytes)} unit="GB" />
                        </dd>
                      </Show>
                      <dt>Buffer di calcolo</dt>
                      <dd class="num">
                        <Val v={gb(e().compute_bytes)} unit="GB" />
                        <Show when={e().compute_from}>
                          <span class="cond"> misurato su {e().compute_from}</span>
                        </Show>
                      </dd>
                      <dt>{e().total_is_lower_bound ? "Totale minimo" : "Totale stimato"}</dt>
                      <dd class="num">
                        <b>
                          {e().total_is_lower_bound ? "≥ " : "~ "}
                          {gb(e().total_bytes)} GB
                        </b>
                        <Show when={vgm()}>
                          <span class="cond"> / {fixed(vgm()!, 0)} GiB dedicati</span>
                        </Show>
                      </dd>
                    </dl>
                    <Show when={vgm()}>
                      <div class={`bar ${quota() > 95 ? "err" : quota() > 85 ? "warn" : "ok"}`} style={{ "margin-top": "6px" }}>
                        <span style={{ width: `${quota()}%` }} />
                      </div>
                    </Show>
                    <For each={e().notes}>
                      {(n) => (
                        <div class="cond" style={{ "margin-top": "4px" }}>
                          {n}
                        </div>
                      )}
                    </For>
                    <div class="cond" style={{ "margin-top": "6px" }}>
                      Stima, sostituita dalla misura dopo l'avvio.
                    </div>
                  </>
                );
              }}
            </Show>
          </div>

          <div class="card">
            <h2>Importa da minis-config</h2>
            <div class="field" style={{ "grid-template-columns": "60px 1fr" }}>
              <label>build</label>
              <input value={importBuild()} onInput={(e) => setImportBuild(e.currentTarget.value)} />
            </div>
            <div class="cond" style={{ margin: "4px 0 8px" }}>
              I JSON non dicono la build: si fissa questa.
            </div>
            <div class="row">
              <button class="btn sm" disabled={busy()} onClick={importJson}>
                Importa JSON…
              </button>
              <button class="btn sm" disabled={busy()} onClick={() => reload(selected())}>
                Rileggi
              </button>
            </div>
          </div>
        </div>

        <div>
          <Show when={!edited() && !entries().length}>
            <Empty title="Un profilo dice come si accende il motore.">
              <div>
                Modello, contesto, tipi di cache, speculazione, porta: tutto esplicito, niente lasciato al default del
                motore, così la riga di comando racconta per intero com'è stato avviato. Il nome del profilo è anche
                l'alias che i client chiedono.
              </div>
            </Empty>
          </Show>
          <Show when={!edited() && entries().length > 0}>
            <Empty title="Scegli un profilo dall'elenco.">
              <div>Le sue leve compaiono qui, e ogni modifica aggiorna la riga di comando e la stima di memoria.</div>
            </Empty>
          </Show>
          <Show when={entry() && !entry()!.profile}>
            <div class="card mb">
              <h2>{entry()!.name}.toml non leggibile</h2>
              <ul class="errors">
                <For each={entry()!.issues}>{(i) => <li>{i.message}</li>}</For>
              </ul>
            </div>
          </Show>

          <Show when={edited()}>
            {(p) => (
              <>
                <div class="card mb">
                  <h2>
                    Profilo{" "}
                    <span class="r mono">
                      {entry()?.file ?? `${props.overview.data_root}\\profiles\\${p().name}.toml · da salvare`}
                    </span>
                  </h2>
                  <FieldRow def={{ path: "name", label: "name · alias", kind: "text" }} base={base()} edited={p()} issues={preview()?.issues ?? []} onSet={set} />
                  <For each={preview()?.warnings ?? []}>
                    {(w) => (
                      <div class="note err" style={{ "margin-top": "8px" }}>
                        {w} Si può avviare lo stesso.
                      </div>
                    )}
                  </For>
                  <Show when={preview()?.proposals.length}>
                    <div class="note" style={{ "margin-top": "8px" }}>
                      <b>Le misure di M-08 suggeriscono {preview()!.proposals.length === 1 ? "una modifica" : `${preview()!.proposals.length} modifiche`}</b>{" "}
                      per questo profilo. Il file non si tocca: si applicano sopra, le vedi nella riga di comando e le salvi tu.
                      <table style={{ margin: "6px 0", "font-size": "12px" }}>
                        <tbody>
                          <For each={preview()!.proposals}>
                            {(x) => (
                              <tr>
                                <td class="mono">{x.field}</td>
                                <td class="mono">
                                  <s>{show(x.current)}</s> → {show(x.value)}
                                </td>
                                <td class="cond">{x.reason}</td>
                              </tr>
                            )}
                          </For>
                        </tbody>
                      </table>
                      <button class="btn sm" onClick={() => preview()!.proposals.forEach((x) => set(x.field, x.value))}>
                        Applica come modifiche
                      </button>
                    </div>
                  </Show>
                  <Show when={p().notes}>
                    <div class="cond" style={{ padding: "4px 6px" }}>
                      {p().notes}
                    </div>
                  </Show>
                </div>

                <div class="grid g2 mb" style={{ "align-items": "start" }}>
                  <For each={SECTIONS}>
                    {(sec) => (
                      <div class="card" style={sec.title === "Server" ? { "grid-row": "span 3" } : {}}>
                        <h2>{sec.title}</h2>
                        <For each={sec.fields}>
                          {(def) => <FieldRow def={def} base={base()} edited={p()} issues={preview()?.issues ?? []} onSet={set} />}
                        </For>
                      </div>
                    )}
                  </For>
                </div>

                <div class="card mb">
                  <h2>
                    Campionamento consigliato <span class="r">dato del modello · non è un default del server</span>
                    <button class="btn sm right" disabled={busy() || !p().model.file} onClick={takeSampling}>
                      Prendi dal catalogo
                    </button>
                  </h2>
                  <Show
                    when={p().sampling_by_mode && Object.keys(p().sampling_by_mode!).length}
                    fallback={
                      <div class="cond">
                        Nessuno: il catalogo lo impara dalla model card del publisher (pagina Catalogo → «Leggi la model
                        card»), poi lo si porta qui. Quello che finisce nel profilo esce anche nelle righe per i client.
                      </div>
                    }
                  >
                    <table>
                      <thead>
                        <tr>
                          <th>modalità</th>
                          <th class="r">temp</th>
                          <th class="r">top_p</th>
                          <th class="r">top_k</th>
                          <th class="r">min_p</th>
                          <th class="r">presence</th>
                          <th>fonte</th>
                        </tr>
                      </thead>
                      <tbody>
                        <For each={Object.entries(p().sampling_by_mode!)}>
                          {([mode, sm]) => (
                            <tr>
                              <td>{mode}</td>
                              <td class="r num">{show(sm.temperature)}</td>
                              <td class="r num">{show(sm.top_p)}</td>
                              <td class="r num">{show(sm.top_k)}</td>
                              <td class="r num">{show(sm.min_p)}</td>
                              <td class="r num">{show(sm.presence_penalty)}</td>
                              <td class="mini">
                                {show(sm.source)}
                                {sm.verified ? ` · ${sm.verified}` : ""}
                              </td>
                            </tr>
                          )}
                        </For>
                      </tbody>
                    </table>
                    <div class="cond" style={{ "margin-top": "6px" }}>
                      Esce anche nelle righe per i client (Impostazioni), con fonte e data: il campionamento lo manda il
                      client a ogni richiesta, llama-server non lo applica da solo.
                    </div>
                  </Show>
                </div>

                <div class="card">
                  <h2>
                    Riga di comando{" "}
                    <span class="r">
                      aggiornata in tempo reale · {preview()?.overrides.length ?? 0}{" "}
                      {preview()?.overrides.length === 1 ? "differenza" : "differenze"} dal profilo
                    </span>
                  </h2>
                  <Show when={preview()}>{(pv) => <CommandLine binary={binary()} args={pv().args} />}</Show>
                  <Show when={preview()?.invalidates_cache}>
                    <div class="note warn" style={{ "margin-top": "8px" }}>
                      Hai cambiato una leva marcata <span class="pill cache">cache</span>: il manifest dichiarerà l'avvio
                      come «invalida la cache».
                    </div>
                  </Show>
                  <Show when={preview()?.blockers.length}>
                    <div class="note err" style={{ "margin-top": "8px" }}>
                      <For each={preview()!.blockers}>{(b) => <div>{b}</div>}</For>
                    </div>
                  </Show>
                  <div class="row" style={{ "margin-top": "10px" }}>
                    <button class="btn primary" disabled={!canStart()} onClick={start}>
                      {preview()?.overrides.length ? "Avvia con le modifiche" : "Avvia"}
                    </button>
                    <button class="btn" disabled={!preview()?.overrides.length || busy()} onClick={() => select(selected())}>
                      Scarta le modifiche
                    </button>
                    <button
                      class="btn"
                      disabled={!preview()?.overrides.length || busy() || p().name !== selected() || !!preview()?.issues.length}
                      onClick={update}
                    >
                      Aggiorna il profilo
                    </button>
                    <button class="btn sm" onClick={() => copy(preview()?.line ?? "")}>
                      Copia riga
                    </button>
                    <Show when={creating()}>
                      <span class="right cond">
                        Profilo non ancora salvato: un avvio cita il profilo da cui è partito, quindi prima si salva.
                      </span>
                    </Show>
                    <Show when={engineOn()}>
                      <span class="right cond">
                        {api.inUse(props.status)
                          ? "Il motore acceso è IN USO: l'avvio parte solo quando torna libero e viene fermato."
                          : "Un motore è già acceso: nella v1 se ne avvia uno alla volta."}
                      </span>
                    </Show>
                    <Show when={props.status?.state === "orphan"}>
                      <span class="right cond">Un llama-server orfano occupa una porta dei profili: vedi la pagina Motore.</span>
                    </Show>
                  </div>
                  <div class="row" style={{ "margin-top": "8px" }}>
                    <input
                      class="mono"
                      style={{ flex: "1", "min-width": "220px", background: "var(--bg)", color: "var(--fg)", border: "1px solid var(--line)", "border-radius": "3px", padding: "4px 6px" }}
                      value={newName()}
                      onInput={(e) => setNewName(e.currentTarget.value)}
                    />
                    <button class="btn" disabled={busy() || !newName().trim() || !!preview()?.issues.length} onClick={saveNew}>
                      Salva come nuovo profilo
                    </button>
                  </div>
                </div>
              </>
            )}
          </Show>
        </div>
      </div>

      <Show when={ask()}>
        {(a) => (
          <Switch>
            <Match when={a().kind === "duplica"}>
              <AskName
                title={`Duplica «${a().name}»`}
                label="nome nuovo"
                value={`${a().name}-2`}
                confirmLabel="Duplica"
                note="Il nome del profilo è anche l'alias servito: due profili non possono chiamarsi uguale."
                onCancel={() => setAsk(null)}
                onConfirm={duplicate}
              />
            </Match>
            <Match when={a().kind === "rinomina"}>
              <AskName
                title={`Rinomina «${a().name}»`}
                label="nome nuovo"
                value={a().name}
                confirmLabel="Rinomina"
                note="Cambia il file e l'alias servito. Gli avvii già registrati continuano a citare il nome vecchio: raccontano com'è andata, non puntano a un file."
                onCancel={() => setAsk(null)}
                onConfirm={rename}
              />
            </Match>
            <Match when={a().kind === "elimina"}>
              <Confirm
                title={`Elimina «${a().name}»`}
                lines={(a() as { lines: string[] }).lines}
                confirmLabel="Elimina il profilo"
                danger
                onCancel={() => setAsk(null)}
                onConfirm={remove}
              />
            </Match>
          </Switch>
        )}
      </Show>
    </section>
  );
}
