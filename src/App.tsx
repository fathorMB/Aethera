import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { createSignal, For, Match, onCleanup, onMount, Show, Switch } from "solid-js";
import * as api from "./api";
import type { EngineStatus, Overview } from "./api";
import { StateBadge, Toggle, UsageBadge } from "./components";
import { duration } from "./format";
import Avvio from "./pages/Avvio";
import Benchmark from "./pages/Benchmark";
import Catalogo from "./pages/Catalogo";
import Impostazioni from "./pages/Impostazioni";
import Motore from "./pages/Motore";

export type PageId = "motore" | "avvio" | "catalogo" | "benchmark" | "impostazioni";

const PAGES: { id: PageId; label: string; key: string }[] = [
  { id: "motore", label: "Motore", key: "1" },
  { id: "avvio", label: "Avvio", key: "2" },
  { id: "catalogo", label: "Catalogo", key: "3" },
  { id: "benchmark", label: "Benchmark", key: "4" },
  { id: "impostazioni", label: "Impostazioni", key: "5" },
];

function readTheme(): "dark" | "light" {
  try {
    return localStorage.getItem("aethera.theme") === "light" ? "light" : "dark";
  } catch {
    return "dark";
  }
}

function Welcome(props: { onDone: (o: Overview) => void }) {
  const [path, setPath] = createSignal("");
  const [error, setError] = createSignal<string | null>(null);
  const confirm = async () => {
    setError(null);
    try {
      props.onDone(await api.setDataRoot(path()));
    } catch (e) {
      setError(String(e));
    }
  };
  return (
    <div class="welcome card">
      <h1>Radice dati</h1>
      <p class="sub">
        Scegli dove Aethera tiene <code>machine.toml</code>, <code>profiles/</code>, <code>builds/</code> e{" "}
        <code>runs/</code>. I profili non contengono percorsi assoluti: puoi spostare la radice più avanti.
      </p>
      <div class="field" style={{ "grid-template-columns": "1fr max-content" }}>
        <input value={path()} placeholder="D:\Aethera" onInput={(e) => setPath(e.currentTarget.value)} />
        <button
          class="btn sm"
          onClick={async () => {
            const picked = await open({ directory: true });
            if (typeof picked === "string") setPath(picked);
          }}
        >
          Sfoglia…
        </button>
      </div>
      <Show when={error()}>
        <div class="note err" style={{ "margin-top": "8px" }}>
          {error()}
        </div>
      </Show>
      <div class="row" style={{ "margin-top": "12px" }}>
        <button class="btn primary right" disabled={!path().trim()} onClick={confirm}>
          Usa questa cartella
        </button>
      </div>
    </div>
  );
}

function ExitDialog(props: { status: EngineStatus | null; onClose: () => void }) {
  const [remember, setRemember] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const exit = async (stop: boolean) => {
    try {
      await api.appExit(stop, remember());
    } catch (e) {
      setError(String(e));
    }
  };
  const usage = () => api.usageOf(props.status);
  return (
    <div class="overlay">
      <div class="card dialog">
        <h2>Esci</h2>
        <p style={{ margin: "0 0 8px" }}>
          Il motore <span class="mono">{api.runOf(props.status)?.profile}</span> è acceso
          <Show when={usage()?.in_use}>
            {" "}
            e risulta <b>in uso</b> ({usage()!.reasons.join(", ")})
          </Show>
          .
        </p>
        <p style={{ margin: "0 0 10px", color: "var(--fg2)" }}>
          Se lo lasci acceso resta orfano: Aethera al prossimo avvio lo mostrerà come tale e potrà solo leggerlo o
          terminarlo.
        </p>
        <Show when={error()}>
          <div class="note err mb">{error()}</div>
        </Show>
        <div class="row">
          <button class="btn danger" disabled={usage()?.in_use} title={usage()?.in_use ? "in uso" : ""} onClick={() => exit(true)}>
            Ferma ed esci
          </button>
          <button class="btn" onClick={() => exit(false)}>
            Lascia acceso ed esci
          </button>
          <button class="btn primary" onClick={props.onClose}>
            Annulla
          </button>
        </div>
        <label class="cond" style={{ display: "block", "margin-top": "8px" }}>
          <input type="checkbox" checked={remember()} onChange={(e) => setRemember(e.currentTarget.checked)} /> Ricorda la
          scelta
        </label>
      </div>
    </div>
  );
}

export default function App() {
  const [page, setPage] = createSignal<PageId>("motore");
  const [theme, setTheme] = createSignal(readTheme());
  const [overview, setOverview] = createSignal<Overview | null>(null);
  const [status, setStatus] = createSignal<EngineStatus | null>(null);
  const [askExit, setAskExit] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  const applyTheme = (t: "dark" | "light") => {
    setTheme(t);
    document.documentElement.dataset.theme = t;
    try {
      localStorage.setItem("aethera.theme", t);
    } catch {
      /* preferenza solo locale */
    }
  };

  onMount(() => {
    applyTheme(theme());
    api.overview().then(setOverview, (e) => setError(String(e)));

    const poll = async () => {
      try {
        setStatus(await api.engineStatus());
      } catch {
        /* il prossimo giro riprova */
      }
    };
    poll();
    const t = setInterval(poll, 1000);

    const onKey = (e: KeyboardEvent) => {
      if ((e.target as HTMLElement).closest("input, select, textarea")) return;
      const hit = PAGES.find((p) => p.key === e.key);
      if (hit) setPage(hit.id);
    };
    document.addEventListener("keydown", onKey);

    const unlisten = listen("aethera://ask-exit", () => setAskExit(true));
    const unlistenNotice = listen<string>("aethera://notice", (e) => setError(e.payload));
    onCleanup(() => {
      clearInterval(t);
      document.removeEventListener("keydown", onKey);
      unlisten.then((f) => f());
      unlistenNotice.then((f) => f());
    });
  });

  const run = () => api.runOf(status());
  const ready = () => {
    const s = status();
    return s?.state === "ready" ? s : null;
  };

  return (
    <div class="app">
      <div class="top">
        <span class="brand">
          Aethera<small>{overview()?.machine?.name ?? ""} · 0.1</small>
        </span>
        <StateBadge status={status()} />
        <Show when={api.isEngineOn(status()) && run()}>
          {(r) => (
            <>
              <span class="mono">{r().profile}</span>
              <span class="pill mono">{r().base_url.replace("http://", "")}</span>
              <span class="pill mono">{r().build}</span>
            </>
          )}
        </Show>
        <UsageBadge status={status()} />
        <span class="spacer" />
        <Show when={api.usageOf(status())}>
          {(u) => (
            <Toggle
              on={u().protected}
              label="Proteggi il motore"
              title="Rifiuta arresto e riavvio finché è attiva"
              onChange={async (on) => {
                await api.engineProtect(on);
                setStatus(await api.engineStatus());
              }}
            />
          )}
        </Show>
        <button class="btn sm" onClick={() => applyTheme(theme() === "dark" ? "light" : "dark")}>
          ☼ tema
        </button>
      </div>

      <nav class="nav">
        <For each={PAGES}>
          {(p) => (
            <button classList={{ on: page() === p.id }} onClick={() => setPage(p.id)}>
              {p.label}
              <span class="k">{p.key}</span>
            </button>
          )}
        </For>
        <div class="foot">
          Radice dati
          <br />
          <span class="mono">{overview()?.data_root ?? "—"}</span>
          <Show when={ready()}>
            {(r) => (
              <>
                <br />
                <br />
                Acceso da <span class="num">{duration(r().uptime_s)}</span>
                <br />
                Avvio <span class="mono">{r().run.run_id}</span>
              </>
            )}
          </Show>
        </div>
      </nav>

      <main class="main">
        <Show when={error()}>
          <div class="note err mb row">
            {error()}
            <button class="btn sm right" onClick={() => setError(null)}>
              Chiudi
            </button>
          </div>
        </Show>
        <Show when={overview()}>
          {(o) => (
            <Show when={o().data_root} fallback={<Welcome onDone={setOverview} />}>
              <Switch>
                <Match when={page() === "motore"}>
                  <Motore status={status()} onGo={setPage} />
                </Match>
                <Match when={page() === "avvio"}>
                  <Avvio overview={o()} status={status()} onStarted={() => setPage("motore")} />
                </Match>
                <Match when={page() === "catalogo"}>
                  <Catalogo />
                </Match>
                <Match when={page() === "benchmark"}>
                  <Benchmark />
                </Match>
                <Match when={page() === "impostazioni"}>
                  <Impostazioni overview={o()} status={status()} onChange={setOverview} />
                </Match>
              </Switch>
            </Show>
          )}
        </Show>
      </main>

      <Show when={askExit()}>
        <ExitDialog status={status()} onClose={() => setAskExit(false)} />
      </Show>
    </div>
  );
}
