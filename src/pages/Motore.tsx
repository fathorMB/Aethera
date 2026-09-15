import { createSignal, For, onCleanup, onMount, Show } from "solid-js";
import * as api from "../api";
import type { EngineStatus, MemorySection, Orphan, ReadyStatus } from "../api";
import { copy, Spark, StateBadge, Toggle, UsageBadge, Val } from "../components";
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

export default function Motore(props: { status: EngineStatus | null; onGo: (page: "avvio") => void }) {
  const [log, setLog] = createSignal("");
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
    };
    tick();
    const t = setInterval(tick, 2000);
    onCleanup(() => clearInterval(t));
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

      <Show when={s()?.state === "orphan" && s()}>
        {(o) => <OrphanCard orphan={(o() as Extract<EngineStatus, { state: "orphan" }>).orphan} />}
      </Show>

      <Show
        when={run()}
        fallback={
          <Show when={s()?.state !== "orphan"}>
            <div class="card">
              <h2>Stato</h2>
              <div class="row">
                <StateBadge status={s()} />
                <span class="cond">nessun avvio in questa sessione</span>
                <button class="btn sm right" onClick={() => props.onGo("avvio")}>
                  Vai ad Avvio
                </button>
              </div>
            </div>
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
                  Cache del prefisso <span class="r">ultimi {ready()?.telemetry.window ?? 0} turni</span>
                </h2>
                <div class="big">
                  <Val v={pct(ready()?.telemetry.cache_share)} />
                  <small>% riusato</small>
                </div>
                <Show when={ready()?.telemetry.cache_series.length} fallback={<div class="cond">nessuna richiesta servita</div>}>
                  <div style={{ margin: "8px 0 4px" }}>
                    <Spark values={ready()!.telemetry.cache_series} max={1} low={0.5} />
                  </div>
                  <div class="cond">
                    Quota di token di prompt dalla cache per richiesta (/metrics). Una barra rossa è sotto il 50 %: il
                    client ha riscritto il prefisso o qualcosa ha invalidato la cache. Tratteggio = non attribuibile.
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

            <div class="grid g2 mb" style={{ "align-items": "start" }}>
              <MemoryCard memory={ready()?.memory ?? null} loadMs={ready()?.load_ms ?? null} />

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
