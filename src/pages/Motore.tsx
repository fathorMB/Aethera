import { createSignal, For, onCleanup, onMount, Show } from "solid-js";
import * as api from "../api";
import type { Conditions, EngineStatus, MemorySection, Orphan, Overview, ReadyStatus, Setup, Summary, TurnKind } from "../api";
import type { PageId } from "../App";
import { copy, Empty, Spark, StateBadge, Toggle, UsageBadge, Val } from "../components";
import { clock, duration, fixed, num, pct } from "../format";

function MemoryCard(props: { memory: MemorySection | null; loadMs: number | null }) {
  const b = () => props.memory?.before ?? null;
  const a = () => props.memory?.after_load ?? null;
  const mibToGib = (m: number | null | undefined) => (m == null ? null : m / 1024);
  return (
    <div class="card">
      <h2>
        Memoria{" "}
        <span class="r">
          <Show when={a()} fallback="in attesa della lettura dopo il caricamento">
            misurata {fixed(a()!.after_ms / 1000, 1)} s dopo l'avvio
            <Show when={props.loadMs != null}> · pronto in {fixed(props.loadMs! / 1000, 1)} s</Show>
          </Show>
        </span>
      </h2>
      <table>
        <thead>
          <tr>
            <th />
            <th class="r">Prima dell'avvio</th>
            <th class="r">Misurato</th>
          </tr>
        </thead>
        <tbody>
          <tr>
            <td>
              VRAM dedicata <span class="cond">GPU Process Memory</span>
            </td>
            <td class="r">
              <Val v={fixed(mibToGib(b()?.vram_free_mib), 2)} unit="GiB liberi" />
            </td>
            <td class="r">
              <Val v={fixed(a()?.vram_dedicated_gib, 2)} unit="GiB" />
            </td>
          </tr>
          <tr>
            <td>VRAM condivisa del processo</td>
            <td class="r cond">—</td>
            <td class="r">
              <Val v={fixed(a()?.vram_shared_gib, 2)} unit="GiB" />
            </td>
          </tr>
          <tr>
            <td>Working set del processo</td>
            <td class="r cond">—</td>
            <td class="r">
              <Val v={fixed(a()?.working_set_gib, 2)} unit="GiB" />
            </td>
          </tr>
          <tr>
            <td>RAM disponibile a Windows</td>
            <td class="r">
              <Val v={fixed(b()?.ram_available_gib, 2)} unit="GiB" />
            </td>
            <td class="r">
              <Val v={fixed(a()?.ram_available_gib, 2)} unit="GiB" />
            </td>
          </tr>
          <tr>
            <td>Doppia copia dei pesi</td>
            <td class="r cond">—</td>
            <td class="r">
              <Show when={a()?.double_copy != null} fallback={<Val v={null} />}>
                <span class={`badge tight ${a()!.double_copy ? "err" : "ok"}`}>
                  <i />
                  {a()!.double_copy ? "sì" : "no"}
                </span>{" "}
                <span class="cond">working set ≥ metà dei pesi</span>
              </Show>
            </td>
          </tr>
        </tbody>
      </table>
      <div class="cond" style={{ "margin-top": "6px" }}>
        <Show when={b()?.device}>
          {b()!.device} · {num(b()!.vram_total_mib)} MiB <span>(--list-devices)</span> ·{" "}
        </Show>
        <Show when={a()} fallback="soglia di margine RAM da machine.toml">
          soglia di margine RAM ≥ {fixed(a()!.ram_margin_gib, 0)} GiB:{" "}
          <Show when={a()!.margin_ok != null} fallback={<Val v={null} />}>
            <span class="num" style={{ color: a()!.margin_ok ? "var(--ok)" : "var(--err)" }}>
              {fixed(a()!.ram_available_gib, 2)} {a()!.margin_ok ? "✓" : "✗ sotto la soglia"}
            </span>
          </Show>
        </Show>
      </div>
    </div>
  );
}

/** Driver, alimentazione, VGM e disco dei pesi: le condizioni che M-08 ha dovuto ricavare a mano. */
function ConditionsCard(props: { conditions: Conditions | null; changed: string[] }) {
  const c = () => props.conditions;
  const changedKey = (k: string) => props.changed.some((x) => x.startsWith(k));
  return (
    <div class="card">
      <h2>
        Condizioni dell'avvio <span class="r">lette all'avvio, nel manifest</span>
      </h2>
      <Show when={c()} fallback={<div class="cond">Non registrate: l'avvio è precedente a quando Aethera le legge.</div>}>
        {(k) => (
          <dl class="kv">
            <For each={k().gpus ?? []} fallback={<><dt>Driver GPU</dt><dd><Val v={null} /></dd></>}>
              {(g) => (
                <>
                  <dt>Driver GPU</dt>
                  <dd>
                    <span class="mono">
                      <Val v={g.version} />
                    </span>{" "}
                    <Show when={changedKey("driver GPU")}>
                      <span class="pill mod">cambiato</span>
                    </Show>
                    <div class="cond">
                      {g.name}
                      {g.date ? ` · ${g.date}` : ""}
                      {k().adrenalin ? ` · AMD Software ${k().adrenalin}` : ""}
                    </div>
                  </dd>
                </>
              )}
            </For>
            <For each={k().npus ?? []} fallback={<><dt>Driver NPU</dt><dd><Val v={null} /></dd></>}>
              {(n) => (
                <>
                  <dt>Driver NPU</dt>
                  <dd>
                    <span class="mono">
                      <Val v={n.version} />
                    </span>{" "}
                    <Show when={changedKey("driver NPU")}>
                      <span class="pill mod">cambiato</span>
                    </Show>
                    <div class="cond">{n.name}</div>
                  </dd>
                </>
              )}
            </For>
            <dt>Alimentazione</dt>
            <dd>
              <Val v={api.overlayName(k().power_overlay)} mono={false} />{" "}
              <Show when={changedKey("alimentazione")}>
                <span class="pill mod">cambiata</span>
              </Show>
              <div class="cond">overlay dal registro: powercfg mostra solo lo schema sotto</div>
            </dd>
            <dt>VGM</dt>
            <dd>
              <Val v={fixed(k().gpus?.find((g) => g.dedicated_gib != null)?.dedicated_gib, 0)} unit="GiB dedicati" />
            </dd>
            <dt>Pesi su</dt>
            <dd>
              <span class="mono">
                <Val v={k().weights_volume} />
              </span>{" "}
              · <Val v={k().weights_disk} mono={false} />
              <div class="cond">
                {k().weights_bus ?? "bus sconosciuto"} ·{" "}
                {k().weights_free_gb != null ? `${num(Math.round(k().weights_free_gb!))} GB liberi all'avvio` : "spazio libero sconosciuto"}
              </div>
            </dd>
          </dl>
        )}
      </Show>
    </div>
  );
}

const KIND: Record<TurnKind, { label: string; cls: string }> = {
  cold: { label: "a freddo", cls: "cold" },
  extends: { label: "estende", cls: "ext" },
  rewrites: { label: "riscrive", cls: "sum" },
  summary: { label: "compattazione · riassunto", cls: "sum" },
  rebuilt: { label: "compattazione · contesto ricostruito", cls: "reb" },
  unknown: { label: "non attribuibile", cls: "cold" },
};

const seconds = (ms: number | null | undefined) => (ms == null ? null : `${fixed(ms / 1000, 1)} s`);

/** Richiesta per richiesta: quanto il motore ha riusato, quanto ha rielaborato, e le compattazioni. */
function TurnsCard(props: { summary: Summary }) {
  const turns = () => [...props.summary.turns].reverse();
  const last = () => props.summary.compactions[props.summary.compactions.length - 1];
  return (
    <div class="card mb">
      <h2>
        Richiesta per richiesta <span class="r">quanto il motore ha riusato e quanto ha rielaborato · dalla più recente</span>
      </h2>
      <Show when={last()}>
        {(c) => (
          <div class="note warn mb">
            <b>Compattazione alle {clock(c().at)}</b>
            {c().client ? ` di ${c().client}` : ""}: {num(c().reprocessed)} token rielaborati fra riassunto e contesto
            ricostruito, <b>{seconds(c().cost_ms)}</b> che la conversazione non avrebbe pagato continuando.
            <Show when={props.summary.compactions.length > 1}>
              {" "}
              Nelle ultime {props.summary.window} richieste: {props.summary.compactions.length} compattazioni,{" "}
              {seconds(props.summary.compactions.reduce((a, x) => a + x.cost_ms, 0))} in tutto.
            </Show>{" "}
            Più contesto servito le rende più rare: vedi <i>Budget di contesto</i> nelle Impostazioni.
          </div>
        )}
      </Show>
      <Show when={turns().length} fallback={<div class="cond">Nessuna richiesta servita da questo avvio.</div>}>
        <table class="ev">
          <thead>
            <tr>
              <th>ora</th>
              <th>client</th>
              <th class="r">prompt</th>
              <th class="r">riusati</th>
              <th class="r">rielaborati</th>
              <th style={{ width: "110px" }}>quota riusata</th>
              <th class="r">prefill</th>
              <th class="r">decode</th>
              <th>lettura</th>
            </tr>
          </thead>
          <tbody>
            <For each={turns()}>
              {(t) => {
                const share = () => (t.prompt_total ? (t.cache_n ?? 0) / t.prompt_total : null);
                const tone = () => (share() == null ? "" : share()! >= 0.9 ? "ok" : share()! >= 0.5 ? "warn" : "err");
                return (
                  <tr classList={{ sel: t.kind === "summary" || t.kind === "rebuilt" }}>
                    <td class="num">{clock(t.at)}</td>
                    <td class="mono">
                      <Val v={t.client} />
                    </td>
                    <td class="r">
                      <Val v={num(t.prompt_total)} />
                    </td>
                    <td class="r">
                      <Val v={num(t.cache_n)} />
                    </td>
                    <td class="r num">{num(t.prompt_n)}</td>
                    <td>
                      <Show when={share() != null} fallback={<Val v={null} />}>
                        <div class={`bar ${tone()}`} title={pct(share()) + " %"}>
                          <span style={{ width: `${Math.max(share()! * 100, 1)}%` }} />
                        </div>
                      </Show>
                    </td>
                    <td class="r num">{seconds(t.prompt_ms)}</td>
                    <td class="r num">{seconds(t.gen_ms)}</td>
                    <td>
                      <span class={`tag ${KIND[t.kind].cls}`}>{KIND[t.kind].label}</span>
                    </td>
                  </tr>
                );
              }}
            </For>
          </tbody>
        </table>
      </Show>
      <div class="cond" style={{ "margin-top": "6px" }}>
        I token riusati vengono dal log del motore (contesto a fine richiesta meno generati e rielaborati, a meno di 2 token
        {props.summary.cache_sources.some((x) => x !== "log") ? `; alcune righe da ${props.summary.cache_sources.filter((x) => x !== "log").join(" e ")}` : ""}).
        Il <b>client</b> si conosce solo se ha preso il lock sull'endpoint di Aethera. <b>Estende</b>: tiene almeno il
        90 % della conversazione di prima, o tutto tranne l'ultimo ubatch. <b>Contesto ricostruito</b>: il prompt è più
        corto del 70 % della conversazione di prima; la richiesta che riscriveva subito prima è il <b>riassunto</b>.
      </div>
    </div>
  );
}

function OrphanCard(props: { orphan: Orphan }) {
  const [confirm, setConfirm] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const terminate = async () => {
    setError(null);
    try {
      await api.orphanTerminate(props.orphan.pid);
    } catch (e) {
      setError(String(e));
    }
    setConfirm(false);
  };
  return (
    <div class="card mb">
      <h2>
        Orfano <span class="r">sola lettura</span>
      </h2>
      <div class="row mb">
        <StateBadge status={{ state: "orphan", orphan: props.orphan, last: null }} />
        <span class="cond">un llama-server su questa porta non è stato avviato da questo Aethera</span>
      </div>
      <dl class="kv">
        <dt>Indirizzo</dt>
        <dd class="mono">{props.orphan.base_url}</dd>
        <dt>Processo</dt>
        <dd class="mono">
          {props.orphan.process} · PID {props.orphan.pid}
        </dd>
        <dt>Alias servito</dt>
        <dd class="mono">
          <Val v={props.orphan.alias} />
        </dd>
        <dt>Contesto servito</dt>
        <dd>
          <Val v={num(props.orphan.ctx)} />
        </dd>
        <dt>Slot</dt>
        <dd>
          <Show when={props.orphan.slot_processing != null} fallback={<Val v={null} />}>
            {props.orphan.slot_processing ? "al lavoro" : "libero"}
          </Show>
        </dd>
        <Show when={props.orphan.left_by_aethera}>
          <dt>Origine</dt>
          <dd>
            lasciato acceso all'uscita dall'avvio <span class="mono">{props.orphan.left_by_aethera}</span>
          </dd>
        </Show>
      </dl>
      <Show when={error()}>
        <div class="note err" style={{ "margin-top": "8px" }}>
          {error()}
        </div>
      </Show>
      <div class="row" style={{ "margin-top": "10px" }}>
        <span class="cond">Aethera non ha il suo log né la sua telemetria: può solo leggerlo o terminarlo.</span>
        <Show
          when={confirm()}
          fallback={
            <button class="btn sm danger right" disabled={props.orphan.slot_processing === true} onClick={() => setConfirm(true)}>
              Termina…
            </button>
          }
        >
          <span class="right">Terminare il PID {props.orphan.pid}?</span>
          <button class="btn sm danger" onClick={terminate}>
            Termina
          </button>
          <button class="btn sm" onClick={() => setConfirm(false)}>
            Annulla
          </button>
        </Show>
      </div>
    </div>
  );
}

/**
 * La prima configurazione, in ordine. Non è una procedura guidata che prende in ostaggio la
 * finestra: è un elenco che dice a che punto si è e dove si fa il passo che manca. Ogni passo si
 * può fare anche per conto proprio, e questo elenco se ne accorge.
 */
function PrimoAvvio(props: { setup: Setup; onGo: (page: PageId) => void }) {
  const next = () => props.setup.steps.find((s) => !s.done);
  return (
    <div class="card mb">
      <h2>
        Primo avvio <span class="r">dalla cartella dei dati al motore acceso</span>
      </h2>
      <div class="steps">
        <For each={props.setup.steps}>
          {(s, i) => (
            <div class="step" classList={{ done: s.done, now: s.id === next()?.id }}>
              <span class="n">{s.done ? "✓" : i() + 1}</span>
              <span>
                <span class="t">{s.title}</span>
                <span class="d"> — {s.done ? s.detail : s.what}</span>
              </span>
              <Show when={s.id === next()?.id} fallback={<span />}>
                <button class="btn sm primary" onClick={() => props.onGo(s.page as PageId)}>
                  Vai
                </button>
              </Show>
            </div>
          )}
        </For>
      </div>
    </div>
  );
}

export default function Motore(props: { status: EngineStatus | null; overview: Overview; onGo: (page: PageId) => void }) {
  const [setup, setSetup] = createSignal<Setup | null>(null);
  const [log, setLog] = createSignal("");
  /** Le righe del log che spiegano un'uscita con errore: un codice di uscita da solo non dice niente. */
  const [failure, setFailure] = createSignal<api.Reason[]>([]);
  const [error, setError] = createSignal<string | null>(null);
  const [busy, setBusy] = createSignal(false);
  const run = () => api.runOf(props.status);
  const s = () => props.status;
  const ready = () => (s()?.state === "ready" ? (s() as ReadyStatus) : null);
  const usage = () => api.usageOf(s());
  const inUse = () => api.inUse(s());

  onMount(() => {
    const tick = async () => {
      if (!run()) return;
      try {
        setLog(await api.engineLog(200));
      } catch (e) {
        setLog(String(e));
      }
      // Solo dopo un'uscita con errore: a motore sano non c'è niente da spiegare.
      setFailure(s()?.state === "exited" ? await api.engineFailure().catch(() => []) : []);
    };
    tick();
    const t = setInterval(tick, 2000);

    // La guida si aggiorna da sola: un passo fatto in un'altra pagina si vede tornando qui.
    const setupTick = async () => setSetup(await api.setup().catch(() => null));
    setupTick();
    const t2 = setInterval(setupTick, 3000);
    onCleanup(() => {
      clearInterval(t);
      clearInterval(t2);
    });
  });

  const act = async (fn: () => Promise<unknown>) => {
    setError(null);
    setBusy(true);
    try {
      await fn();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const copyClient = () =>
    act(async () => {
      const sn = await api.clientSnippets();
      if (sn) await copy(sn.toml);
    });

  const canRestart = () => !busy() && !inUse() && (s()?.state === "ready" || s()?.state === "exited" || (s()?.state === "off" && run() != null));
  const decodeLow = () => {
    const d = ready()?.telemetry.decode_median;
    return d == null ? undefined : d * 0.7;
  };

  return (
    <section>
      <h1>Motore</h1>
      <p class="sub">
        Un processo <code>llama-server</code>, avviato e fermato da Aethera. Tutto quello che vedi qui è misurato dopo
        l'avvio; quello che non è misurato è «sconosciuto».
      </p>

      <Show when={error()}>
        <div class="note err mb">{error()}</div>
      </Show>

      <Show when={s()?.state === "exited"}>
        <div class="card mb">
          <h2>
            Perché è uscito <span class="r">dalle righe del log, non dal codice di uscita</span>
          </h2>
          <Show
            when={failure().length}
            fallback={
              <div class="cond">
                Il log non contiene nessuna riga riconoscibile come causa. Resta il codice di uscita qui sotto e il log
                per intero in fondo alla pagina: meglio dire che non si sa, che indicare una causa sbagliata.
              </div>
            }
          >
            <For each={failure()}>
              {(r) => (
                <div style={{ "margin-bottom": "8px" }}>
                  <pre class="cmd" style={{ margin: "0 0 4px" }}>
                    {r.line}
                  </pre>
                  <Show when={r.hint}>
                    <div class="note warn">{r.hint}</div>
                  </Show>
                </div>
              )}
            </For>
          </Show>
        </div>
      </Show>

      <Show when={setup() && !setup()!.complete}>
        <PrimoAvvio setup={setup()!} onGo={props.onGo} />
      </Show>

      <Show when={s()?.state === "orphan" && s()}>
        {(o) => <OrphanCard orphan={(o() as Extract<EngineStatus, { state: "orphan" }>).orphan} />}
      </Show>

      <Show
        when={run()}
        fallback={
          <Show when={s()?.state !== "orphan"}>
            <Empty
              title="Nessun motore acceso da Aethera."
              action={
                <button class="btn sm primary" onClick={() => props.onGo("avvio")}>
                  Vai ad Avvio
                </button>
              }
            >
              <div>
                Questa pagina misura un <code>llama-server</code> mentre gira: memoria occupata davvero, velocità
                recenti, quota di cache del prefisso, chi lo sta usando. Finché non ne accendi uno non c'è niente da
                misurare — e quello che non è misurato resta «sconosciuto», non zero.
              </div>
            </Empty>
          </Show>
        }
      >
        {(r) => (
          <>
            <div class="grid g4 mb">
              <div class="card">
                <h2>
                  Stato <span class="r">/health · /slots</span>
                </h2>
                <div class="row">
                  <StateBadge status={s()} />
                  <UsageBadge status={s()} detail={false} />
                </div>
                <dl class="kv" style={{ "margin-top": "8px" }}>
                  <Show when={s()?.state === "loading" && s()}>
                    {(x) => {
                      const l = x() as Extract<EngineStatus, { state: "loading" }>;
                      return (
                        <>
                          <dt>In caricamento da</dt>
                          <dd class="num">{duration(l.elapsed_s)}</dd>
                          <dt>/health</dt>
                          <dd>
                            <Val v={l.health} />
                          </dd>
                        </>
                      );
                    }}
                  </Show>
                  <Show when={ready()}>
                    {(rd) => (
                      <>
                        <dt>Acceso da</dt>
                        <dd class="num">
                          {duration(rd().uptime_s)} <span class="cond">dal {clock(r().started_at)}</span>
                        </dd>
                        <dt>Token di prompt</dt>
                        <dd>
                          <Val v={num(rd().counters ? rd().counters!.prompt_tokens + rd().counters!.cached_tokens : null)} />{" "}
                          <span class="cond">/metrics, cache inclusa</span>
                        </dd>
                        <dt>Token generati</dt>
                        <dd>
                          <Val v={num(rd().counters?.predicted_tokens)} />
                        </dd>
                        <dt>Richieste</dt>
                        <dd class="num">{num(rd().telemetry.requests)}</dd>
                        <dt>Slot</dt>
                        <dd>
                          <span class="pill mono">{r().n_parallel}</span>{" "}
                          <Show when={rd().usage.slot_processing != null} fallback={<Val v={null} />}>
                            <span class="cond">is_processing = {String(rd().usage.slot_processing)}</span>
                          </Show>
                        </dd>
                      </>
                    )}
                  </Show>
                  <Show when={s()?.state === "exited" && s()}>
                    {(x) => {
                      const f = (x() as Extract<EngineStatus, { state: "exited" }>).finished;
                      return (
                        <>
                          <dt>Uscito</dt>
                          <dd class="num">{clock(f.ended_at)}</dd>
                          <dt>Codice</dt>
                          <dd>
                            <Val v={f.code} />
                          </dd>
                        </>
                      );
                    }}
                  </Show>
                  <Show when={(s()?.state === "off" || s()?.state === "orphan") && s()}>
                    {(x) => {
                      const f = (x() as Extract<EngineStatus, { state: "off" }>).last!;
                      return (
                        <>
                          <dt>Fermato</dt>
                          <dd class="num">{clock(f.ended_at)}</dd>
                          <Show when={f.left_running}>
                            <dt>Nota</dt>
                            <dd class="cond">lasciato acceso all'uscita: non più gestito</dd>
                          </Show>
                        </>
                      );
                    }}
                  </Show>
                  <For each={usage()?.locks ?? []}>
                    {(l) => (
                      <>
                        <dt>Client dichiarato</dt>
                        <dd class="mono">
                          {l.client}
                          {l.label ? ` · ${l.label}` : ""} <span class="cond">lock · scade fra {duration(l.expires_in_s)}</span>
                        </dd>
                      </>
                    )}
                  </For>
                  <dt>Avvio</dt>
                  <dd class="mono">
                    {r().run_id} · PID {r().pid}
                  </dd>
                </dl>
                <Show when={usage()}>
                  {(u) => (
                    <div style={{ "margin-top": "8px" }}>
                      <Toggle
                        on={u().protected}
                        label="Proteggi il motore"
                        title="Rifiuta arresto e riavvio finché è attiva"
                        onChange={(on) => act(() => api.engineProtect(on))}
                      />
                    </div>
                  )}
                </Show>
              </div>

              <div class="card">
                <h2>
                  Velocità recenti <span class="r">ultime {ready()?.telemetry.window ?? 0} richieste</span>
                </h2>
                <div class="grid g2">
                  <div>
                    <div class="big">
                      <Val v={fixed(ready()?.telemetry.prefill_median, 0)} />
                      <small>tok/s</small>
                    </div>
                    <div class="cond">prefill · mediana · ≥ 128 token</div>
                  </div>
                  <div>
                    <div class="big">
                      <Val v={fixed(ready()?.telemetry.decode_median, 1)} />
                      <small>tok/s</small>
                    </div>
                    <div class="cond">decode · mediana</div>
                  </div>
                  <div>
                    <div class="big">
                      <Val v={pct(ready()?.telemetry.acceptance)} />
                      <small>%</small>
                    </div>
                    <div class="cond">accettazione draft</div>
                  </div>
                  <div>
                    <div class="big">
                      <Val v={num(ready()?.telemetry.last_prompt)} />
                      <small>tok</small>
                    </div>
                    <div class="cond">ultimo prompt · elaborati + cache</div>
                  </div>
                </div>
                <Show when={ready()?.telemetry.decode_series.length}>
                  <Spark values={ready()!.telemetry.decode_series} low={decodeLow()} height={26} />
                </Show>
              </div>

              <div class="card">
                <h2>
                  Riuso del prompt <span class="r">ultime {ready()?.telemetry.window ?? 0} richieste · dal log</span>
                </h2>
                <div class="grid g2">
                  <div>
                    <div class="big">
                      <Val v={pct(ready()?.telemetry.cache_share)} />
                      <small>% riusato</small>
                    </div>
                    <div class="cond">token dalla cache / token di prompt</div>
                  </div>
                  <div>
                    <div class="big">
                      {ready()?.telemetry.compactions.length ?? 0}
                      <small>{ready()?.telemetry.compactions.length === 1 ? "compattazione" : "compattazioni"}</small>
                    </div>
                    <div class="cond">
                      <Show when={ready()?.telemetry.compactions.length} fallback="nessuna nella finestra">
                        costate {seconds(ready()!.telemetry.compactions.reduce((a, c) => a + c.cost_ms, 0))}
                      </Show>
                    </div>
                  </div>
                </div>
                <Show when={ready()?.telemetry.cache_series.length} fallback={<div class="cond">nessuna richiesta servita</div>}>
                  <div style={{ margin: "8px 0 4px" }}>
                    <Spark values={ready()!.telemetry.cache_series} max={1} low={0.5} />
                  </div>
                  <div class="cond">
                    Quota riusata per richiesta. Rossa sotto il 50 %: il client ha cambiato il prompt prima della coda.
                    Tratteggio = non attribuibile.
                  </div>
                </Show>
              </div>

              <div class="card">
                <h2>
                  Contesto <span class="r">/props</span>
                </h2>
                <dl class="kv">
                  <dt>Dichiarato</dt>
                  <dd class="num">{num(r().ctx_declared)}</dd>
                  <dt>Servito</dt>
                  <dd>
                    <Show when={ready()} fallback={<Val v={null} />}>
                      {(rd) => (
                        <>
                          <Val v={num(rd().ctx_served)} />{" "}
                          <Show when={rd().ctx_served === r().ctx_declared}>
                            <span class="badge ok tight">
                              <i />
                              coincide
                            </span>
                          </Show>
                        </>
                      )}
                    </Show>
                  </dd>
                  <dt>n_parallel</dt>
                  <dd class="num">{r().n_parallel}</dd>
                  <dt>Alias servito</dt>
                  <dd class="mono">
                    <Val v={ready()?.alias_served} />
                  </dd>
                  <dt>Build</dt>
                  <dd class="mono">{r().build}</dd>
                  <dt>Indirizzo</dt>
                  <dd class="mono">{r().base_url}</dd>
                </dl>
              </div>
            </div>

            <For each={ready()?.degraded ?? []}>
              {(msg) => <div class="note warn mb">Degradato: {msg}. «Riavvia con la stessa riga» quando il motore è libero.</div>}
            </For>
            <For each={ready()?.divergences ?? []}>
              {(msg) => <div class="note warn mb">{msg} — si registra, non si corregge</div>}
            </For>
            <Show when={ready()?.conditions_changed.length}>
              <div class="note warn mb">
                <b>Condizioni cambiate dall'avvio precedente:</b> {ready()!.conditions_changed.join(" · ")}. La soglia
                «degradato» non confronta questo avvio con quelli di prima:{" "}
                <Show
                  when={ready()!.reference}
                  fallback="la mediana di riferimento riparte da qui (nessun avvio con queste condizioni finora)."
                >
                  {(ref) => (
                    <>
                      la mediana di riferimento riparte ({fixed(ref().decode_median, 1)} tok/s, {ref().runs}{" "}
                      {ref().runs === 1 ? "avvio" : "avvii"} con queste condizioni finora).
                    </>
                  )}
                </Show>
              </div>
            </Show>

            <Show when={ready()}>{(rd) => <TurnsCard summary={rd().telemetry} />}</Show>

            <div class="grid g2 mb" style={{ "align-items": "start" }}>
              <div class="grid" style={{ "align-items": "start" }}>
                <ConditionsCard conditions={ready()?.conditions ?? null} changed={ready()?.conditions_changed ?? []} />
                <MemoryCard memory={ready()?.memory ?? null} loadMs={ready()?.load_ms ?? null} />
              </div>

              <div class="card">
                <h2>
                  Riga di comando <span class="r">esatta, come lanciata</span>
                </h2>
                <pre class="cmd">{r().command_line}</pre>
                <div class="row" style={{ "margin-top": "8px" }}>
                  <button class="btn sm" onClick={() => copy(r().command_line)}>
                    Copia riga
                  </button>
                  <button class="btn sm" onClick={() => copy(r().manifest_path)} title={r().manifest_path}>
                    Copia percorso manifest
                  </button>
                  <button class="btn sm" disabled={!api.isEngineOn(s())} onClick={copyClient}>
                    Riga per i client
                  </button>
                  <span class="right" />
                  <button
                    class="btn sm"
                    disabled={!canRestart()}
                    title={inUse() ? `In uso: ${usage()!.reasons.join(", ")}` : "Stesso profilo e stesse differenze, nuovo avvio"}
                    onClick={() => act(() => api.engineRestart())}
                  >
                    Riavvia con la stessa riga
                  </button>
                  <button
                    class="btn sm danger"
                    disabled={busy() || !api.isEngineOn(s()) || inUse()}
                    title={inUse() ? `In uso: ${usage()!.reasons.join(", ")}` : ""}
                    onClick={() => act(() => api.engineStop())}
                  >
                    Ferma
                  </button>
                </div>
                <Show when={inUse()}>
                  <div class="note busy" style={{ "margin-top": "8px" }}>
                    Arresto e riavvio sono rifiutati finché lo stato è <b>IN USO</b> ({usage()!.reasons.join(", ")}):
                    riavviare svuota la cache del prefisso del client che sta lavorando.
                  </div>
                </Show>
                <Show when={r().overrides.length}>
                  <hr />
                  <h2>
                    Differenze dal profilo{" "}
                    <Show when={r().invalidates_cache}>
                      <span class="pill cache">invalida la cache</span>
                    </Show>
                  </h2>
                  <table>
                    <tbody>
                      <For each={r().overrides}>
                        {(o) => (
                          <tr>
                            <td class="mono">{o.field}</td>
                            <td class="mono">
                              <s>{o.base ?? "—"}</s>
                            </td>
                            <td class="mono">{o.value ?? "—"}</td>
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
                Log <span class="r mono">{r().log_path}</span>
              </h2>
              <pre class="log">{log() || "…"}</pre>
            </div>
          </>
        )}
      </Show>
    </section>
  );
}
