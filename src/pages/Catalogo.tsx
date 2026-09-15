import { open } from "@tauri-apps/plugin-dialog";
import { createMemo, createSignal, For, onCleanup, onMount, Show } from "solid-js";
import * as api from "../api";
import type { AdoptPlan, BuildsView, Device, Estimate, ModelRow, TaskView } from "../api";
import { Confirm, Val } from "../components";
import { clock, fixed, num, show } from "../format";

const STATES: Record<api.ModelState, { cls: string; text: string }> = {
  verified: { cls: "ok", text: "verificato" },
  present: { cls: "warn", text: "presente" },
  mismatch: { cls: "err", text: "hash diverso" },
  downloading: { cls: "acc", text: "in download" },
  downloadable: { cls: "", text: "scaricabile" },
  missing: { cls: "", text: "mancante" },
};

const gb = (bytes: number | null | undefined) => (bytes == null ? null : fixed(bytes / 1e9, 2));

/** Hash abbreviato come nel mockup: b46fedd3…9064772. */
function shortHash(h: string | null): string | null {
  if (!h) return null;
  const s = h.replace(/^sha256:/i, "").toLowerCase();
  return s.length === 64 ? `${s.slice(0, 8)}…${s.slice(57)}` : s;
}

function EstimateCell(props: { row: ModelRow }) {
  const e = () => props.row.estimate;
  const title = () => {
    const x = e();
    if (!x) return "";
    const part = (label: string, v: number | null) => (v == null ? `${label}: sconosciuto` : `${label}: ${fixed(v / 1e9, 2)} GB`);
    return [
      part("pesi", x.weights_bytes),
      part("cache KV", x.kv_bytes),
      part("stato ricorrente", x.state_bytes),
      x.compute_bytes == null
        ? "buffer di calcolo: sconosciuto"
        : `buffer di calcolo: ${fixed(x.compute_bytes / 1e9, 2)} GB (misurato su ${x.compute_from})`,
      ...x.notes,
    ].join("\n");
  };
  return (
    <Show when={e()} fallback={<span class="cond">—</span>}>
      <span class="num" title={title()}>
        {e()!.total_is_lower_bound ? "≥ " : "~ "}
        {gb(e()!.total_bytes)}
        <Show when={props.row.estimate_ctx}>
          <span class="cond"> @{num(props.row.estimate_ctx)}</span>
        </Show>
      </span>
    </Show>
  );
}

function EstimateCard(props: { row: ModelRow }) {
  const e = (): Estimate | null => props.row.estimate;
  const line = (label: string, v: number | null, cond?: string) => (
    <>
      <dt>{label}</dt>
      <dd>
        <Val v={gb(v)} unit="GB" />
        <Show when={cond}>
          <span class="cond"> {cond}</span>
        </Show>
      </dd>
    </>
  );
  return (
    <Show when={e()} fallback={<div class="cond">Nessun profilo usa questi pesi: senza un contesto dichiarato una stima sarebbe inventata.</div>}>
      <dl class="kv">
        {line("Pesi", e()!.weights_bytes, "dal file, misurato")}
        {line("Cache KV", e()!.kv_bytes, `${num(props.row.estimate_ctx)} token · ${num(e()!.full_attention_blocks)} blocchi ad attenzione piena`)}
        {line("Stato ricorrente", e()!.state_bytes, "non cresce col contesto")}
        {line("Buffer di calcolo", e()!.compute_bytes, e()!.compute_from ? `misurato su ${e()!.compute_from}` : undefined)}
        <dt>{e()!.total_is_lower_bound ? "Totale minimo" : "Totale stimato"}</dt>
        <dd class="num">
          <b>
            {e()!.total_is_lower_bound ? "≥ " : "~ "}
            {gb(e()!.total_bytes)} GB
          </b>
        </dd>
      </dl>
      <For each={e()!.notes}>
        {(n) => (
          <div class="note warn" style={{ "margin-top": "6px" }}>
            {n}
          </div>
        )}
      </For>
      <div class="cond" style={{ "margin-top": "6px" }}>
        Stima dai metadati del GGUF, sostituita dalla misura dopo il caricamento.
      </div>
    </Show>
  );
}

function Tasks(props: { tasks: TaskView[]; onCancel: (id: string) => void; onClear: () => void }) {
  const label = (t: TaskView) =>
    t.kind === "verify"
      ? "verifica SHA-256"
      : t.kind === "download"
        ? "download"
        : t.kind === "copy"
          ? "copia da un altro volume"
          : "installazione";
  return (
    <Show when={props.tasks.length}>
      <div class="card mb">
        <h2>
          Lavori in corso
          <button class="btn sm right" onClick={props.onClear}>
            Togli i finiti
          </button>
        </h2>
        <For each={props.tasks}>
          {(t) => (
            <div class="row" style={{ "margin-bottom": "6px" }}>
              <span class="mono">{t.target}</span>
              <span class="pill">{label(t)}</span>
              <Show when={t.state === "running"} fallback={<span class={`badge ${t.state === "failed" ? "err" : t.state === "cancelled" ? "warn" : "ok"} tight`}><i />{t.state === "done" ? "fatto" : t.state === "failed" ? "fallito" : "annullato"}</span>}>
                <span style={{ flex: "1", "min-width": "140px" }}>
                  <div class="bar">
                    <span style={{ width: `${t.total ? Math.min(100, (t.done / t.total) * 100) : 0}%` }} />
                  </div>
                </span>
                <span class="num cond">
                  {gb(t.done)} / {t.total ? `${gb(t.total)} GB` : "?"}
                </span>
                <button class="btn sm" onClick={() => props.onCancel(t.id)}>
                  Ferma
                </button>
              </Show>
              <Show when={t.message}>
                <span class="cond">{t.message}</span>
              </Show>
            </div>
          )}
        </For>
      </div>
    </Show>
  );
}

function BuildsTab(props: { onError: (e: string) => void; onMessage: (m: string) => void }) {
  const [view, setView] = createSignal<BuildsView | null>(null);
  const [devices, setDevices] = createSignal<{ id: string; list: Device[] } | null>(null);
  const [busy, setBusy] = createSignal(false);

  const load = async (refresh: boolean) => {
    setBusy(true);
    try {
      setView(await api.buildsList(refresh));
    } catch (e) {
      props.onError(String(e));
    } finally {
      setBusy(false);
    }
  };
  onMount(() => load(false));

  const install = async (tag: string, backend: string) => {
    try {
      await api.buildsInstall(tag, backend);
    } catch (e) {
      props.onError(String(e));
    }
  };

  /** Gemello di «Aggiungi build…» delle Impostazioni: una cartella di llama.cpp già scaricata. */
  const importDir = async () => {
    const picked = await open({ directory: true });
    if (typeof picked !== "string") return;
    setBusy(true);
    try {
      setView(await api.buildsImportDir(picked));
      props.onMessage(`Cartella dichiarata in machine.toml: ${picked}`);
    } catch (e) {
      props.onError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const showDevices = async (id: string) => {
    try {
      setDevices({ id, list: await api.buildDevices(id) });
    } catch (e) {
      props.onError(String(e));
    }
  };

  return (
    <>
      <div class="row mb">
        <button class="btn sm primary" disabled={busy()} onClick={() => load(true)}>
          Cerca le release di ggml-org
        </button>
        <button class="btn sm" disabled={busy()} onClick={importDir}>
          Importa cartella…
        </button>
        <span class="right cond">Cambiare build è una variabile di prova, non un aggiornamento automatico.</span>
      </div>

      <h2>Installate</h2>
      <table class="mb">
        <thead>
          <tr>
            <th>Id</th>
            <th>Cartella</th>
            <th>Origine</th>
            <th />
          </tr>
        </thead>
        <tbody>
          <For each={view()?.installed ?? []} fallback={<tr><td colspan={4} class="cond">Nessuna build installata.</td></tr>}>
            {(b) => (
              <tr>
                <td class="mono">{b.id}</td>
                <td class="mono mini">{b.dir}</td>
                <td class="cond">{b.source}</td>
                <td>
                  <button class="btn sm" onClick={() => showDevices(b.id)}>
                    --list-devices
                  </button>
                </td>
              </tr>
            )}
          </For>
        </tbody>
      </table>

      <Show when={devices()}>
        {(d) => (
          <div class="card mb">
            <h2>
              Dispositivi <span class="r mono">{d().id}</span>
            </h2>
            <pre class="log">
              {d().list.map((x) => `${x.id}: ${x.name} (${x.total_mib} MiB, ${x.free_mib} MiB free)`).join("\n") ||
                "nessun dispositivo elencato"}
            </pre>
          </div>
        )}
      </Show>

      <Show when={view()?.releases_error}>
        <div class="note err mb">{view()!.releases_error}</div>
      </Show>
      <Show when={view()?.releases.length}>
        <h2>Scaricabili</h2>
        <div class="cond mb">
          Le build escono come prerelease con tag <span class="mono">b&lt;numero&gt;</span>; ogni pacchetto porta il
          proprio digest SHA-256, verificato prima di estrarlo.
        </div>
        <table>
          <thead>
            <tr>
              <th>Tag</th>
              <th>Pubblicata</th>
              <th>Backend</th>
              <th class="r">MB</th>
              <th>Digest</th>
              <th />
            </tr>
          </thead>
          <tbody>
            <For each={view()!.releases.slice(0, 4)}>
              {(r) => (
                <For each={r.assets.filter((a) => a.backend.startsWith("win-"))}>
                  {(a) => (
                    <tr>
                      <td class="mono">{r.tag}</td>
                      <td class="num mini">{r.published ? clock(r.published) : "—"}</td>
                      <td class="mono">{a.backend}</td>
                      <td class="r num">{fixed(a.size / 1e6, 0)}</td>
                      <td class="mono mini">{shortHash(a.digest) ?? <span class="unk">assente</span>}</td>
                      <td>
                        <button class="btn sm" onClick={() => install(r.tag, a.backend)}>
                          Installa
                        </button>
                      </td>
                    </tr>
                  )}
                </For>
              )}
            </For>
          </tbody>
        </table>
      </Show>
    </>
  );
}

export default function Catalogo(props: { onLaunch?: (file: string) => void }) {
  const [tab, setTab] = createSignal<"modelli" | "build">("modelli");
  const [rows, setRows] = createSignal<ModelRow[]>([]);
  const [tasks, setTasks] = createSignal<TaskView[]>([]);
  const [focus, setFocus] = createSignal<string | null>(null);
  const [error, setError] = createSignal<string | null>(null);
  const [message, setMessage] = createSignal<string | null>(null);
  const [repo, setRepo] = createSignal("");
  const [file, setFile] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [adopt, setAdopt] = createSignal<AdoptPlan | null>(null);

  const load = async () => {
    try {
      setRows(await api.catalogList());
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  };

  onMount(() => {
    load();
    const poll = async () => {
      try {
        const list = await api.tasksList();
        const before = tasks();
        setTasks(list);
        // Quando un lavoro finisce, il catalogo ha nuovi hash o nuovi file: si rilegge.
        if (before.some((t) => t.state === "running") && list.every((t) => t.state !== "running")) load();
      } catch {
        /* il prossimo giro riprova */
      }
    };
    poll();
    const t = setInterval(poll, 1000);
    onCleanup(() => clearInterval(t));
  });

  const act = async (fn: () => Promise<unknown>) => {
    setBusy(true);
    setError(null);
    try {
      await fn();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const selected = createMemo(() => rows().find((r) => r.id === focus()) ?? null);

  /**
   * Registra un `.gguf` che sta fuori dalla cartella dei pesi. Sullo stesso volume è un hard link
   * e non costa niente; da un altro volume è una copia, e allora si chiede prima.
   */
  const importFromDisk = () =>
    act(async () => {
      const picked = await open({ multiple: false, filters: [{ name: "Pesi GGUF", extensions: ["gguf"] }] });
      if (typeof picked !== "string") return;
      const plan = await api.catalogImportPlan(picked);
      if (plan.action === "copy" && !plan.blocker) {
        setAdopt(plan);
        return;
      }
      const out = await api.catalogImport(picked, false);
      if (out.rows.length) setRows(out.rows);
      setMessage(out.message);
    });

  const confirmCopy = () =>
    act(async () => {
      const plan = adopt()!;
      setAdopt(null);
      const out = await api.catalogImport(plan.source, true);
      if (out.rows.length) setRows(out.rows);
      setMessage(out.message);
    });

  /** Campionamento consigliato dalla model card del publisher, con fonte e data di lettura. */
  const readCard = (row: ModelRow, replace: boolean) =>
    act(async () => {
      const r = await api.catalogModelcard(row.id, replace);
      setRows(r.rows);
      setMessage(r.notes.join(" · "));
    });

  const remove = (row: ModelRow) =>
    act(async () => {
      const plan = await api.catalogRemovalPlan(row.id);
      const warn = plan.warnings.length ? `\n\n${plan.warnings.join("\n")}` : "";
      const deleteFile = plan.path != null && confirm(`Cancellare anche il file ${plan.path}?${warn}`);
      if (deleteFile && plan.warnings.length && !confirm("Confermi comunque la cancellazione?")) return;
      setRows(await api.catalogRemove(row.id, deleteFile, true));
      setMessage(`«${row.id}» tolto dal catalogo${deleteFile ? " e cancellato dal disco" : ""}.`);
    });

  return (
    <section>
      <h1>Catalogo</h1>
      <p class="sub">
        Pesi e build presenti su questa macchina, verificati per hash. Un file già presente viene riconosciuto per nome
        e hash, anche quando è un hard link creato da un altro strumento.
      </p>

      <Show when={error()}>
        <div class="note err mb row">
          {error()}
          <button class="btn sm right" onClick={() => setError(null)}>
            Chiudi
          </button>
        </div>
      </Show>
      <Show when={message()}>
        <div class="note mb row">
          {message()}
          <button class="btn sm right" onClick={() => setMessage(null)}>
            Chiudi
          </button>
        </div>
      </Show>

      <div class="tabs">
        <button classList={{ on: tab() === "modelli" }} onClick={() => setTab("modelli")}>
          Modelli
        </button>
        <button classList={{ on: tab() === "build" }} onClick={() => setTab("build")}>
          Build llama.cpp
        </button>
      </div>

      <Tasks
        tasks={tasks()}
        onCancel={(id) => act(() => api.taskCancel(id))}
        onClear={() => act(async () => { await api.tasksClear(); setTasks(await api.tasksList()); })}
      />

      <Show when={tab() === "modelli"} fallback={<BuildsTab onError={setError} onMessage={setMessage} />}>
        <div class="row mb">
          <input
            class="mono"
            style={{ width: "260px", background: "var(--bg)", color: "var(--fg)", border: "1px solid var(--line)", "border-radius": "3px", padding: "4px 6px" }}
            placeholder="bartowski/Qwen_…-GGUF"
            value={repo()}
            onInput={(e) => setRepo(e.currentTarget.value)}
          />
          <input
            class="mono"
            style={{ width: "260px", background: "var(--bg)", color: "var(--fg)", border: "1px solid var(--line)", "border-radius": "3px", padding: "4px 6px" }}
            placeholder="Nome-Q4_K_M.gguf"
            value={file()}
            onInput={(e) => setFile(e.currentTarget.value)}
          />
          <button
            class="btn sm primary"
            disabled={busy() || !repo().trim() || !file().trim()}
            onClick={() =>
              act(async () => {
                setRows(await api.catalogAdd(repo().trim(), file().trim()));
                setMessage(`«${file().trim()}» aggiunto: oid LFS e dimensione letti dal publisher.`);
                setRepo("");
                setFile("");
              })
            }
          >
            Aggiungi da Hugging Face
          </button>
          <button class="btn sm" disabled={busy()} onClick={importFromDisk}>
            Importa da disco…
          </button>
          <button class="btn sm" disabled={busy()} onClick={load}>
            Riscansiona
          </button>
          <span class="right legend">
            <span>
              <i style={{ background: "var(--ok)" }} />
              verificato
            </span>
            <span>
              <i style={{ background: "var(--warn)" }} />
              presente, da verificare
            </span>
            <span>
              <i style={{ background: "var(--acc)" }} />
              in download
            </span>
            <span>
              <i style={{ background: "var(--fg3)" }} />
              scaricabile
            </span>
          </span>
        </div>

        <div style={{ "overflow-x": "auto" }}>
          <table>
            <thead>
              <tr>
                <th>Modello</th>
                <th>Publisher · file</th>
                <th>Quant</th>
                <th class="r">GB</th>
                <th>SHA-256</th>
                <th>Arch · MTP · ctx train</th>
                <th class="r">Stima</th>
                <th>Stato</th>
                <th />
              </tr>
            </thead>
            <tbody>
              <For each={rows()} fallback={<tr><td colspan={9} class="cond">Nessun modello: aggiungine uno da Hugging Face o metti un .gguf nella cartella dei pesi.</td></tr>}>
                {(r) => (
                  <tr class="click" classList={{ sel: focus() === r.id }} onClick={() => setFocus(r.id)}>
                    <td class="mono">{r.id}</td>
                    <td class="mono mini">
                      {r.repo ? `${r.repo} · ` : ""}
                      {r.file}
                    </td>
                    <td>{show(r.quant ?? r.info?.dominant_type)}</td>
                    <td class="r num">
                      <Val v={gb(r.size_bytes) ?? (r.size_gb != null ? fixed(r.size_gb, 2) : null)} />
                    </td>
                    <td class="mono mini">
                      <Show when={r.sha256_verified ?? r.sha256} fallback={<span class="unk">—</span>}>
                        <span class={`badge ${r.state === "verified" ? "ok" : r.state === "mismatch" ? "err" : "warn"} tight`}>
                          <i />
                          {shortHash(r.sha256_verified ?? r.sha256)}
                        </span>
                      </Show>
                    </td>
                    <td class="mini">
                      <Show when={r.info} fallback={<span class="unk cond">—</span>}>
                        {r.info!.arch} · MTP {show(r.info!.mtp_layers)} · {num(r.info!.context_train)}
                      </Show>
                    </td>
                    <td class="r">
                      <EstimateCell row={r} />
                    </td>
                    <td>
                      <span class={`badge ${STATES[r.state].cls}`}>
                        <i />
                        {STATES[r.state].text}
                      </span>
                      <Show when={r.part_bytes}>
                        <div class="cond">{gb(r.part_bytes)} GB già scaricati</div>
                      </Show>
                      <Show when={(r.hard_links ?? 1) > 1}>
                        <span class="pill" title="Lo stesso file è collegato da un altro strumento">
                          hard link ×{r.hard_links}
                        </span>
                      </Show>
                    </td>
                    <td onClick={(e) => e.stopPropagation()}>
                      <Show when={r.state === "downloadable" || r.state === "downloading"}>
                        <button class="btn sm" disabled={busy()} onClick={() => act(() => api.catalogDownload(r.id))}>
                          {r.state === "downloading" ? "Riprendi" : "Scarica"}
                        </button>
                      </Show>
                      <Show when={r.size_bytes != null}>
                        <button class="btn sm" disabled={busy()} onClick={() => act(() => api.catalogVerify(r.id))}>
                          Verifica
                        </button>
                        <button
                          class="btn sm primary"
                          title="Apre la pagina Avvio con questi pesi: il profilo che li usa, o uno nuovo"
                          onClick={() => props.onLaunch?.(r.file)}
                        >
                          Avvia…
                        </button>
                      </Show>
                    </td>
                  </tr>
                )}
              </For>
            </tbody>
          </table>
        </div>

        <Show when={selected()}>
          {(r) => (
            <div class="grid g2" style={{ "margin-top": "12px", "align-items": "start" }}>
              <div class="card">
                <h2>
                  {r().id} <span class="r">dettaglio</span>
                </h2>
                <dl class="kv">
                  <dt>Percorso</dt>
                  <dd class="mono">{show(r().path)}</dd>
                  <dt>Sorgente</dt>
                  <dd class="mono">{r().repo ? `huggingface.co/${r().repo} · resolve/main` : "—"}</dd>
                  <dt>SHA-256</dt>
                  <dd class="mono">
                    <Val v={shortHash(r().sha256_verified ?? r().sha256)} />
                    <Show when={r().verified_at}>
                      <span class="cond"> · verificato {clock(r().verified_at!)}</span>
                    </Show>
                    <Show when={!r().verified_at && r().sha256}>
                      <span class="cond"> · dichiarato, non ricalcolato</span>
                    </Show>
                  </dd>
                  <dt>Dimensione</dt>
                  <dd class="num">
                    <Val v={gb(r().size_bytes)} unit="GB" />
                    <span class="cond"> decimali, come il publisher</span>
                  </dd>
                  <dt>Tensori MTP</dt>
                  <dd class="mono">
                    <Show when={r().info?.mtp_types.length} fallback={<span class="unk">—</span>}>
                      {r().info!.mtp_types.join(" · ")}
                      <span class="cond"> in {num(r().info!.mtp_layers)} layer</span>
                    </Show>
                  </dd>
                  <dt>Architettura</dt>
                  <dd class="mono">
                    <Show when={r().info} fallback={<span class="unk">—</span>}>
                      {r().info!.arch} · {num(r().info!.block_count)} blocchi · {num(r().info!.expert_count)} esperti /{" "}
                      {num(r().info!.expert_used_count)} attivi
                      <Show when={r().info!.full_attention_interval}>
                        <span> · attenzione piena ogni {r().info!.full_attention_interval}</span>
                      </Show>
                    </Show>
                  </dd>
                  <dt>Collegamenti</dt>
                  <dd>
                    <Val v={r().hard_links} />
                    <span class="cond">
                      {" "}
                      {(r().hard_links ?? 1) > 1 ? "lo stesso file è usato da un altro strumento" : "nessun altro hard link"}
                    </span>
                  </dd>
                  <dt>Profili che lo usano</dt>
                  <dd class="mono">{r().profiles.length ? r().profiles.join(" · ") : "—"}</dd>
                </dl>
                <div class="row" style={{ "margin-top": "8px" }}>
                  <button class="btn sm" disabled={busy() || r().size_bytes == null} onClick={() => act(() => api.catalogVerify(r().id))}>
                    Ricalcola SHA-256
                  </button>
                  <button class="btn sm" disabled={busy()} onClick={() => act(async () => setRows(await api.catalogReread(r().id)))}>
                    Rileggi metadati GGUF
                  </button>
                  <button
                    class="btn sm"
                    disabled={busy() || !r().repo}
                    title={r().repo ? "Legge il README del publisher" : "Senza repository non c'è una model card"}
                    onClick={() => readCard(r(), false)}
                  >
                    Leggi la model card
                  </button>
                  <button class="btn sm danger" disabled={busy()} onClick={() => remove(r())}>
                    Rimuovi…
                  </button>
                </div>
              </div>

              <div class="card">
                <h2>
                  Stima di memoria <span class="r">{r().estimate_profile ? `profilo ${r().estimate_profile}` : "senza profilo"}</span>
                </h2>
                <EstimateCard row={r()} />
                <Show
                  when={Object.keys(r().sampling_by_mode).length}
                  fallback={
                    <>
                      <hr />
                      <div class="cond">
                        Nessun campionamento consigliato registrato. «Leggi la model card» prende dal publisher quello
                        che ci scrive, con fonte e data; da qui lo si porta in un profilo e nelle righe per i client.
                      </div>
                    </>
                  }
                >
                  <hr />
                  <h2>
                    Campionamento consigliato <span class="r">dalla model card</span>
                  </h2>
                  <table style={{ "font-size": "12px" }}>
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
                      <For each={Object.entries(r().sampling_by_mode)}>
                        {([mode, s]) => (
                          <tr>
                            <td>{mode}</td>
                            <td class="r num">{show(s.temperature)}</td>
                            <td class="r num">{show(s.top_p)}</td>
                            <td class="r num">{show(s.top_k)}</td>
                            <td class="r num">{show(s.min_p)}</td>
                            <td class="r num">{show(s.presence_penalty)}</td>
                            <td class="mini">
                              {show(s.source)}
                              {s.verified ? ` · ${s.verified}` : ""}
                            </td>
                          </tr>
                        )}
                      </For>
                    </tbody>
                  </table>
                  <div class="note" style={{ "margin-top": "8px" }}>
                    Dato del modello, non un default del server: il campionamento lo manda il client in ogni richiesta.
                  </div>
                </Show>
              </div>
            </div>
          )}
        </Show>
      </Show>

      <Show when={adopt()}>
        {(pl) => (
          <Confirm
            title={`Copiare «${pl().file}»?`}
            lines={[
              ...pl().notes,
              `Da copiare: ${(pl().bytes / 1e9).toFixed(2)} GB${
                pl().free_disk != null ? ` · liberi sul volume di destinazione: ${(pl().free_disk! / 1e9).toFixed(1)} GB` : ""
              }`,
              `Destinazione: ${pl().target}`,
            ]}
            confirmLabel="Copia"
            onCancel={() => setAdopt(null)}
            onConfirm={confirmCopy}
          />
        )}
      </Show>
    </section>
  );
}
