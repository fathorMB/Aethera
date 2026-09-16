import { open } from "@tauri-apps/plugin-dialog";
import { createEffect, createMemo, createSignal, For, Match, on, onCleanup, onMount, Show, Switch } from "solid-js";
import * as api from "../api";
import type { EngineStatus, Issue, ModelRow, Overview, Preview, Profile, ProfileEntry, ResolvedBuild } from "../api";
import { AskName, CommandLine, Confirm, copy, Dialog, Empty, Unknown, Val } from "../components";
import { fixed, getPath, num, setPath, show } from "../format";
import { Alerts, type AlertItem, Field, Head, Seg, Stack } from "../ui";
import { splitBuildId } from "../ui-logic";
import {
  ADVANCED,
  baseText,
  ESSENTIAL,
  gpuMode,
  groupSummary,
  type Input,
  type Lever,
  leverPaths,
  M08,
  modifiedPaths,
  parse,
  toText,
} from "./leve";

const STATE_TEXT: Record<api.ModelState, string> = {
  verified: "verificato",
  present: "presente, da verificare",
  mismatch: "hash diverso",
  downloading: "in download",
  downloadable: "scaricabile",
  missing: "mancante",
};

/** Un controllo di una leva: casella, tendina o on/off. */
function Control(props: { input: Input; label: string; edited: Profile; onRaw: (input: Input, raw: string) => void }) {
  const value = () => getPath(props.edited, props.input.path);
  const optional = () => props.input.kind === "optselect" || props.input.kind === "optbool";
  const options = () => {
    const base = props.input.kind === "optbool" || props.input.kind === "bool" ? ["on", "off"] : (props.input.options ?? []);
    const cur = toText(value());
    // Un valore che la lista non conosce resta visibile: meglio vederlo che perderlo in silenzio.
    return cur !== "" && !base.includes(cur) ? [...base, cur] : base;
  };
  const style = () => (props.input.width ? { width: props.input.width, flex: "none" } : { flex: "1" });
  const aria = () => `${props.label} · ${props.input.path}`;
  return (
    <Show
      when={props.input.kind === "select" || props.input.kind === "bool" || optional()}
      fallback={
        <input
          style={style()}
          aria-label={aria()}
          value={toText(value())}
          placeholder={props.input.placeholder ?? (props.input.kind.startsWith("opt") ? "non impostato" : "")}
          onInput={(e) => props.onRaw(props.input, e.currentTarget.value)}
          spellcheck={false}
        />
      }
    >
      <select
        style={style()}
        aria-label={aria()}
        value={props.input.kind === "bool" ? (value() ? "on" : "off") : toText(value())}
        onChange={(e) => props.onRaw(props.input, e.currentTarget.value)}
      >
        <Show when={optional()}>
          <option value="">{props.input.placeholder ?? "non impostato"}</option>
        </Show>
        <For each={options()}>{(o) => <option value={o}>{o}</option>}</For>
      </select>
    </Show>
  );
}

/** Una riga del modulo: etichetta, controlli, valore di prima, suggerimento, verdetto, errori. */
function LeverRow(props: {
  lever: Lever;
  base: Profile | null;
  edited: Profile;
  issues: Issue[];
  builds: ResolvedBuild[];
  modelNote?: string | null;
  onSet: (path: string, value: unknown) => void;
}) {
  const [bad, setBad] = createSignal<Record<string, boolean>>({});
  const [forceNum, setForceNum] = createSignal(false);
  const mod = () => modifiedPaths(props.lever, props.base, props.edited).length > 0;
  const issues = () => props.issues.filter((i) => leverPaths(props.lever).includes(i.field));
  const anyBad = () => Object.values(bad()).some(Boolean);
  const onRaw = (input: Input, raw: string) => {
    const r = parse(input.kind, raw);
    setBad({ ...bad(), [input.path]: !r.ok });
    if (r.ok) props.onSet(input.path, r.value);
  };

  // Build: le coppie build · backend che la macchina conosce, più quella del profilo se manca.
  const buildOptions = () => {
    const seen = new Set<string>();
    const out: { key: string; label: string }[] = [];
    for (const b of props.builds) {
      const { build, backend } = splitBuildId(b.id);
      if (!build || !backend) continue;
      const key = `${build}|${backend}`;
      if (!seen.has(key)) {
        seen.add(key);
        out.push({ key, label: `${build} · ${backend}` });
      }
    }
    if (!out.length) return out;
    const cur = `${props.edited.runtime.build}|${props.edited.runtime.backend}`;
    if (!seen.has(cur)) out.push({ key: cur, label: `${props.edited.runtime.build} · ${props.edited.runtime.backend} (non trovata)` });
    return out;
  };
  const gpu = () => (forceNum() ? "num" : gpuMode(props.edited.server.n_gpu_layers));

  return (
    <>
      <Field
        label={props.lever.label}
        lever={props.lever.lever}
        mod={mod()}
        bad={anyBad() || issues().length > 0}
        hint={
          <>
            <Show when={mod()}>
              <span class="base">{baseText(props.lever, props.base, props.edited)}</span>
            </Show>
            <Show when={props.lever.cache}>
              <span class="pill cache">cache</span>
            </Show>
            <Show when={props.modelNote}>
              <span>{props.modelNote}</span>
            </Show>
            <Show when={props.lever.hint}>
              <span>{props.lever.hint}</span>
            </Show>
            <Show when={props.lever.verdict}>
              <span class="pill x" title={`${props.lever.verdictTitle ?? ""} — ${M08}`}>
                {props.lever.verdict}
              </span>
            </Show>
          </>
        }
      >
        <Switch
          fallback={
            <For each={props.lever.inputs}>
              {(inp) => <Control input={inp} label={props.lever.label} edited={props.edited} onRaw={onRaw} />}
            </For>
          }
        >
          <Match when={props.lever.special === "build" && buildOptions().length > 0}>
            <select
              aria-label={props.lever.label}
              value={`${props.edited.runtime.build}|${props.edited.runtime.backend}`}
              onChange={(e) => {
                const [build, backend] = e.currentTarget.value.split("|");
                props.onSet("runtime.build", build);
                props.onSet("runtime.backend", backend);
              }}
            >
              <For each={buildOptions()}>{(o) => <option value={o.key}>{o.label}</option>}</For>
            </select>
          </Match>
          <Match when={props.lever.special === "gpu"}>
            <Seg
              label={props.lever.label}
              value={gpu()}
              options={[
                { id: "all", label: "tutti (999)" },
                { id: "num", label: "numero" },
                { id: "fit", label: "decide fit" },
              ]}
              onChange={(m) => {
                setForceNum(m === "num");
                if (m === "all") props.onSet("server.n_gpu_layers", 999);
                if (m === "fit") props.onSet("server.n_gpu_layers", null);
              }}
            />
            <Show when={gpu() === "num"}>
              <Control input={props.lever.inputs[0]} label={props.lever.label} edited={props.edited} onRaw={onRaw} />
            </Show>
          </Match>
        </Switch>
      </Field>
      <For each={issues()}>{(i) => <div class="f-err">{i.message}</div>}</For>
    </>
  );
}

/** Il dialogo aperto: uno alla volta, e nessuno usa `window.prompt`, che nella WebView non c'è. */
type Ask =
  | { kind: "duplica"; name: string }
  | { kind: "rinomina"; name: string }
  | { kind: "salva"; name: string }
  | { kind: "importa" }
  | { kind: "elimina"; name: string; lines: string[] };

/** Chiede la build da fissare nei profili importati: i JSON di minis-config non la dicono. */
function ImportDialog(props: { value: string; onCancel: () => void; onConfirm: (build: string) => void }) {
  const [build, setBuild] = createSignal(props.value);
  return (
    <Dialog
      title="Importa da minis-config"
      actions={
        <>
          <button class="btn primary" disabled={!build().trim()} onClick={() => props.onConfirm(build().trim())}>
            Scegli i JSON…
          </button>
          <button class="btn" onClick={props.onCancel}>
            Annulla
          </button>
        </>
      }
    >
      <div class="field" style={{ "grid-template-columns": "110px 1fr" }}>
        <label>build</label>
        <input class="mono" autofocus value={build()} onInput={(e) => setBuild(e.currentTarget.value)} />
      </div>
      <div class="cond" style={{ "margin-top": "6px" }}>
        I JSON non dicono la build: si fissa questa.
      </div>
    </Dialog>
  );
}

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
  const [importBuild, setImportBuild] = createSignal(props.overview.builds[0]?.id.split("-")[0] ?? "b10809");
  const [busy, setBusy] = createSignal(false);
  const [ask, setAsk] = createSignal<Ask | null>(null);
  const [search, setSearch] = createSignal("");
  const [catalog, setCatalog] = createSignal<ModelRow[]>([]);

  const entry = createMemo(() => entries().find((e) => e.name === selected()) ?? null);
  const base = () => entry()?.profile ?? null;
  const engineOn = () => api.isEngineOn(props.status);

  const select = (name: string, list = entries()) => {
    setSelected(name);
    const e = list.find((x) => x.name === name);
    setEdited(e?.profile ? structuredClone(e.profile) : null);
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
    // Il catalogo dice se i pesi del profilo sono verificati: se non si legge, resta «sconosciuto».
    api.catalogList().then((v) => setCatalog(v.rows), () => setCatalog([]));
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

  const saveNew = (name: string) =>
    act(async () => {
      const saved = await api.saveProfile({ ...edited()!, name }, null);
      setAsk(null);
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

  const importJson = (build: string) =>
    act(async () => {
      setAsk(null);
      setImportBuild(build);
      const picked = await open({ multiple: true, filters: [{ name: "Profili minis-config", extensions: ["json"] }] });
      if (!picked) return;
      const paths = Array.isArray(picked) ? picked : [picked];
      const lines: string[] = [];
      let last: string | undefined;
      let failed = false;
      for (const path of paths) {
        try {
          const r = await api.importMinis(path, build);
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
  const overrides = () => preview()?.overrides.length ?? 0;
  const canStart = () => !busy() && !engineOn() && !creating() && preview() != null && preview()!.blockers.length === 0;
  const nameIssues = () => (preview()?.issues ?? []).filter((i) => i.field === "name" || i.field === "schema_version");

  const visible = () => {
    const q = search().trim().toLowerCase();
    if (!q) return entries();
    return entries().filter((e) => e.name.toLowerCase().includes(q) || (e.profile?.model.file.toLowerCase().includes(q) ?? false));
  };

  const modelRow = () => {
    const f = edited()?.model.file.toLowerCase();
    return f ? (catalog().find((r) => r.file.toLowerCase() === f) ?? null) : null;
  };
  const modelNote = () => {
    const p = edited();
    if (!p) return null;
    const bits = [p.model.quant, p.model.size_gb != null ? `${fixed(p.model.size_gb, 1)} GB` : null];
    const row = modelRow();
    bits.push(row ? STATE_TEXT[row.state] : "fuori dal catalogo");
    return bits.filter(Boolean).join(" · ");
  };

  const vgm = () => props.overview.system.gpus.find((g) => g.dedicated_gib)?.dedicated_gib ?? null;

  const topAlerts = (): AlertItem[] => {
    const pv = preview();
    if (!pv) return [];
    const out: AlertItem[] = pv.warnings.map((w, i) => ({ key: `w${i}`, tone: "err", title: "Attenzione", detail: `${w} Si può avviare lo stesso.` }));
    if (pv.proposals.length) {
      out.push({
        key: "prop",
        tone: "acc",
        title: `Le misure di M-08 suggeriscono ${pv.proposals.length === 1 ? "una modifica" : `${pv.proposals.length} modifiche`}`,
        detail: (
          <>
            <For each={pv.proposals}>
              {(x) => (
                <div>
                  <span class="mono">{x.field}</span> <s>{show(x.current)}</s> → <b>{show(x.value)}</b> · {x.reason}
                </div>
              )}
            </For>
            <div>Il file non si tocca: si applicano sopra, le vedi nella riga di comando e le salvi tu.</div>
          </>
        ),
        actions: (
          <button class="btn sm" onClick={() => pv.proposals.forEach((x) => set(x.field, x.value))}>
            Applica
          </button>
        ),
      });
    }
    return out;
  };

  return (
    <section>
      <Head
        title="Avvio"
        sub="Un profilo dice come si accende il motore. Ogni leva che cambi resta in sovrapposizione (●) e aggiorna la riga di comando: puoi avviare così (il manifest registra profilo + differenze) o salvare come profilo nuovo."
      >
        <button class="btn sm primary" disabled={busy()} onClick={() => newProfile(null)}>
          Nuovo profilo…
        </button>
        <button class="btn sm" disabled={busy()} onClick={() => setAsk({ kind: "importa" })}>
          Importa JSON da minis-config…
        </button>
      </Head>

      <Show when={message()}>
        {(m) => (
          <pre class={`note ${m().kind === "err" ? "err" : ""} mb`} style={{ "white-space": "pre-wrap", margin: "0 0 12px" }}>
            {m().text}
          </pre>
        )}
      </Show>

      <div class="split">
        <div>
          <div class="row mb">
            <input
              class="txt mono"
              style={{ width: "100%" }}
              placeholder="cerca fra i profili"
              aria-label="cerca fra i profili"
              value={search()}
              onInput={(e) => setSearch(e.currentTarget.value)}
            />
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
              each={visible()}
              fallback={
                <Show when={!creating()}>
                  <div class="cond">
                    <Show
                      when={entries().length}
                      fallback="Nessun profilo ancora. «Nuovo profilo…» ne scrive uno con valori sensati sul modello che scegli; se vieni da minis-config, «Importa JSON…» li converte."
                    >
                      Nessun profilo con «{search()}».
                    </Show>
                  </div>
                </Show>
              }
            >
              {(e) => (
                <div
                  class="it"
                  classList={{ on: e.name === selected() }}
                  role="button"
                  tabindex={0}
                  onClick={() => select(e.name)}
                  onKeyDown={(k) => {
                    if (k.key === "Enter" || k.key === " ") {
                      k.preventDefault();
                      select(e.name);
                    }
                  }}
                >
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
                    <Show when={e.name === selected() && overrides() > 0}>
                      <span class="pill mod">
                        {overrides()} {overrides() === 1 ? "modifica" : "modifiche"}
                      </span>
                    </Show>
                    <Show when={e.name === selected() && (preview()?.proposals.length ?? 0) > 0}>
                      <span class="pill acc">
                        {preview()!.proposals.length} {preview()!.proposals.length === 1 ? "proposta" : "proposte"}
                      </span>
                    </Show>
                    <Show when={engineOn() && api.runOf(props.status)?.profile === e.name}>
                      <span class="badge ok tight">
                        <i />
                        acceso
                      </span>
                    </Show>
                  </div>
                </div>
              )}
            </For>
          </div>
          <div class="row" style={{ "margin-top": "8px" }}>
            <button class="btn sm" disabled={busy() || !selected()} onClick={() => setAsk({ kind: "duplica", name: selected() })}>
              Duplica…
            </button>
            <button class="btn sm" disabled={busy() || !selected()} onClick={() => setAsk({ kind: "rinomina", name: selected() })}>
              Rinomina…
            </button>
            <button class="btn sm danger" disabled={busy() || !selected()} onClick={askDelete}>
              Elimina…
            </button>
            <button class="btn sm" disabled={busy()} onClick={() => reload(selected())}>
              Rileggi
            </button>
          </div>
          <div class="cond mono" style={{ "margin-top": "8px", "word-break": "break-all" }}>
            {props.overview.data_root}\profiles
          </div>
        </div>

        <div>
          <Show when={!edited() && !entries().length}>
            <Empty title="Un profilo dice come si accende il motore.">
              <div>
                Modello, contesto, tipi di cache, speculazione, porta: tutto esplicito, niente lasciato al default del motore,
                così la riga di comando racconta per intero com'è stato avviato. Il nome del profilo è anche l'alias che i
                client chiedono.
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
            {(p) => {
              const ess = () => groupSummary(ESSENTIAL, base(), p());
              const adv = () => groupSummary(ADVANCED, base(), p());
              const advIssues = () => (preview()?.issues ?? []).some((i) => ADVANCED.some((l) => leverPaths(l).includes(i.field)));
              return (
                <>
                  <div class="actbar">
                    <span class="name">
                      {p().name}
                      <small>
                        alias servito · {entry()?.file ?? `${props.overview.data_root}\\profiles\\${p().name}.toml · da salvare`}
                      </small>
                    </span>
                    <Show when={overrides() > 0}>
                      <span class="pill mod">
                        {overrides()} {overrides() === 1 ? "modifica" : "modifiche"}
                      </span>
                    </Show>
                    <Show when={preview()?.invalidates_cache}>
                      <span
                        class="pill cache"
                        title="Hai cambiato una leva marcata cache: il manifest dichiarerà l'avvio come «invalida la cache»."
                      >
                        invalida la cache
                      </span>
                    </Show>
                    <span class="right" />
                    <button class="btn primary" disabled={!canStart()} onClick={start}>
                      {overrides() ? "Avvia con le modifiche" : "Avvia"}
                    </button>
                    <button class="btn" disabled={!overrides() || busy()} onClick={() => select(selected())}>
                      Scarta
                    </button>
                    <button
                      class="btn"
                      disabled={!overrides() || busy() || p().name !== selected() || !!preview()?.issues.length}
                      onClick={update}
                    >
                      Aggiorna il profilo
                    </button>
                    <button
                      class="btn"
                      disabled={busy() || !!preview()?.issues.filter((i) => i.field !== "name").length}
                      onClick={() =>
                        setAsk({ kind: "salva", name: creating() ? p().name : `${p().name}.variante` })
                      }
                    >
                      Salva come…
                    </button>
                    <button class="btn sm" disabled={!preview()} onClick={() => copy(preview()?.line ?? "")}>
                      Copia riga
                    </button>
                    <Show when={preview()?.blockers.length}>
                      <div class="full note err">
                        <For each={preview()!.blockers}>{(b) => <div>{b}</div>}</For>
                      </div>
                    </Show>
                    <For each={nameIssues()}>{(i) => <div class="full cond" style={{ color: "var(--err)" }}>{i.message}</div>}</For>
                    <Show when={creating()}>
                      <div class="full cond">
                        Profilo non ancora salvato: un avvio cita il profilo da cui è partito, quindi prima si salva.
                      </div>
                    </Show>
                    <Show when={engineOn()}>
                      <div class="full cond">
                        {api.inUse(props.status) ? (
                          <>
                            Il motore acceso è <b>IN USO</b>: l'avvio parte solo quando torna libero e viene fermato.
                          </>
                        ) : (
                          "Un motore è già acceso: nella v1 se ne avvia uno alla volta."
                        )}
                      </div>
                    </Show>
                    <Show when={props.status?.state === "orphan"}>
                      <div class="full cond">Un llama-server orfano occupa una porta dei profili: vedi la pagina Motore.</div>
                    </Show>
                    <Show when={p().notes}>
                      <div class="full cond">{p().notes}</div>
                    </Show>
                  </div>

                  <Alerts items={topAlerts()} />

                  <div class="grid g2 av2 mb" style={{ "align-items": "start" }}>
                    <div>
                      <div class="card">
                        <h2>
                          Essenziali <span class="r">{ess().modified ? `${ess().modified} modificate` : ""}</span>
                        </h2>
                        <For each={ESSENTIAL}>
                          {(l) => (
                            <LeverRow
                              lever={l}
                              base={base()}
                              edited={p()}
                              issues={preview()?.issues ?? []}
                              builds={props.overview.builds}
                              modelNote={l.id === "model" ? modelNote() : null}
                              onSet={set}
                            />
                          )}
                        </For>
                      </div>

                      <details style={{ "margin-top": "12px" }} open={advIssues() || undefined}>
                        <summary>
                          Avanzate
                          <span class="r">
                            {adv().levers} leve ({adv().fields} campi) · {adv().modified} {adv().modified === 1 ? "modificata" : "modificate"} ·{" "}
                            {adv().verdicts} con un verdetto di M-08
                          </span>
                        </summary>
                        <div class="body">
                          <For each={ADVANCED}>
                            {(l) => (
                              <LeverRow
                                lever={l}
                                base={base()}
                                edited={p()}
                                issues={preview()?.issues ?? []}
                                builds={props.overview.builds}
                                onSet={set}
                              />
                            )}
                          </For>
                        </div>
                      </details>
                    </div>

                    <div class="grid" style={{ "align-items": "start" }}>
                      <div class="card">
                        <h2>
                          Riga di comando{" "}
                          <span class="r">
                            aggiornata in tempo reale · {overrides()} {overrides() === 1 ? "differenza" : "differenze"} dal profilo
                          </span>
                        </h2>
                        <Show when={preview()} fallback={<div class="cond">…</div>}>
                          {(pv) => <CommandLine binary={binary()} args={pv().args} />}
                        </Show>
                        <Show when={preview()?.invalidates_cache}>
                          <div class="note warn" style={{ "margin-top": "8px" }}>
                            Hai cambiato una leva marcata <span class="pill cache">cache</span>: il manifest dichiarerà l'avvio
                            come «invalida la cache».
                          </div>
                        </Show>
                      </div>

                      <div class="card">
                        <h2>
                          Stima di memoria <span class="r">prima dell'avvio · sostituita dalla misura</span>
                        </h2>
                        <Show when={preview()?.estimate} fallback={<div class="cond">Scegli un profilo per vedere la stima.</div>}>
                          {(e) => {
                            const gib = 1024 ** 3;
                            const toGib = (b: number | null) => (b == null ? null : b / gib);
                            const gb = (b: number | null) => (b == null ? null : fixed(b / 1e9, 2));
                            const quota = () => {
                              const t = e().total_bytes;
                              const v = vgm();
                              return t != null && v ? (t / (v * gib)) * 100 : null;
                            };
                            const tone = () => {
                              const q = quota();
                              return q == null ? "" : q > 95 ? "var(--err)" : q > 85 ? "var(--warn)" : "var(--ok)";
                            };
                            const ctx = () => num(p().server.ctx);
                            const other = () =>
                              e().state_bytes == null && e().compute_bytes == null
                                ? null
                                : (e().state_bytes ?? 0) + (e().compute_bytes ?? 0);
                            return (
                              <>
                                <Stack
                                  legend={false}
                                  total={vgm()}
                                  parts={[
                                    { key: "w", label: "pesi", value: toGib(e().weights_bytes), cls: "ded" },
                                    { key: "kv", label: "cache KV", value: toGib(e().kv_bytes), cls: "kv2" },
                                    { key: "o", label: "stato + buffer", value: toGib(other()), cls: "oth" },
                                  ]}
                                />
                                <dl class="kv" style={{ "margin-top": "8px" }}>
                                  <dt>
                                    <i class="sw" style={{ background: "var(--acc)" }} /> Pesi
                                  </dt>
                                  <dd>
                                    <Val v={gb(e().weights_bytes)} unit="GB" />
                                  </dd>
                                  <dt>
                                    <i class="sw" style={{ background: "var(--busy)" }} /> Cache KV <span class="cond">@{ctx()}</span>
                                  </dt>
                                  <dd>
                                    <Val v={gb(e().kv_bytes)} unit="GB" />
                                  </dd>
                                  <Show when={e().state_bytes}>
                                    <dt>
                                      <i class="sw" style={{ background: "var(--fg3)" }} /> Stato ricorrente
                                    </dt>
                                    <dd class="num">{gb(e().state_bytes)} GB</dd>
                                  </Show>
                                  <dt>
                                    <i class="sw" style={{ background: "var(--fg3)" }} /> Buffer di calcolo
                                  </dt>
                                  <dd>
                                    <Val v={gb(e().compute_bytes)} unit="GB" />
                                    <Show when={e().compute_from}>
                                      <span class="cond"> misurato su {e().compute_from}</span>
                                    </Show>
                                  </dd>
                                  <dt>{e().total_is_lower_bound ? "Totale minimo" : "Totale stimato"}</dt>
                                  <dd class="num">
                                    <b style={{ color: tone() }}>
                                      {e().total_is_lower_bound ? "≥ " : "~ "}
                                      {gb(e().total_bytes)} GB
                                    </b>
                                    <span class="cond">
                                      {" "}
                                      / <Show when={vgm()} fallback={<Unknown />}>{fixed(vgm()!, 0)}</Show> GiB dedicati
                                    </span>
                                  </dd>
                                </dl>
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
                        <h2>
                          Campionamento consigliato <span class="r">dato del modello · non è un default del server</span>
                          <button class="btn sm" disabled={busy() || !p().model.file} onClick={takeSampling}>
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
                          <div style={{ "overflow-x": "auto" }}>
                            <table class="compact">
                              <thead>
                                <tr>
                                  <th>modalità</th>
                                  <th class="r">temp</th>
                                  <th class="r">top_p</th>
                                  <th class="r">top_k</th>
                                  <th class="r">min_p</th>
                                  <th class="r">presence</th>
                                </tr>
                              </thead>
                              <tbody>
                                <For each={Object.entries(p().sampling_by_mode!)}>
                                  {([mode, sm]) => (
                                    <tr>
                                      <td>
                                        {mode}
                                        <div class="mini">
                                          {show(sm.source)}
                                          {sm.verified ? ` · ${sm.verified}` : ""}
                                        </div>
                                      </td>
                                      <td class="r num">{show(sm.temperature)}</td>
                                      <td class="r num">{show(sm.top_p)}</td>
                                      <td class="r num">{show(sm.top_k)}</td>
                                      <td class="r num">{show(sm.min_p)}</td>
                                      <td class="r num">{show(sm.presence_penalty)}</td>
                                    </tr>
                                  )}
                                </For>
                              </tbody>
                            </table>
                          </div>
                          <div class="cond" style={{ "margin-top": "6px" }}>
                            Esce anche nelle righe per i client (Impostazioni), con fonte e data: il campionamento lo manda il
                            client a ogni richiesta, llama-server non lo applica da solo.
                          </div>
                        </Show>
                      </div>
                    </div>
                  </div>
                </>
              );
            }}
          </Show>
        </div>
      </div>

      <Show when={ask()}>
        {(a) => (
          <Switch>
            <Match when={a().kind === "duplica"}>
              <AskName
                title={`Duplica «${(a() as { name: string }).name}»`}
                label="nome nuovo"
                value={`${(a() as { name: string }).name}-2`}
                confirmLabel="Duplica"
                note="Il nome del profilo è anche l'alias servito: due profili non possono chiamarsi uguale."
                onCancel={() => setAsk(null)}
                onConfirm={duplicate}
              />
            </Match>
            <Match when={a().kind === "rinomina"}>
              <AskName
                title={`Rinomina «${(a() as { name: string }).name}»`}
                label="nome nuovo"
                value={(a() as { name: string }).name}
                confirmLabel="Rinomina"
                note="Cambia il file e l'alias servito. Gli avvii già registrati continuano a citare il nome vecchio: raccontano com'è andata, non puntano a un file."
                onCancel={() => setAsk(null)}
                onConfirm={rename}
              />
            </Match>
            <Match when={a().kind === "salva"}>
              <AskName
                title="Salva come profilo nuovo"
                label="nome"
                value={(a() as { name: string }).name}
                allowSame
                confirmLabel="Salva"
                note="Il nome del profilo è anche l'alias servito. Il profilo di partenza non si tocca."
                onCancel={() => setAsk(null)}
                onConfirm={saveNew}
              />
            </Match>
            <Match when={a().kind === "importa"}>
              <ImportDialog value={importBuild()} onCancel={() => setAsk(null)} onConfirm={importJson} />
            </Match>
            <Match when={a().kind === "elimina"}>
              <Confirm
                title={`Elimina «${(a() as { name: string }).name}»`}
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
