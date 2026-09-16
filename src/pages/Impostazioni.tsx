import { open } from "@tauri-apps/plugin-dialog";
import { createEffect, createSignal, For, on, Show } from "solid-js";
import * as api from "../api";
import type { ClientSnippets, EngineStatus, ExitBehavior, MachineConfig, MachineText, Overview } from "../api";
import { Confirm, copy, Empty, Val } from "../components";
import { num, pct } from "../format";

type ClientTab = "nonio" | "opencode" | "claude" | "env";
const TABS: { id: ClientTab; label: string }[] = [
  { id: "nonio", label: "Nonio" },
  { id: "opencode", label: "OpenCode" },
  { id: "claude", label: "Claude Code" },
  { id: "env", label: "env per banchi" },
];

/** Quanto spazio resta a ogni client prima di compattare, sull'avvio acceso. */
function BudgetCard(props: { sn: ClientSnippets; ctx: number | null }) {
  return (
    <div class="card mb">
      <h2>
        Budget di contesto{" "}
        <span class="r">per client, sull'avvio acceso{props.ctx != null ? ` · ${num(props.ctx)} token serviti` : ""}</span>
      </h2>
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
                    <span class={`tag ${b.tight ? "reb" : "ext"}`}>{b.tight ? "compatta presto" : "largo"}</span>
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
      <div class="cond" style={{ "margin-top": "6px" }}>
        Il <b>prompt fisso</b> (istruzioni e strumenti del client) è misurato, mai stimato: passa il mouse sul numero per
        la fonte. <b>Spazio di lavoro</b> = contesto servito − prompt fisso − output riservato; rosso sotto il 30 % del
        contesto. Più spazio vuol dire compattazioni più rare, e ognuna costa decine di secondi di prefill.
      </div>
    </div>
  );
}

const input = {
  background: "var(--bg)",
  color: "var(--fg)",
  border: "1px solid var(--line)",
  "border-radius": "3px",
  padding: "3px 6px",
  font: "inherit",
  "font-family": "var(--mono)",
  "font-size": "12px",
  width: "100%",
};

export default function Impostazioni(props: { overview: Overview; status: EngineStatus | null; onChange: (o: Overview) => void }) {
  const [machine, setMachine] = createSignal<MachineConfig | null>(null);
  const [message, setMessage] = createSignal<{ kind: "ok" | "err"; text: string } | null>(null);
  const [snippets, setSnippets] = createSignal<ClientSnippets | null>(null);
  /** Il testo di machine.toml: serve solo quando non si lascia leggere, perché senza vederlo non si corregge. */
  const [raw, setRaw] = createSignal<MachineText | null>(null);
  const [askReset, setAskReset] = createSignal(false);
  const [tab, setTab] = createSignal<ClientTab>("nonio");
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

  const sys = () => props.overview.system;

  return (
    <section>
      <h1>Impostazioni</h1>
      <p class="sub">
        Configurazione della macchina e dell'app. I profili non contengono niente di quello che vedi qui: percorsi e
        build sono della macchina.
      </p>
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
            Aethera non lo tocca: finché è così non sa dove sono i pesi né quali build hai dichiarato, ma niente è andato
            perso. Correggilo con un editor — il contenuto è qui sotto — oppure fattene scrivere uno nuovo: il vecchio
            viene messo da parte con la data, non cancellato.
          </p>
          <Show when={raw()?.text}>
            <pre class="log">{raw()!.text}</pre>
          </Show>
          <div class="row" style={{ "margin-top": "8px" }}>
            <button class="btn danger" onClick={() => setAskReset(true)}>
              Scrivi un machine.toml nuovo…
            </button>
            <button class="btn" onClick={() => run(async () => { setRaw(await api.machineText()); })}>
              Rileggi
            </button>
          </div>
        </div>
      </Show>

      <div class="grid g2" style={{ "align-items": "start" }}>
        <div>
          <div class="card mb">
            <h2>
              Radice dati <span class="r">machine.toml · profiles · builds · runs</span>
            </h2>
            <div class="field">
              <label>Radice</label>
              <div class="mono">{props.overview.data_root}</div>
              <button class="btn sm" onClick={changeRoot}>
                Cambia…
              </button>
            </div>
            <Show when={machine()}>
              {(m) => (
                <>
                  <div class="field">
                    <label>Nome macchina</label>
                    <input value={m().name} onInput={(e) => patch({ name: e.currentTarget.value })} />
                    <span />
                  </div>
                  <div class="field">
                    <label>Cartella pesi</label>
                    <input value={m().models_dir ?? ""} onInput={(e) => patch({ models_dir: e.currentTarget.value.trim() || null })} />
                    <button class="btn sm" onClick={() => run(async () => { const d = await pickDir(); if (d) patch({ models_dir: d }); })}>
                      Cambia…
                    </button>
                  </div>
                  <div class="field">
                    <label>Margine RAM</label>
                    <input
                      value={String(m().ram_margin_gib)}
                      onInput={(e) => {
                        const n = Number(e.currentTarget.value.replace(",", "."));
                        if (Number.isFinite(n)) patch({ ram_margin_gib: n });
                      }}
                    />
                    <span class="cond">GiB disponibili al picco</span>
                  </div>
                </>
              )}
            </Show>
          </div>

          <div class="card mb">
            <h2>
              Build llama.cpp <span class="r">dichiarate in machine.toml o in builds/</span>
            </h2>
            <table>
              <thead>
                <tr>
                  <th>id</th>
                  <th>cartella</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                <For each={builds()}>
                  {(b, i) => (
                    <tr>
                      <td style={{ width: "30%" }}>
                        <input
                          style={input}
                          value={b.id}
                          onInput={(e) => patch({ build: builds().map((x, j) => (j === i() ? { ...x, id: e.currentTarget.value } : x)) })}
                        />
                      </td>
                      <td class="mono mini">
                        {b.path}
                        <Show when={props.overview.builds_missing.some((x) => x.path === b.path)}>
                          {" "}
                          <span class="badge err tight">
                            <i />
                            llama-server assente
                          </span>
                        </Show>
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
            <div class="cond" style={{ margin: "6px 0" }}>
              L'id porta build e backend separati da trattini (per esempio <code>b10809-vulkan</code>): un profilo con
              build <code>b10809</code> e backend <code>vulkan</code> la trova così.
            </div>
            <div class="row">
              <button class="btn sm" onClick={addBuild}>
                Aggiungi build…
              </button>
              <button class="btn sm primary right" disabled={!machine()} onClick={save}>
                Salva machine.toml
              </button>
            </div>
            <Show when={props.overview.builds.length}>
              <hr />
              <h2>Trovate</h2>
              <For each={props.overview.builds}>
                {(b) => (
                  <div class="mono mini">
                    {b.id} · {b.binary} <span class="cond">({b.source})</span>
                  </div>
                )}
              </For>
            </Show>
          </div>
        </div>

        <div>
          <div class="card mb">
            <h2>
              Riga per i client <span class="r">derivata dall'avvio acceso</span>
            </h2>
            <Show
              when={snippets()}
              fallback={
                <Empty title="Nessun motore acceso da questo Aethera.">
                  <div>
                    Le righe da incollare nei client nascono da un avvio vero: indirizzo, alias servito, contesto
                    davvero servito e campionamento consigliato del modello. Prima dell'avvio sarebbero un'ipotesi.
                  </div>
                </Empty>
              }
            >
              {(sn) => (
                <>
                  <div class="tabs">
                    <For each={TABS}>
                      {(t) => (
                        <button classList={{ on: tab() === t.id }} onClick={() => setTab(t.id)}>
                          {t.label}
                        </button>
                      )}
                    </For>
                  </div>
                  <Show when={tab() === "nonio"}>
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
                  <Show when={tab() === "opencode"}>
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
                  <Show when={tab() === "claude"}>
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
                      <span class="cond">
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
                  <Show when={tab() === "env"}>
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
                </>
              )}
            </Show>
          </div>

          <Show when={snippets()}>{(sn) => <BudgetCard sn={sn()} ctx={ctxServed()} />}</Show>

          <div class="card mb">
            <h2>
              Endpoint Aethera <span class="r mono">{props.overview.endpoint ?? "non attivo"}</span>
            </h2>
            <Show when={props.overview.endpoint_error}>
              <div class="note err mb">{props.overview.endpoint_error}</div>
            </Show>
            <table style={{ "font-size": "12px" }}>
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
                    dichiara «sto lavorando»: <code>{'{"client", "label", "ttl_s"}'}</code>; stesso client ed etichetta rinnovano
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
            <div class="note" style={{ "margin-top": "8px" }}>
              Chi non lo usa è coperto dall'osservazione di <code>/slots</code> e dall'interruttore manuale. Il lock non
              cambia il motore: rende solo rifiutati arresto e riavvio. Le richieste da un browser (con <code>Origin</code>)
              sono rifiutate.
            </div>
          </div>

          <div class="card mb">
            <h2>
              Macchina <span class="r">letto, non impostato</span>
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
                <Val v={sys().ram_total_gib} unit="GiB" />
              </dd>
              <dt>RAM disponibile</dt>
              <dd>
                <Val v={sys().ram_available_gib} unit="GiB" /> <span class="cond">ora</span>
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
                <Val v={api.overlayName(cond().power_overlay)} mono={false} /> <span class="cond">overlay dal registro</span>
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
            <div class="note" style={{ "margin-top": "8px" }}>
              Driver, alimentazione e volume finiscono nel manifest di ogni avvio: la pagina Benchmark non confronta due
              avvii con condizioni diverse senza dirlo. Un valore che Windows non dà resta «sconosciuto».
            </div>
          </div>

          <div class="card">
            <h2>Comportamento</h2>
            <div class="field">
              <label>Chiudere la finestra</label>
              <div>
                <span class="pill">con il motore acceso riduce nella tray</span>
              </div>
              <span />
            </div>
            <div class="field">
              <label>All'uscita</label>
              <select value={props.overview.exit_behavior} onChange={(e) => exitBehavior(e.currentTarget.value as ExitBehavior)}>
                <option value="ask">chiedi ogni volta</option>
                <option value="stop">ferma il motore</option>
                <option value="leave">lascia il motore acceso</option>
              </select>
              <span />
            </div>
            <div class="field">
              <label>Stato «in uso»</label>
              <div>
                slot attivo su <code>/slots</code>, richieste negli ultimi <span class="num">30</span> s, lock dichiarato o
                protezione manuale
              </div>
              <span />
            </div>
            <div class="field">
              <label>Segnala degradato</label>
              <div>
                acceso da più di <span class="num">24</span> h o decode delle ultime 5 richieste sotto il{" "}
                <span class="num">70</span> % della mediana dell'avvio
              </div>
              <span />
            </div>
          </div>
        </div>
      </div>
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
