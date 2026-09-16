import { open } from "@tauri-apps/plugin-dialog";
import { createEffect, createMemo, createSignal, For, onCleanup, onMount, Show } from "solid-js";
import * as api from "../api";
import type { AdoptPlan, BuildsView, Device, EngineStatus, Estimate, ModelRow, TaskView } from "../api";
import { AskName, Confirm, Dialog, Empty, Unknown, Val } from "../components";
import { clock, fixed, num, show } from "../format";
import { Chips, Head, Q, Stack } from "../ui";
import { countBy } from "../ui-logic";

const STATES: Record<api.ModelState, { cls: string; text: string; chip: string }> = {
  verified: { cls: "ok", text: "verificato", chip: "verificati" },
  present: { cls: "warn", text: "presente", chip: "presenti, da verificare" },
  mismatch: { cls: "err", text: "hash diverso", chip: "hash diverso" },
  downloading: { cls: "acc", text: "in download", chip: "in download" },
  downloadable: { cls: "", text: "scaricabile", chip: "scaricabili" },
  missing: { cls: "", text: "mancante", chip: "mancanti" },
};
const STATE_ORDER: api.ModelState[] = ["verified", "present", "mismatch", "downloading", "downloadable", "missing"];

const gb = (bytes: number | null | undefined) => (bytes == null ? null : fixed(bytes / 1e9, 2));
const GIB = 1024 ** 3;

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

function EstimateCard(props: { row: ModelRow; vgm: number | null }) {
  const e = (): Estimate | null => props.row.estimate;
  const toGib = (b: number | null) => (b == null ? null : b / GIB);
  const line = (label: string, v: number | null, cond?: string, sw?: string) => (
    <>
      <dt>
        <Show when={sw}>
          <i class="sw" style={{ background: sw }} />{" "}
        </Show>
        {label}
      </dt>
      <dd>
        <Val v={gb(v)} unit="GB" />
        <Show when={cond}>
          <span class="cond"> {cond}</span>
        </Show>
      </dd>
    </>
  );
  const other = () => {
    const x = e();
    return x == null || (x.state_bytes == null && x.compute_bytes == null) ? null : (x.state_bytes ?? 0) + (x.compute_bytes ?? 0);
  };
  return (
    <Show when={e()} fallback={<div class="cond">Nessun profilo usa questi pesi: senza un contesto dichiarato una stima sarebbe inventata.</div>}>
      <Stack
        legend={false}
        total={props.vgm}
        parts={[
          { key: "w", label: "pesi", value: toGib(e()!.weights_bytes), cls: "ded" },
          { key: "kv", label: "cache KV", value: toGib(e()!.kv_bytes), cls: "kv2" },
          { key: "o", label: "stato + buffer", value: toGib(other()), cls: "oth" },
        ]}
      />
      <dl class="kv" style={{ "margin-top": "8px" }}>
        {line("Pesi", e()!.weights_bytes, "dal file, misurato", "var(--acc)")}
        {line("Cache KV", e()!.kv_bytes, `${num(props.row.estimate_ctx)} token · ${num(e()!.full_attention_blocks)} blocchi ad attenzione piena`, "var(--busy)")}
        {line("Stato ricorrente", e()!.state_bytes, "non cresce col contesto", "var(--fg3)")}
        {line("Buffer di calcolo", e()!.compute_bytes, e()!.compute_from ? `misurato su ${e()!.compute_from}` : undefined, "var(--fg3)")}
        <dt>{e()!.total_is_lower_bound ? "Totale minimo" : "Totale stimato"}</dt>
        <dd class="num">
          <b>
            {e()!.total_is_lower_bound ? "≥ " : "~ "}
            {gb(e()!.total_bytes)} GB
          </b>
          <span class="cond">
            {" "}
            / <Show when={props.vgm} fallback={<Unknown />}>{fixed(props.vgm!, 0)}</Show> GiB dedicati
          </span>
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

const taskLabel = (t: TaskView) =>
  t.kind === "verify" ? "Verifica SHA-256" : t.kind === "download" ? "Download" : t.kind === "copy" ? "Copia da un altro volume" : "Installazione";

const taskTone = (t: TaskView) =>
  t.state === "running" ? "acc" : t.state === "failed" ? "err" : t.state === "cancelled" ? "warn" : "ok";

/**
 * I lavori lunghi come fascia: una riga per lavoro, con la barra e il verbo. Le righe sono legate
 * all'id del lavoro, non all'oggetto che arriva ogni secondo: così «Ferma» non viene ricreato
 * mentre lo si clicca.
 */
function TaskBand(props: { tasks: TaskView[]; onCancel: (id: string) => void }) {
  const ids = createMemo(() => props.tasks.map((t) => t.id), [], { equals: (a, b) => a.join("|") === b.join("|") });
  return (
    <Show when={ids().length}>
      <div class="alerts">
        <For each={ids()}>
          {(id) => {
            const t = () => props.tasks.find((x) => x.id === id);
            return (
              <Show when={t()}>
                {(task) => (
                  <div class={`alert ${taskTone(task())}`}>
                    <span class="t">{taskLabel(task())}</span>
                    <span class="d row" style={{ gap: "10px" }}>
                      <span class="mono" style={{ "overflow-wrap": "anywhere" }}>
                        {task().target}
                      </span>
                      <Show
                        when={task().state === "running"}
                        fallback={
                          <span class={`badge ${taskTone(task())} tight`}>
                            <i />
                            {task().state === "done" ? "fatto" : task().state === "failed" ? "fallito" : "annullato"}
                            {task().ended ? ` · ${clock(task().ended!)}` : ""}
                          </span>
                        }
                      >
                        <span class="bar" style={{ flex: "1", "max-width": "260px" }}>
                          <span style={{ width: `${task().total ? Math.min(100, (task().done / task().total!) * 100) : 0}%` }} />
                        </span>
                        <span class="num cond">
                          {gb(task().done)} /{" "}
                          {task().total ? `${gb(task().total)} GB · ${((task().done / task().total!) * 100).toFixed(0)} %` : "? GB"}
                        </span>
                      </Show>
                      <Show when={task().message}>
                        <span class="cond">{task().message}</span>
                      </Show>
                    </span>
                    <Show when={task().state === "running"}>
                      <span class="a">
                        <button class="btn sm" onClick={() => props.onCancel(id)}>
                          Ferma
                        </button>
                      </span>
                    </Show>
                  </div>
                )}
              </Show>
            );
          }}
        </For>
      </div>
    </Show>
  );
}

function BuildsTab(props: {
  view: BuildsView | null;
  setView: (v: BuildsView) => void;
  status: EngineStatus | null;
  onError: (e: string) => void;
  onMessage: (m: string) => void;
}) {
  const [devices, setDevices] = createSignal<{ id: string; list: Device[] } | null>(null);
  const [busy, setBusy] = createSignal(false);
  const [focus, setFocus] = createSignal<string | null>(null);

  const load = async (refresh: boolean) => {
    setBusy(true);
    try {
      props.setView(await api.buildsList(refresh));
    } catch (e) {
      props.onError(String(e));
    } finally {
      setBusy(false);
    }
  };

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
      props.setView(await api.buildsImportDir(picked));
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

  const installed = () => props.view?.installed ?? [];
  const selected = () => installed().find((b) => b.id === focus()) ?? installed()[0] ?? null;
  /** La build dell'avvio acceso: il suo eseguibile è nella riga di comando. */
  const inUse = (binary: string) => {
    const r = api.isEngineOn(props.status) ? api.runOf(props.status) : null;
    return r != null && r.command_line.includes(binary);
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

      <div class="md">
        <div class="card flush">
          <table>
            <thead>
              <tr>
                <th>Id</th>
                <th>Origine</th>
                <th>Cartella</th>
                <th />
              </tr>
            </thead>
            <tbody>
              <For
                each={installed()}
                fallback={
                  <tr>
                    <td colspan={4}>
                      <Empty title="Nessuna build di llama.cpp su questa macchina.">
                        <div>
                          Senza build non c'è niente che possa caricare i pesi. «Cerca le release di ggml-org» le elenca e le
                          installa verificandone il digest; «Importa cartella…» dichiara una cartella che hai già scaricato a
                          mano. La build resta fissata nel profilo: cambiarla è una variabile di prova, non un aggiornamento
                          automatico.
                        </div>
                      </Empty>
                    </td>
                  </tr>
                }
              >
                {(b) => (
                  <tr class="click" classList={{ sel: selected()?.id === b.id }} onClick={() => setFocus(b.id)}>
                    <td class="mono">{b.id}</td>
                    <td class="cond">{b.source}</td>
                    <td class="mono mini" style={{ "overflow-wrap": "anywhere" }}>
                      {b.dir}
                    </td>
                    <td class="r">
                      <Show when={inUse(b.binary)}>
                        <span class="badge ok tight">
                          <i />
                          in uso ora
                        </span>
                      </Show>{" "}
                      <Show when={props.view?.used_by[b.id]}>
                        <span class="cond">{props.view!.used_by[b.id]} avvii</span>
                      </Show>
                    </td>
                  </tr>
                )}
              </For>
            </tbody>
          </table>
        </div>

        <div class="grid" style={{ "align-items": "start" }}>
          <Show when={selected()}>
            {(b) => (
              <div class="card">
                <h2>
                  {b().id} <span class="r">{b().source}</span>
                </h2>
                <div class="mono mini mb" style={{ "overflow-wrap": "anywhere" }}>
                  {b().binary}
                </div>
                <div class="row mb">
                  <button class="btn sm" onClick={() => showDevices(b().id)}>
                    --list-devices
                  </button>
                </div>
                <Show when={devices()?.id === b().id} fallback={<div class="cond">«--list-devices» chiede alla build che dispositivi vede.</div>}>
                  <pre class="log" style={{ "max-height": "120px" }}>
                    {devices()!
                      .list.map((x) => `${x.id}: ${x.name} (${x.total_mib} MiB, ${x.free_mib} MiB free)`)
                      .join("\n") || "nessun dispositivo elencato"}
                  </pre>
                </Show>
              </div>
            )}
          </Show>

          <Show when={props.view?.releases_error}>
            <div class="note err">{props.view!.releases_error}</div>
          </Show>
          <Show when={props.view?.releases.length}>
            <div class="card">
              <h2>Scaricabili</h2>
              <div class="cond mb">
                Le build escono come prerelease con tag <span class="mono">b&lt;numero&gt;</span>; ogni pacchetto porta il
                proprio digest SHA-256, verificato prima di estrarlo.
              </div>
              <div style={{ "overflow-x": "auto" }}>
                <table style={{ "font-size": "12px" }}>
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
                    <For each={props.view!.releases.slice(0, 4)}>
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
              </div>
            </div>
          </Show>
        </div>
      </div>
    </>
  );
}

/** Rimozione con scritto prima che cosa comporta: dal catalogo sì o no, il file sì o no. */
type Removal = { row: ModelRow; plan: api.RemovalPlan; deleteFile: boolean };

/**
 * Il dialogo della rimozione. Prima era un `window.confirm` doppio; ora è un dialogo in-app come gli
 * altri, con la cancellazione del file come scelta esplicita e gli avvisi scritti prima di confermare.
 */
function RemoveDialog(props: { removal: Removal; onToggle: (deleteFile: boolean) => void; onCancel: () => void; onConfirm: () => void }) {
  const r = () => props.removal;
  return (
    <Dialog
      title={`Rimuovere «${r().row.id}» dal catalogo?`}
      actions={
        <>
          <button class="btn danger" onClick={props.onConfirm}>
            {r().deleteFile ? "Rimuovi e cancella il file" : "Rimuovi dal catalogo"}
          </button>
          <button class="btn primary" onClick={props.onCancel}>
            Annulla
          </button>
        </>
      }
    >
      <p style={{ margin: "0 0 6px", color: "var(--fg2)" }}>
        {r().deleteFile ? "La voce esce dal catalogo e il file viene cancellato dal disco." : "La voce esce dal catalogo; il file resta dov'è."}
      </p>
      <Show when={r().plan.path}>
        <label class="row" style={{ margin: "0 0 6px", "align-items": "flex-start", "flex-wrap": "nowrap" }}>
          <input type="checkbox" checked={r().deleteFile} onChange={(e) => props.onToggle(e.currentTarget.checked)} />
          <span>
            Cancella anche il file <span class="mono" style={{ "overflow-wrap": "anywhere" }}>{r().plan.path}</span>
          </span>
        </label>
      </Show>
      <Show when={r().deleteFile}>
        <For each={r().plan.warnings}>{(w) => <div class="note warn" style={{ "margin-top": "6px" }}>{w}</div>}</For>
      </Show>
    </Dialog>
  );
}

export default function Catalogo(props: { onLaunch?: (file: string) => void; vgmGib?: number | null; status?: EngineStatus | null }) {
  const [tab, setTab] = createSignal<"modelli" | "build">("modelli");
  const [rows, setRows] = createSignal<ModelRow[]>([]);
  const [tasks, setTasks] = createSignal<TaskView[]>([]);
  const [focus, setFocus] = createSignal<string | null>(null);
  const [error, setError] = createSignal<string | null>(null);
  const [message, setMessage] = createSignal<string | null>(null);
  const [showAdd, setShowAdd] = createSignal(false);
  const [repo, setRepo] = createSignal("");
  const [file, setFile] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [adopt, setAdopt] = createSignal<AdoptPlan | null>(null);
  /** Perché il catalogo dichiarato non è stato letto: le righe qui sotto vengono dal disco. */
  const [catalogError, setCatalogError] = createSignal<string | null>(null);
  const [relink, setRelink] = createSignal<ModelRow | null>(null);
  const [removal, setRemoval] = createSignal<Removal | null>(null);
  const [filter, setFilter] = createSignal("tutti");
  const [search, setSearch] = createSignal("");
  const [builds, setBuilds] = createSignal<BuildsView | null>(null);

  const load = async () => {
    try {
      const view = await api.catalogList();
      setRows(view.rows);
      setCatalogError(view.error);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  };

  onMount(() => {
    load();
    api.buildsList(false).then(setBuilds, (e) => setError(String(e)));
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

  const chips = createMemo(() => {
    const counts = countBy(rows(), (r) => r.state);
    return [
      { id: "tutti", label: "tutti", count: rows().length },
      ...STATE_ORDER.filter((s) => counts.some((c) => c.id === s)).map((s) => ({
        id: s,
        label: STATES[s].chip,
        count: counts.find((c) => c.id === s)!.count,
      })),
    ];
  });
  // Un filtro che non ha più righe torna a «tutti»: una tabella vuota senza motivo inganna.
  createEffect(() => {
    if (filter() !== "tutti" && !chips().some((c) => c.id === filter())) setFilter("tutti");
  });

  const visible = createMemo(() => {
    const q = search().trim().toLowerCase();
    return rows().filter(
      (r) =>
        (filter() === "tutti" || r.state === filter()) &&
        (!q || [r.id, r.file, r.repo ?? "", r.quant ?? ""].some((x) => x.toLowerCase().includes(q))),
    );
  });

  const selected = createMemo(() => visible().find((r) => r.id === focus()) ?? visible()[0] ?? null);

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

  const askRemove = (row: ModelRow) =>
    act(async () => {
      const plan = await api.catalogRemovalPlan(row.id);
      setRemoval({ row, plan, deleteFile: false });
    });

  const remove = () =>
    act(async () => {
      const r = removal()!;
      setRemoval(null);
      setRows(await api.catalogRemove(r.row.id, r.deleteFile, true));
      setMessage(`«${r.row.id}» tolto dal catalogo${r.deleteFile ? " e cancellato dal disco" : ""}.`);
    });

  const add = () =>
    act(async () => {
      setRows(await api.catalogAdd(repo().trim(), file().trim()));
      setMessage(`«${file().trim()}» aggiunto: oid LFS e dimensione letti dal publisher.`);
      setRepo("");
      setFile("");
      setShowAdd(false);
    });

  const inputStyle = { width: "260px" };

  return (
    <section>
      <Head
        title="Catalogo"
        sub="Pesi e build presenti su questa macchina, verificati per hash. Un file già presente viene riconosciuto per nome e hash, anche quando è un hard link creato da un altro strumento."
      >
        <button class="btn sm primary" disabled={busy()} aria-expanded={showAdd()} onClick={() => setShowAdd(!showAdd())}>
          Aggiungi da Hugging Face…
        </button>
        <button class="btn sm" disabled={busy()} onClick={importFromDisk}>
          Importa da disco…
        </button>
        <button class="btn sm" disabled={busy()} onClick={load}>
          Riscansiona
        </button>
      </Head>

      <Show when={error()}>
        <div class="note err mb row">
          {error()}
          <button class="btn sm right" onClick={() => setError(null)}>
            Chiudi
          </button>
        </div>
      </Show>
      <Show when={catalogError()}>
        <div class="note err mb">
          <b>catalog.toml non è stato letto.</b> {catalogError()}
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

      <Show when={showAdd()}>
        <div class="card mb">
          <h2>
            Aggiungi da Hugging Face <span class="r">dimensione e oid LFS letti dal publisher, poi «Scarica» verifica</span>
          </h2>
          <div class="row">
            <input
              class="txt mono"
              style={inputStyle}
              aria-label="repository"
              placeholder="bartowski/Qwen_…-GGUF"
              value={repo()}
              onInput={(e) => setRepo(e.currentTarget.value)}
            />
            <input
              class="txt mono"
              style={inputStyle}
              aria-label="nome file"
              placeholder="Nome-Q4_K_M.gguf"
              value={file()}
              onInput={(e) => setFile(e.currentTarget.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && repo().trim() && file().trim()) add();
              }}
            />
            <button class="btn sm primary" disabled={busy() || !repo().trim() || !file().trim()} onClick={add}>
              Aggiungi
            </button>
            <button class="btn sm" onClick={() => setShowAdd(false)}>
              Annulla
            </button>
          </div>
        </div>
      </Show>

      <div class="tabs">
        <button classList={{ on: tab() === "modelli" }} onClick={() => setTab("modelli")}>
          Modelli<span class="n">{rows().length}</span>
        </button>
        <button classList={{ on: tab() === "build" }} onClick={() => setTab("build")}>
          Build llama.cpp
          <Show when={builds()}>
            <span class="n">{builds()!.installed.length}</span>
          </Show>
        </button>
      </div>

      <TaskBand tasks={tasks()} onCancel={(id) => act(() => api.taskCancel(id))} />
      <Show when={tasks().some((t) => t.state !== "running")}>
        <div class="row mb" style={{ "margin-top": "-6px" }}>
          <button
            class="btn sm ghost right"
            onClick={() =>
              act(async () => {
                await api.tasksClear();
                setTasks(await api.tasksList());
              })
            }
          >
            Togli i finiti
          </button>
        </div>
      </Show>

      <Show
        when={tab() === "modelli"}
        fallback={<BuildsTab view={builds()} setView={setBuilds} status={props.status ?? null} onError={setError} onMessage={setMessage} />}
      >
        <div class="row mb">
          <Chips label="filtra per stato" items={chips()} value={filter()} onChange={setFilter} />
          <input
            class="txt mono right"
            style={inputStyle}
            aria-label="cerca nel catalogo"
            placeholder="cerca per nome, quant o publisher"
            value={search()}
            onInput={(e) => setSearch(e.currentTarget.value)}
          />
        </div>

        <div class="md">
          <div class="card flush">
            <table>
              <thead>
                <tr>
                  <th>Modello</th>
                  <th>Quant</th>
                  <th class="r">GB</th>
                  <th>Stato</th>
                  <th class="r">
                    <span class="row nw" style={{ "justify-content": "flex-end", gap: "4px" }}>
                      Stima{" "}
                      <Q title="Pesi + cache KV + stato ricorrente + buffer di calcolo, al contesto del profilo che usa questi pesi. «≥» quando manca un pezzo: il totale è un minimo." />
                    </span>
                  </th>
                  <th>MTP</th>
                </tr>
              </thead>
              <tbody>
                <For
                  each={visible()}
                  fallback={
                    <tr>
                      <td colspan={6}>
                        <Show
                          when={rows().length}
                          fallback={
                            <Empty title="Nessun modello nel catalogo.">
                              <div>
                                «Aggiungi da Hugging Face…» con repository e nome file — Aethera ne legge dimensione e oid LFS
                                dal publisher e lo scarica verificandolo — oppure «Importa da disco…» per registrare un{" "}
                                <code>.gguf</code> che hai già, senza duplicarlo. Un file messo a mano nella cartella dei pesi
                                compare qui da solo.
                              </div>
                            </Empty>
                          }
                        >
                          <div class="cond" style={{ padding: "10px 2px" }}>
                            Nessun modello con questo filtro.
                          </div>
                        </Show>
                      </td>
                    </tr>
                  }
                >
                  {(r) => (
                    <tr class="click" classList={{ sel: selected()?.id === r.id }} onClick={() => setFocus(r.id)}>
                      <td>
                        <div class="mono">{r.id}</div>
                        <div class="mini">{r.repo ?? "file nella cartella pesi, senza repository"}</div>
                      </td>
                      <td>{show(r.quant ?? r.info?.dominant_type)}</td>
                      <td class="r num">
                        <Val v={gb(r.size_bytes) ?? (r.size_gb != null ? fixed(r.size_gb, 2) : null)} />
                      </td>
                      <td>
                        <span class={`badge ${STATES[r.state].cls} tight`}>
                          <i />
                          {STATES[r.state].text}
                        </span>
                        <Show when={(r.hard_links ?? 1) > 1}>
                          {" "}
                          <span class="pill" title="Lo stesso file è collegato da un altro strumento">
                            hard link ×{r.hard_links}
                          </span>
                        </Show>
                        <Show when={r.part_bytes}>
                          <div class="cond">{gb(r.part_bytes)} GB già scaricati</div>
                        </Show>
                        <Show when={r.renamed_candidates.length}>
                          <div class="cond" title={r.renamed_candidates.join("\n")}>
                            forse rinominato in {r.renamed_candidates[0]}
                          </div>
                        </Show>
                      </td>
                      <td class="r">
                        <EstimateCell row={r} />
                      </td>
                      <td class="mono mini">
                        <Show when={r.info} fallback={<span class="unk">—</span>}>
                          {r.info!.mtp_layers ? `${r.info!.mtp_layers} layer` : "—"}
                        </Show>
                      </td>
                    </tr>
                  )}
                </For>
              </tbody>
            </table>
          </div>

          <Show
            when={selected()}
            fallback={
              <div class="card cond">Clic su un modello per il dettaglio: percorso, hash, architettura, stima e azioni.</div>
            }
          >
            {(r) => (
              <div class="grid" style={{ "align-items": "start" }}>
                <div class="card">
                  <h2>
                    <span class="mono" style={{ "text-transform": "none", "letter-spacing": "0", "overflow-wrap": "anywhere" }}>
                      {r().id}
                    </span>{" "}
                    <span class="r">dettaglio</span>
                  </h2>
                  <div class="row mb">
                    <Show when={r().size_bytes != null}>
                      <button
                        class="btn sm primary"
                        title="Apre la pagina Avvio con questi pesi: il profilo che li usa, o uno nuovo"
                        onClick={() => props.onLaunch?.(r().file)}
                      >
                        Avvia…
                      </button>
                    </Show>
                    <Show when={r().state === "downloadable" || r().state === "downloading"}>
                      <button class="btn sm primary" disabled={busy()} onClick={() => act(() => api.catalogDownload(r().id))}>
                        {r().state === "downloading" ? "Riprendi" : "Scarica"}
                      </button>
                    </Show>
                    <Show when={r().renamed_candidates.length}>
                      <button class="btn sm" disabled={busy()} onClick={() => setRelink(r())}>
                        Ricollega…
                      </button>
                    </Show>
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
                    <button class="btn sm danger right" disabled={busy()} onClick={() => askRemove(r())}>
                      Rimuovi…
                    </button>
                  </div>
                  <dl class="kv">
                    <dt>File</dt>
                    <dd class="mono">{r().file}</dd>
                    <dt>Percorso</dt>
                    <dd class="mono">{show(r().path)}</dd>
                    <dt>Sorgente</dt>
                    <dd class="mono">{r().repo ? `huggingface.co/${r().repo} · resolve/main` : "—"}</dd>
                    <dt>SHA-256</dt>
                    <dd>
                      <span class="mono">
                        <Val v={shortHash(r().sha256_verified ?? r().sha256)} />
                      </span>{" "}
                      <Show when={r().verified_at}>
                        <span class={`badge ${r().state === "mismatch" ? "err" : "ok"} tight`}>
                          <i />
                          {r().state === "mismatch" ? "diverso" : "verificato"} {clock(r().verified_at!)}
                        </span>
                      </Show>
                      <Show when={!r().verified_at && r().sha256}>
                        <span class="cond">dichiarato, non ricalcolato</span>
                      </Show>
                    </dd>
                    <dt>Dimensione</dt>
                    <dd>
                      <Val v={gb(r().size_bytes)} unit="GB" />
                      <span class="cond"> decimali, come il publisher</span>
                    </dd>
                    <dt>Architettura</dt>
                    <dd class="mono">
                      <Show when={r().info} fallback={<span class="unk">—</span>}>
                        {r().info!.arch} · {num(r().info!.block_count)} blocchi · {num(r().info!.expert_count)} esperti /{" "}
                        {num(r().info!.expert_used_count)} attivi
                        <Show when={r().info!.full_attention_interval}>
                          <span class="cond"> · attenzione piena ogni {r().info!.full_attention_interval}</span>
                        </Show>
                      </Show>
                    </dd>
                    <dt>Tensori MTP</dt>
                    <dd class="mono">
                      <Show when={r().info?.mtp_types.length} fallback={<span class="unk">—</span>}>
                        {r().info!.mtp_types.join(" · ")}
                        <span class="cond"> in {num(r().info!.mtp_layers)} layer</span>
                      </Show>
                    </dd>
                    <dt>Contesto di training</dt>
                    <dd>
                      <Val v={num(r().info?.context_train)} />
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
                    <Show when={r().renamed_candidates.length}>
                      <dt>Forse rinominato</dt>
                      <dd class="mono">{r().renamed_candidates.join(" · ")}</dd>
                    </Show>
                  </dl>
                </div>

                <div class="card">
                  <h2>
                    Stima di memoria{" "}
                    <span class="r">
                      {r().estimate_profile ? `profilo ${r().estimate_profile}` : "senza profilo"}
                      {r().estimate_ctx ? ` · ${num(r().estimate_ctx)} token` : ""}
                    </span>
                  </h2>
                  <EstimateCard row={r()} vgm={props.vgmGib ?? null} />
                  <hr />
                  <Show
                    when={Object.keys(r().sampling_by_mode).length}
                    fallback={
                      <div class="cond">
                        Nessun campionamento consigliato registrato. «Leggi la model card» prende dal publisher quello che ci
                        scrive, con fonte e data; da qui lo si porta in un profilo e nelle righe per i client.
                      </div>
                    }
                  >
                    <h2>
                      Campionamento consigliato <span class="r">dalla model card</span>
                    </h2>
                    <div style={{ "overflow-x": "auto" }}>
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
                    </div>
                    <div class="cond" style={{ "margin-top": "6px" }}>
                      Dato del modello, non un default del server: il campionamento lo manda il client in ogni richiesta.
                    </div>
                  </Show>
                </div>
              </div>
            )}
          </Show>
        </div>
      </Show>

      <Show when={relink()}>
        {(r) => (
          <AskName
            title={`Ricollega «${r().id}»`}
            label="file"
            value={r().renamed_candidates[0] ?? r().file}
            confirmLabel="Ricollega"
            note="Lo SHA-256 già calcolato vale per il file vecchio: viene buttato, e il nuovo va verificato. Solo l'hash dice se è davvero lo stesso modello."
            onCancel={() => setRelink(null)}
            onConfirm={(file) =>
              act(async () => {
                const id = r().id;
                setRelink(null);
                setRows(await api.catalogRelink(id, file));
                setMessage(`«${id}» ricollegato a ${file}. Verificalo per sapere se è davvero lo stesso modello.`);
              })
            }
          />
        )}
      </Show>

      <Show when={removal()}>
        {(x) => (
          <RemoveDialog
            removal={x()}
            onToggle={(deleteFile) => setRemoval({ ...x(), deleteFile })}
            onCancel={() => setRemoval(null)}
            onConfirm={remove}
          />
        )}
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
