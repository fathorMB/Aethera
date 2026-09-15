import { createSignal, For, onCleanup, onMount, Show } from "solid-js";
import * as api from "../api";
import type { EngineStatus } from "../api";
import { copy, StateBadge, Val } from "../components";
import { clock, duration, num } from "../format";

export default function Motore(props: { status: EngineStatus | null; onGo: (page: "avvio") => void }) {
  const [log, setLog] = createSignal("");
  const [error, setError] = createSignal<string | null>(null);
  const run = () => api.runOf(props.status);
  const s = () => props.status;

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

  const stop = async () => {
    setError(null);
    try {
      await api.engineStop();
    } catch (e) {
      setError(String(e));
    }
  };

  const clientSnippet = () => {
    const r = run();
    if (!r) return "";
    return `[backend]\nbase_url = "${r.base_url}/v1"\n\n[model]\nexpected = "${r.profile}"`;
  };

  return (
    <section>
      <h1>Motore</h1>
      <p class="sub">
        Un processo <code>llama-server</code>, avviato e fermato da Aethera. Quello che non è misurato è «sconosciuto».
      </p>

      <Show when={error()}>
        <div class="note err mb">{error()}</div>
      </Show>

      <Show
        when={run()}
        fallback={
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
        }
      >
        {(r) => (
          <>
            <div class="grid g3 mb">
              <div class="card">
                <h2>
                  Stato <span class="r">/health</span>
                </h2>
                <div class="row">
                  <StateBadge status={s()} />
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
                  <Show when={s()?.state === "ready" && s()}>
                    {(x) => {
                      const rd = x() as Extract<EngineStatus, { state: "ready" }>;
                      return (
                        <>
                          <dt>Acceso da</dt>
                          <dd class="num">
                            {duration(rd.uptime_s)} <span class="cond">dal {clock(r().started_at)}</span>
                          </dd>
                          <dt>Pronto in</dt>
                          <dd class="num">{(rd.load_ms / 1000).toFixed(1).replace(".", ",")} s</dd>
                        </>
                      );
                    }}
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
                  <Show when={s()?.state === "off" && s()}>
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
                  <dt>Avvio</dt>
                  <dd class="mono">{r().run_id}</dd>
                  <dt>PID</dt>
                  <dd class="num">{r().pid}</dd>
                </dl>
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
                    <Show when={s()?.state === "ready" && s()} fallback={<Val v={null} />}>
                      {(x) => {
                        const rd = x() as Extract<EngineStatus, { state: "ready" }>;
                        return (
                          <>
                            <Val v={num(rd.ctx_served)} />{" "}
                            <Show when={rd.ctx_served === r().ctx_declared}>
                              <span class="badge ok tight">
                                <i />
                                coincide
                              </span>
                            </Show>
                          </>
                        );
                      }}
                    </Show>
                  </dd>
                  <dt>Alias servito</dt>
                  <dd class="mono">
                    <Show when={s()?.state === "ready" && s()} fallback={<Val v={null} />}>
                      {(x) => <Val v={(x() as Extract<EngineStatus, { state: "ready" }>).alias_served} />}
                    </Show>
                  </dd>
                  <dt>Build</dt>
                  <dd class="mono">{r().build}</dd>
                  <dt>Indirizzo</dt>
                  <dd class="mono">{r().base_url}</dd>
                </dl>
                <Show when={s()?.state === "ready" && (s() as Extract<EngineStatus, { state: "ready" }>).divergences}>
                  {(d) => (
                    <For each={d()}>{(msg) => <div class="note warn" style={{ "margin-top": "6px" }}>{msg} — si registra, non si corregge</div>}</For>
                  )}
                </Show>
              </div>

              <div class="card">
                <h2>
                  Memoria e velocità <span class="r">M-03</span>
                </h2>
                <dl class="kv">
                  <dt>VRAM misurata</dt>
                  <dd>
                    <Val v={null} />
                  </dd>
                  <dt>Prefill · decode</dt>
                  <dd>
                    <Val v={null} />
                  </dd>
                  <dt>Cache del prefisso</dt>
                  <dd>
                    <Val v={null} />
                  </dd>
                </dl>
                <div class="cond" style={{ "margin-top": "6px" }}>
                  Memoria misurata e telemetria arrivano con il milestone successivo.
                </div>
              </div>
            </div>

            <div class="grid g2 mb">
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
                  <span class="right" />
                  <button class="btn sm danger" disabled={!api.isEngineOn(s())} onClick={stop}>
                    Ferma
                  </button>
                </div>
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

              <div class="card">
                <h2>
                  Riga per i client <span class="r">Nonio · profile.toml</span>
                </h2>
                <pre class="cmd">{clientSnippet()}</pre>
                <div class="row" style={{ "margin-top": "8px" }}>
                  <button class="btn sm" onClick={() => copy(clientSnippet())}>
                    Copia TOML
                  </button>
                </div>
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
