import { open } from "@tauri-apps/plugin-dialog";
import { createEffect, createSignal, For, on, onMount, Show } from "solid-js";
import * as api from "../api";
import type {
  BuildProvenance,
  ClientSnippets,
  EngineStatus,
  ExitBehavior,
  MachineConfig,
  MachineText,
  Overview,
  ProfileEntry,
} from "../api";
import type { SettingsTab } from "../App";
import { Confirm, copy, Empty, Val } from "../components";
import { clock, duration, fixed, num, pct } from "../format";
import { Field, Head, Help, Q, Seg } from "../ui";
import { provenanceView, samePath, seconds } from "../ui-logic";

type ClientTab = "nonio" | "opencode" | "claude" | "galaxy" | "env";
const CLIENT_TABS: { id: ClientTab; label: string }[] = [
  { id: "nonio", label: "Nonio" },
  { id: "opencode", label: "OpenCode" },
  { id: "claude", label: "Claude Code" },
  { id: "galaxy", label: "GalaxyCenter" },
  { id: "env", label: "env per banchi" },
];

const TABS: { id: SettingsTab; label: string }[] = [
  { id: "macchina", label: "Macchina" },
  { id: "client", label: "Client" },
  { id: "app", label: "App" },
];

/**
 * Da dove viene una build (M-14): il file di provenienza quando c'è, «build scaricata da ggml-org»
 * o «provenienza assente» quando non c'è. `entry` assente = cartella non letta: sconosciuta.
 */
function ProvenanceLine(props: { id: string; entry: BuildProvenance | undefined }) {
  const view = () => provenanceView(props.id, props.entry);
  const p = () => props.entry?.provenance ?? null;
  const short = (commit: string) => commit.slice(0, 12);
  return (
    <div style={{ "margin-top": "4px", "font-family": "var(--sans)" }}>
      <span class={`badge tight ${view().tone}`} title={view().help}>
        <i />
        {view().label}
      </span>
      <Show when={p()}>
        {(prov) => (
          <>
            {" "}
            <span class="cond">
              compilata il <Val v={prov().data ? clock(prov().data!) : null} /> in{" "}
              <Val v={prov().durata_build_s == null ? null : duration(prov().durata_build_s!)} />
            </span>
            <Help summary="tag, rami e commit" style={{ "margin-top": "4px" }}>
              <dl class="kv">
                <dt>Id</dt>
                <dd class="mono">{prov().id}</dd>
                <dt>Tag base</dt>
                <dd class="mono">
                  {prov().base} @ {short(prov().commit_base)}
                </dd>
                <dt>Serie</dt>
                <dd class="mono">
                  moro{prov().serie} @ {short(prov().commit)}{" "}
                  <Show when={!prov().patch?.length}>
                    <span class="cond" style={{ "font-family": "var(--sans)" }}>
                      tag liscio compilato qui, senza patch
                    </span>
                  </Show>
                </dd>
                <For each={prov().patch ?? []}>
                  {(b) => (
                    <>
                      <dt>Ramo</dt>
                      <dd>
                        <span class="mono">
                          {b.ramo} @ {short(b.commit)}
                        </span>
                        <For each={b.commit_della_patch ?? []}>{(c) => <div class="mono mini">{c}</div>}</For>
                      </dd>
                    </>
                  )}
                </For>
                <dt>Compilatore</dt>
                <dd>
                  <Val v={prov().compilatore} mono={false} />
                </dd>
              </dl>
            </Help>
          </>
        )}
      </Show>
    </div>
  );
}

/** Quanto spazio resta a ogni client prima di compattare, sull'avvio acceso. */
function BudgetCard(props: { sn: ClientSnippets; ctx: number | null; status: EngineStatus | null }) {
  const lastCompaction = () => {
    const s = props.status;
    if (s?.state !== "ready") return null;
    const c = s.telemetry.compactions;
    return c[c.length - 1] ?? null;
  };
  return (
    <div class="card">
      <h2>
        Budget di contesto{" "}
        <Q title="Il prompt fisso (istruzioni e strumenti del client) è misurato, mai stimato: passa il mouse sul numero per la fonte. Spazio di lavoro = contesto servito − prompt fisso − output riservato; rosso sotto il 30 % del contesto. Più spazio vuol dire compattazioni più rare, e ognuna costa decine di secondi di prefill." />
        <span class="r">per client, sull'avvio acceso{props.ctx != null ? ` · ${num(props.ctx)} token serviti` : ""}</span>
      </h2>
      <div style={{ "overflow-x": "auto" }}>
        <table style={{ "font-size": "12px" }}>
          <thead>
            <tr>
              <th>client</th>
              <th class="r">prompt fisso</th>
              <th class="r">output riservato</th>
              <th class="r">spazio di lavoro</th>
              <th style={{ width: "90px" }} />
              <th>lettura</th>
            </tr>
          </thead>
          <tbody>
            <For each={props.sn.budgets}>
              {(b) => (
                <tr>
                  <td>{b.client}</td>
                  <td class="r" title={b.fixed_source ?? ""}>
                    <Show when={b.fixed_prompt != null} fallback={<span class="unk">non misurato</span>}>
                      <span class="num">{num(b.fixed_prompt)}</span>
                    </Show>
                  </td>
                  <td class="r">
                    <Val v={num(b.reserved_output)} />
                  </td>
                  <td class="r">
                    <Val v={num(b.workspace)} />
                  </td>
                  <td>
                    <Show when={b.share != null}>
                      <div class={`bar ${b.tight ? "err" : "ok"}`} title={`${pct(b.share)} % del contesto`}>
                        <span style={{ width: `${Math.max(b.share! * 100, 1)}%` }} />
                      </div>
                    </Show>
                  </td>
                  <td>
                    <Show
                      when={b.tight != null}
                      fallback={
                        <span class="cond">
                          {b.fixed_prompt == null
                            ? "si misura dalla sua prima richiesta a freddo, se prende il lock"
                            : "manca l'output riservato: client.reserved_output_tokens nel profilo"}
                        </span>
                      }
                    >
                      <span class={`tag ${b.tight ? "reb" : "ext"}`}>{b.tight ? "compatta presto" : "largo"}</span>{" "}
                      <Show when={b.share != null}>
                        <span class="cond">{pct(b.share)} %</span>
                      </Show>
                      <Show when={b.tight && b.ctx_needed != null}>
                        <div class="cond">con {num(b.ctx_needed)} token serviti lo spazio arriva al 30 %</div>
                      </Show>
                    </Show>
                  </td>
                </tr>
              )}
            </For>
          </tbody>
        </table>
      </div>
      <Show when={lastCompaction()}>
        {(c) => (
          <div class="note warn" style={{ "margin-top": "8px" }}>
            La compattazione delle {clock(c().at).slice(6, 11)} è costata {seconds(c().cost_ms)}. La soglia del 30 % dice se
            il client parte largo, non quante compattazioni farà un compito lungo.
          </div>
        )}
      </Show>
    </div>
  );
}

export default function Impostazioni(props: {
  overview: Overview;
  status: EngineStatus | null;
  onChange: (o: Overview) => void;
  tab: SettingsTab;
  onTab: (t: SettingsTab) => void;
  theme: "dark" | "light";
  onTheme: (t: "dark" | "light") => void;
}) {
  const [machine, setMachine] = createSignal<MachineConfig | null>(null);
  const [message, setMessage] = createSignal<{ kind: "ok" | "err"; text: string } | null>(null);
  const [snippets, setSnippets] = createSignal<ClientSnippets | null>(null);
  /** Il testo di machine.toml: serve solo quando non si lascia leggere, perché senza vederlo non si corregge. */
  const [raw, setRaw] = createSignal<MachineText | null>(null);
  const [askReset, setAskReset] = createSignal(false);
  const [clientTab, setClientTab] = createSignal<ClientTab>("nonio");
  const [profiles, setProfiles] = createSignal<ProfileEntry[]>([]);
  const ctxServed = () => {
    const s = props.status;
    return s?.state === "ready" ? s.ctx_served : null;
  };
  const cond = () => props.overview.conditions;

  createEffect(() => setMachine(props.overview.machine ? structuredClone(props.overview.machine) : null));
  // Quando machine.toml non è valido l'app non ha niente da mostrare nei campi: si mostra il file.
  createEffect(
    on(
      () => props.overview.machine_error,
      async (e) => setRaw(e ? await api.machineText().catch(() => null) : null),
    ),
  );

  // Le righe cambiano con l'avvio e con il contesto servito, non a ogni secondo.
  const runKey = () => {
    const s = props.status;
    if (s?.state === "ready") return `${s.run.run_id}:${s.ctx_served}`;
    if (s?.state === "loading") return s.run.run_id;
    return "";
  };
  createEffect(
    on(runKey, async (key) => {
      setSnippets(key ? await api.clientSnippets().catch(() => null) : null);
    }),
  );

  const patch = (p: Partial<MachineConfig>) => setMachine({ ...machine()!, ...p });
  const builds = () => machine()?.build ?? [];
  /** La provenienza letta dal backend per una cartella; undefined se la cartella non è fra le build trovate. */
  const provenanceOf = (dir: string) => (props.overview.builds_provenance ?? []).find((x) => samePath(x.dir, dir));
  /** machine.toml ha modifiche non salvate: il pulsante nella testa lo dice. */
  const dirty = () => JSON.stringify(machine()) !== JSON.stringify(props.overview.machine);

  const run = async (fn: () => Promise<void>) => {
    setMessage(null);
    try {
      await fn();
    } catch (e) {
      setMessage({ kind: "err", text: String(e) });
    }
  };

  const pickDir = async () => {
    const picked = await open({ directory: true });
    return typeof picked === "string" ? picked : null;
  };

  const changeRoot = () =>
    run(async () => {
      const dir = await pickDir();
      if (dir) props.onChange(await api.setDataRoot(dir));
    });

  const addBuild = () =>
    run(async () => {
      const dir = await pickDir();
      if (!dir) return;
      const folder = dir.split(/[\\/]/).pop() ?? dir;
      patch({ build: [...builds(), { id: folder.replace(/^llama-/, ""), path: dir }] });
    });

  const save = () =>
    run(async () => {
      props.onChange(await api.saveMachine(machine()!));
      setMessage({ kind: "ok", text: "machine.toml salvato." });
    });

  const exitBehavior = (b: ExitBehavior) => run(() => api.setExitBehavior(b));
  const setDefaultProfile = (name: string) =>
    run(async () => {
      props.onChange(await api.setDefaultProfile(name || null));
    });

  onMount(() => {
    // Per la select del profilo principale: se la lista non si legge la select mostra solo «nessuno».
    api.listProfiles().then(setProfiles, () => setProfiles([]));
  });

  const sys = () => props.overview.system;
  const readyRun = () => (props.status?.state === "ready" ? props.status.run : null);

  return (
    <section>
      <Head
        title="Impostazioni"
        sub="Della macchina e dell'app. I profili non contengono niente di quello che vedi qui: percorsi e build sono della macchina."
      >
        <Show when={dirty()}>
          <span class="pill mod">modifiche non salvate</span>
        </Show>
        <button class="btn sm primary" disabled={!machine() || !dirty()} onClick={save}>
          Salva machine.toml
        </button>
      </Head>
      <Show when={message()}>
        {(m) => <div class={`note ${m().kind === "err" ? "err" : ""} mb`}>{m().text}</div>}
      </Show>
      <Show when={props.overview.machine_error}>
        <div class="card mb">
          <h2>
            machine.toml non è valido <span class="r mono">{raw()?.path}</span>
          </h2>
          <div class="note err mb">{props.overview.machine_error}</div>
          <p style={{ margin: "0 0 8px", color: "var(--fg2)" }}>
            Aethera non lo tocca: finché è così non sa dove sono i pesi né quali build hai dichiarato, ma niente è andato perso.
            Correggilo con un editor — il contenuto è qui sotto — oppure fattene scrivere uno nuovo: il vecchio viene messo da
            parte con la data, non cancellato.
          </p>
          <Show when={raw()?.text}>
            <pre class="log">{raw()!.text}</pre>
          </Show>
          <div class="row" style={{ "margin-top": "8px" }}>
            <button class="btn danger" onClick={() => setAskReset(true)}>
              Scrivi un machine.toml nuovo…
            </button>
            <button
              class="btn"
              onClick={() =>
                run(async () => {
                  setRaw(await api.machineText());
                })
              }
            >
              Rileggi
            </button>
          </div>
        </div>
      </Show>
      <Show when={props.overview.endpoint_error}>
        <div class="note err mb">
          <b>Endpoint Aethera:</b> {props.overview.endpoint_error}
        </div>
      </Show>

      <div class="tabs" role="tablist">
        <For each={TABS}>
          {(t) => (
            <button role="tab" aria-selected={props.tab === t.id} classList={{ on: props.tab === t.id }} onClick={() => props.onTab(t.id)}>
              {t.label}
            </button>
          )}
        </For>
      </div>

      <Show when={props.tab === "macchina"}>
        <div class="grid g2" style={{ "align-items": "start" }}>
          <div class="grid" style={{ "align-items": "start" }}>
            <div class="card">
              <h2>
                Radice dati <span class="r">machine.toml · profiles · builds · runs</span>
              </h2>
              <Field label="Radice" hint={<button class="btn sm" onClick={changeRoot}>Cambia…</button>}>
                <span class="mono" style={{ "overflow-wrap": "anywhere" }}>
                  {props.overview.data_root}
                </span>
              </Field>
              <Show when={machine()}>
                {(m) => (
                  <>
                    <Field label="Nome macchina" lever="name">
                      <input aria-label="Nome macchina" value={m().name} onInput={(e) => patch({ name: e.currentTarget.value })} />
                    </Field>
                    <Field
                      label="Cartella pesi"
                      lever="models_dir"
                      hint={
                        <button
                          class="btn sm"
                          onClick={() =>
                            run(async () => {
                              const d = await pickDir();
                              if (d) patch({ models_dir: d });
                            })
                          }
                        >
                          Cambia…
                        </button>
                      }
                    >
                      <input
                        aria-label="Cartella pesi"
                        value={m().models_dir ?? ""}
                        placeholder="non impostata"
                        onInput={(e) => patch({ models_dir: e.currentTarget.value.trim() || null })}
                      />
                    </Field>
                    <Field label="Margine RAM" lever="ram_margin_gib" hint="GiB disponibili al picco">
                      <input
                        aria-label="Margine RAM"
                        value={String(m().ram_margin_gib)}
                        onInput={(e) => {
                          const n = Number(e.currentTarget.value.replace(",", "."));
                          if (Number.isFinite(n)) patch({ ram_margin_gib: n });
                        }}
                      />
                    </Field>
                  </>
                )}
              </Show>
            </div>

            <div class="card">
              <h2>
                Build llama.cpp{" "}
                <Q title="Sotto ogni cartella, da dove viene la build. «build scaricata da ggml-org»: accanto a llama-server non c'è provenienza.toml. Una build compilata su questa macchina porta il tag di partenza, la serie (moro0 = tag liscio, moro1 e oltre = con patch), i rami con il loro commit, data e durata della build. «provenienza assente»: l'id dichiara una serie del fork ma il file manca. Un profilo usa una build patchata solo se la chiede per nome." />
                <span class="r">dichiarate in machine.toml o trovate in builds/</span>
              </h2>
              <div style={{ "overflow-x": "auto" }}>
                <table>
                  <thead>
                    <tr>
                      <th>id</th>
                      <th>cartella</th>
                      <th />
                    </tr>
                  </thead>
                  <tbody>
                    <For
                      each={builds()}
                      fallback={
                        <tr>
                          <td colspan={3} class="cond">
                            Nessuna build dichiarata in machine.toml.
                          </td>
                        </tr>
                      }
                    >
                      {(b, i) => (
                        <tr>
                          <td style={{ width: "30%" }}>
                            <input
                              class="txt mono"
                              style={{ width: "100%" }}
                              aria-label="id della build"
                              value={b.id}
                              onInput={(e) => patch({ build: builds().map((x, j) => (j === i() ? { ...x, id: e.currentTarget.value } : x)) })}
                            />
                          </td>
                          <td class="mono mini" style={{ "overflow-wrap": "anywhere" }}>
                            {b.path}
                            <Show when={props.overview.builds_missing.some((x) => x.path === b.path)}>
                              {" "}
                              <span class="badge err tight">
                                <i />
                                llama-server assente
                              </span>
                            </Show>
                            <ProvenanceLine id={b.id} entry={provenanceOf(b.path)} />
                          </td>
                          <td class="r">
                            <button class="btn sm" onClick={() => patch({ build: builds().filter((_, j) => j !== i()) })}>
                              Togli
                            </button>
                          </td>
                        </tr>
                      )}
                    </For>
                  </tbody>
                </table>
              </div>
              <div class="row" style={{ "margin-top": "8px" }}>
                <button class="btn sm" onClick={addBuild}>
                  Aggiungi build…
                </button>
                <span class="cond" style={{ flex: "1", "min-width": "200px" }}>
                  L'id porta build e backend separati da trattini (per esempio <code>b10809-vulkan</code>): un profilo con build{" "}
                  <code>b10809</code> e backend <code>vulkan</code> la trova così.
                </span>
              </div>
              <Show when={props.overview.builds.length}>
                <hr />
                <h2>Trovate</h2>
                <table style={{ "font-size": "12px" }}>
                  <tbody>
                    <For each={props.overview.builds}>
                      {(b) => (
                        <tr>
                          <td class="mono">{b.id}</td>
                          <td class="mono mini" style={{ "overflow-wrap": "anywhere" }}>
                            {b.binary}
                            <ProvenanceLine id={b.id} entry={provenanceOf(b.dir)} />
                          </td>
                          <td class="cond">{b.source}</td>
                        </tr>
                      )}
                    </For>
                  </tbody>
                </table>
              </Show>
            </div>
          </div>

          <div class="card">
            <h2>
              Cosa Aethera legge della macchina{" "}
              <Q title="Driver, alimentazione e volume finiscono nel manifest di ogni avvio: la pagina Benchmark non confronta due avvii con condizioni diverse senza dirlo. Un valore che Windows non dà resta «sconosciuto»." />
              <span class="r">letto, non impostato</span>
            </h2>
            <dl class="kv">
              <dt>Host</dt>
              <dd class="mono">
                <Val v={sys().hostname} />
              </dd>
              <dt>OS</dt>
              <dd>
                <Val v={sys().os} mono={false} />
              </dd>
              <dt>CPU</dt>
              <dd>
                <Val v={sys().cpu} mono={false} />
              </dd>
              <dt>RAM vista</dt>
              <dd>
                <Val v={fixed(sys().ram_total_gib, 2)} unit="GiB" /> · disponibile ora{" "}
                <Val v={fixed(sys().ram_available_gib, 2)} unit="GiB" />
              </dd>
              <For each={sys().gpus} fallback={<><dt>GPU</dt><dd><Val v={null} /></dd></>}>
                {(g) => (
                  <>
                    <dt>GPU</dt>
                    <dd>
                      {g.name} · <Val v={g.dedicated_gib} unit="GiB dedicati" />{" "}
                      <span class="cond">registro del driver: UMA + VGM</span>
                    </dd>
                  </>
                )}
              </For>
              <For each={cond().gpus ?? []}>
                {(g) => (
                  <>
                    <dt>Driver GPU</dt>
                    <dd>
                      <span class="mono">
                        <Val v={g.version} />
                      </span>{" "}
                      <span class="cond">
                        {g.date ?? ""}
                        {cond().adrenalin ? ` · AMD Software ${cond().adrenalin}` : ""}
                      </span>
                    </dd>
                  </>
                )}
              </For>
              <For each={cond().npus ?? []} fallback={<><dt>NPU</dt><dd><Val v={null} /></dd></>}>
                {(n) => (
                  <>
                    <dt>NPU</dt>
                    <dd>
                      {n.name} ·{" "}
                      <span class="mono">
                        <Val v={n.version} />
                      </span>
                    </dd>
                  </>
                )}
              </For>
              <dt>Alimentazione</dt>
              <dd>
                <Val v={api.overlayName(cond().power_overlay)} mono={false} />{" "}
                <span class="cond">overlay dal registro · powercfg mostra solo lo schema sotto</span>
              </dd>
              <dt>Volume dei pesi</dt>
              <dd>
                <span class="mono">
                  <Val v={cond().weights_volume} />
                </span>{" "}
                · <Val v={cond().weights_disk} mono={false} /> <span class="cond">{cond().weights_bus ?? ""}</span>
                <Show when={cond().weights_free_gb != null}>
                  {" "}
                  · <span class="num">{num(Math.round(cond().weights_free_gb!))}</span> GB liberi
                </Show>
              </dd>
            </dl>
            <div class="note" style={{ "margin-top": "10px" }}>
              Questi valori non si impostano da qui: la VGM si cambia da Adrenalin e chiede un riavvio; l'alimentazione da
              Windows. Aethera li legge a ogni avvio e li scrive nel manifest.
            </div>
          </div>
        </div>
      </Show>

      <Show when={props.tab === "client"}>
        <Show
          when={snippets()}
          fallback={
            <Empty title="Nessun motore acceso da questo Aethera.">
              <div>
                Le righe da incollare nei client nascono da un avvio vero: indirizzo, alias servito, contesto davvero servito e
                campionamento consigliato del modello. Prima dell'avvio sarebbero un'ipotesi. Anche il budget di contesto si
                legge sull'avvio acceso.
              </div>
            </Empty>
          }
        >
          {(sn) => (
            <div class="grid g2" style={{ "align-items": "start" }}>
              <div class="card">
                <h2>
                  Riga per i client{" "}
                  <span class="r">
                    derivata dall'avvio acceso
                    <Show when={readyRun()}>
                      {(r) => (
                        <>
                          {" "}
                          · {r().profile} · {r().base_url.replace(/^https?:\/\/[^:/]+/, "")}
                          <Show when={ctxServed() != null}> · {num(ctxServed())}</Show>
                        </>
                      )}
                    </Show>
                  </span>
                </h2>
                <div class="tabs" style={{ "margin-bottom": "8px" }}>
                  <For each={CLIENT_TABS}>
                    {(t) => (
                      <button classList={{ on: clientTab() === t.id }} onClick={() => setClientTab(t.id)}>
                        {t.label}
                      </button>
                    )}
                  </For>
                </div>
                <Show when={clientTab() === "nonio"}>
                  <div class="mini" style={{ "margin-bottom": "4px" }}>
                    <code>profile.toml</code> di Nonio
                  </div>
                  <pre class="cmd">{sn().toml}</pre>
                  <div class="row" style={{ "margin-top": "8px" }}>
                    <button class="btn sm" onClick={() => copy(sn().toml)}>
                      Copia TOML
                    </button>
                  </div>
                </Show>
                <Show when={clientTab() === "opencode"}>
                  <div class="mini" style={{ "margin-bottom": "4px" }}>
                    <code>opencode.json</code> · provider compatibile OpenAI
                  </div>
                  <pre class="cmd">{sn().opencode}</pre>
                  <div class="row" style={{ "margin-top": "8px" }}>
                    <button class="btn sm" onClick={() => copy(sn().opencode)}>
                      Copia JSON
                    </button>
                    <span class="cond right">contesto e output sono quelli dell'avvio acceso</span>
                  </div>
                </Show>
                <Show when={clientTab() === "galaxy"}>
                  <Show
                    when={sn().galaxycenter}
                    fallback={
                      <div class="note warn">
                        GalaxyCenter vuole tre endpoint — chat, embedding e rerank — e il suo contratto dice che i
                        server li gestisce l'operatore. Qui c'è solo la chat: accendi i motori di servizio nella
                        pagina Motore, e questa scheda si riempie.
                      </div>
                    }
                  >
                    {(t) => (
                      <>
                        <div class="mini" style={{ "margin-bottom": "4px" }}>
                          <code>%APPDATA%\GalaxyCenter\settings.toml</code> · la sezione dei modelli
                        </div>
                        <pre class="cmd">{t()}</pre>
                        <div class="row" style={{ "margin-top": "8px" }}>
                          <button class="btn sm" onClick={() => copy(t())}>
                            Copia TOML
                          </button>
                          <span class="cond right">
                            le dimensioni dell'embedding non si cambiano a cuor leggero: chi le consuma le fissa, e
                            cambiarle obbliga a reindicizzare tutto
                          </span>
                        </div>
                      </>
                    )}
                  </Show>
                </Show>
                <Show when={clientTab() === "claude"}>
                  <div class="row mb">
                    <Show
                      when={sn().claude_code_ready}
                      fallback={
                        <span class="badge err">
                          <i />
                          template del modello
                        </span>
                      }
                    >
                      <span class="badge ok">
                        <i />
                        template tollerante attivo
                      </span>
                    </Show>
                    <span class="cond" style={{ flex: "1", "min-width": "200px" }}>
                      <Show
                        when={sn().claude_code_ready}
                        fallback="Claude Code riceverà errore 500 alla seconda richiesta: avvia un profilo con chat_template_file = qwen3.6-tollerante.jinja."
                      >
                        l'avvio passa <code>--chat-template-file {sn().chat_template}</code>: Claude Code punta dritto al motore
                      </Show>
                    </span>
                  </div>
                  <div class="mini" style={{ "margin-bottom": "4px" }}>
                    PowerShell · da incollare prima di <code>claude</code>
                  </div>
                  <pre class="cmd">{sn().claude_code_powershell}</pre>
                  <div class="row" style={{ "margin-top": "8px" }}>
                    <button class="btn sm" onClick={() => copy(sn().claude_code_powershell)}>
                      Copia come PowerShell
                    </button>
                    <button class="btn sm" onClick={() => copy(sn().claude_code_bash)}>
                      Copia come bash
                    </button>
                    <span class="cond right">alias, porta e output sono quelli dell'avvio acceso</span>
                  </div>
                </Show>
                <Show when={clientTab() === "env"}>
                  <div class="mini" style={{ "margin-bottom": "4px" }}>
                    Blocco d'ambiente per banchi e Diorama
                  </div>
                  <pre class="cmd">{sn().env}</pre>
                  <div class="row" style={{ "margin-top": "8px" }}>
                    <button class="btn sm" onClick={() => copy(sn().env)}>
                      Copia blocco env
                    </button>
                    <button class="btn sm" onClick={() => copy(sn().powershell)}>
                      Copia come PowerShell
                    </button>
                  </div>
                </Show>
              </div>
              <BudgetCard sn={sn()} ctx={ctxServed()} status={props.status} />
            </div>
          )}
        </Show>
      </Show>

      <Show when={props.tab === "app"}>
        <div class="card mb">
          <h2>
            Profilo principale{" "}
            <Q title="Il profilo che Avvio seleziona da solo aprendo la pagina, in cima alla lista con l'etichetta «principale», e che la tray può accendere con «Avvia». «Nessuno»: Avvio si apre come oggi, sul primo della lista." />
          </h2>
          <Field label="Profilo">
            <select
              aria-label="Profilo principale"
              value={props.overview.default_profile ?? ""}
              onChange={(e) => setDefaultProfile(e.currentTarget.value)}
            >
              <option value="">nessuno</option>
              <For each={profiles()}>{(p) => <option value={p.name}>{p.name}</option>}</For>
              <Show when={props.overview.default_profile && !profiles().some((p) => p.name === props.overview.default_profile)}>
                <option value={props.overview.default_profile!}>{props.overview.default_profile} (non trovato)</option>
              </Show>
            </select>
          </Field>
          <div class="cond" style={{ "margin-top": "6px" }}>
            <b>Come si legge:</b> serve a chi usa sempre lo stesso modello: la pagina Avvio si apre già su questo profilo,
            e la tray offre «Avvia {props.overview.default_profile ?? "…"}» con gli stessi controlli dell'avvio dalla
            finestra. Se il profilo viene rinominato o cancellato, questa scelta non si cancella da sola: Avvio lo dice e
            torna al comportamento di prima finché non ne scegli un altro qui.
          </div>
        </div>
        <div class="grid g2" style={{ "align-items": "start" }}>
          <div class="card">
            <h2>Comportamento</h2>
            <Field label="Chiudere la finestra">
              <span class="pill">con il motore acceso riduce nella tray</span>
            </Field>
            <Field label="All'uscita">
              <select
                aria-label="All'uscita"
                value={props.overview.exit_behavior}
                onChange={(e) => exitBehavior(e.currentTarget.value as ExitBehavior)}
              >
                <option value="ask">chiedi ogni volta</option>
                <option value="stop">ferma il motore</option>
                <option value="leave">lascia il motore acceso</option>
              </select>
            </Field>
            <Field label="Tema" hint="solo su questo computer">
              <Seg
                label="Tema"
                value={props.theme}
                options={[
                  { id: "dark", label: "scuro" },
                  { id: "light", label: "chiaro" },
                ]}
                onChange={(t) => props.onTheme(t as "dark" | "light")}
              />
            </Field>
          </div>
          <div class="card">
            <h2>
              Soglie <span class="r">da confermare · approvate con M-09 dove indicato</span>
            </h2>
            <dl class="kv">
              <dt>Stato «in uso»</dt>
              <dd>
                slot attivo su <code>/slots</code>, richieste negli ultimi <span class="num">30</span> s, lock dichiarato o
                protezione manuale
              </dd>
              <dt>Segnala degradato</dt>
              <dd>
                acceso da più di <span class="num">24</span> h, o decode delle ultime 5 richieste sotto il{" "}
                <span class="num">70</span> % della mediana di riferimento (stesse condizioni), altrimenti di quella dell'avvio
              </dd>
              <dt>Richiesta «estende»</dt>
              <dd>
                tiene almeno il <span class="num">90</span> % della conversazione di prima, o tutto tranne l'ultimo ubatch{" "}
                <span class="cond">M-09</span>
              </dd>
              <dt>Contesto ricostruito</dt>
              <dd>
                prompt più corto del <span class="num">70</span> % della conversazione di prima <span class="cond">M-09</span>
              </dd>
              <dt>Budget rosso</dt>
              <dd>
                spazio di lavoro sotto il <span class="num">30</span> % del contesto servito <span class="cond">M-09</span>
              </dd>
              <dt>Avviso mmap</dt>
              <dd>
                load_mode con mmap e pesi oltre il <span class="num">90</span> % della memoria dedicata{" "}
                <span class="cond">M-09</span>
              </dd>
            </dl>
          </div>
        </div>

        <details style={{ "margin-top": "12px" }}>
          <summary>
            Endpoint Aethera <span class="mono cond">{props.overview.endpoint ?? "non attivo"}</span>
            <span class="r">per i client che vogliono dichiarare «sto lavorando»</span>
          </summary>
          <div class="body">
            <div style={{ "overflow-x": "auto" }}>
              <table style={{ "font-size": "12px", "margin-top": "8px" }}>
                <thead>
                  <tr>
                    <th>Metodo</th>
                    <th>Percorso</th>
                    <th>Cosa fa</th>
                  </tr>
                </thead>
                <tbody>
                  <tr>
                    <td class="mono">GET</td>
                    <td class="mono">/status</td>
                    <td>stato, alias, porta, id avvio, acceso da, in uso e perché</td>
                  </tr>
                  <tr>
                    <td class="mono">GET</td>
                    <td class="mono">/run</td>
                    <td>manifest dell'avvio corrente</td>
                  </tr>
                  <tr>
                    <td class="mono">POST</td>
                    <td class="mono">/lock</td>
                    <td>
                      dichiara «sto lavorando»: <code>{'{"client", "label", "ttl_s"}'}</code>; stesso client ed etichetta
                      rinnovano
                    </td>
                  </tr>
                  <tr>
                    <td class="mono">DELETE</td>
                    <td class="mono">/lock</td>
                    <td>
                      rilascia per <code>?id=</code> o <code>?client=</code>
                    </td>
                  </tr>
                  <tr>
                    <td class="mono">GET</td>
                    <td class="mono">/telemetry/recent</td>
                    <td>
                      prefill, decode, accettazione, quota cache, richieste con come hanno trattato la conversazione e
                      compattazioni delle ultime <code>?n=</code>
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>
            <div class="note" style={{ "margin-top": "8px" }}>
              Chi non lo usa è coperto dall'osservazione di <code>/slots</code> e dall'interruttore manuale. Il lock non cambia il
              motore: rende solo rifiutati arresto e riavvio. Le richieste da un browser (con <code>Origin</code>) sono
              rifiutate.
            </div>
          </div>
        </details>
      </Show>

      <Show when={askReset()}>
        <Confirm
          title="Scrivere un machine.toml nuovo?"
          lines={[
            `Il file di adesso viene rinominato in machine.toml.<data>.bak e resta nella radice dati: da lì puoi ricopiarne la cartella dei pesi e le build.`,
            "Al suo posto ne nasce uno vuoto, con il nome di questa macchina e nient'altro: cartella dei pesi e build vanno ridichiarate.",
            "I profili, gli avvii registrati e il catalogo non vengono toccati.",
          ]}
          confirmLabel="Scrivilo"
          danger
          onCancel={() => setAskReset(false)}
          onConfirm={() =>
            run(async () => {
              setAskReset(false);
              props.onChange(await api.machineReset());
              setMessage({ kind: "ok", text: "machine.toml rifatto. Il vecchio è nella radice dati con estensione .bak." });
            })
          }
        />
      </Show>
    </section>
  );
}
