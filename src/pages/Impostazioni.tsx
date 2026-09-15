import { open } from "@tauri-apps/plugin-dialog";
import { createEffect, createSignal, For, Show } from "solid-js";
import * as api from "../api";
import type { ExitBehavior, MachineConfig, Overview } from "../api";
import { Val } from "../components";

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

export default function Impostazioni(props: { overview: Overview; onChange: (o: Overview) => void }) {
  const [machine, setMachine] = createSignal<MachineConfig | null>(null);
  const [message, setMessage] = createSignal<{ kind: "ok" | "err"; text: string } | null>(null);

  createEffect(() => setMachine(props.overview.machine ? structuredClone(props.overview.machine) : null));

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
        <div class="note err mb">{props.overview.machine_error}</div>
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
            </dl>
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
          </div>
        </div>
      </div>
    </section>
  );
}
