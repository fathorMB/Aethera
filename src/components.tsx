import { createSignal, For, JSX, Show } from "solid-js";
import type { ArgGroup, EngineStatus } from "./api";

export function Unknown(props: { children?: JSX.Element }) {
  return <span class="unk">{props.children ?? "sconosciuto"}</span>;
}

/** Valore o «sconosciuto»: assente non è zero. */
export function Val(props: { v: string | number | null | undefined; unit?: string; mono?: boolean }) {
  return (
    <Show when={props.v != null && props.v !== ""} fallback={<Unknown />}>
      <span class={props.mono === false ? "" : "num"}>
        {props.v}
        {props.unit ? ` ${props.unit}` : ""}
      </span>
    </Show>
  );
}

export function StateBadge(props: { status: EngineStatus | null; big?: boolean }) {
  const look = () => {
    const s = props.status;
    switch (s?.state) {
      case "loading":
        return { cls: "acc", text: "IN CARICAMENTO" };
      case "ready":
        if (s.degraded.length) return { cls: "warn", text: "DEGRADATO" };
        return s.divergences.length ? { cls: "warn", text: "DIVERGENTE" } : { cls: "ok", text: "PRONTO" };
      case "exited":
        return { cls: "err", text: "USCITO CON ERRORE" };
      case "orphan":
        return { cls: "err", text: "ORFANO" };
      default:
        return { cls: "", text: "SPENTO" };
    }
  };
  return (
    <span class={`badge ${look().cls}`} classList={{ big: !!props.big }}>
      <i />
      {look().text}
    </span>
  );
}

/** «IN USO» con il primo motivo (lock di un client, slot attivo, richiesta recente, protezione). */
export function UsageBadge(props: { status: EngineStatus | null; detail?: boolean; big?: boolean }) {
  const u = () => (props.status?.state === "loading" || props.status?.state === "ready" ? props.status.usage : null);
  return (
    <Show when={u()?.in_use}>
      <span class="badge busy" classList={{ big: !!props.big }} title={u()!.reasons.join(" · ")}>
        <i />
        IN USO{props.detail !== false && u()!.reasons.length ? ` · ${u()!.reasons[0]}` : ""}
      </span>
    </Show>
  );
}

export function Toggle(props: { on: boolean; label: string; title?: string; onChange: (on: boolean) => void }) {
  return (
    <span
      class="toggle"
      classList={{ on: props.on }}
      title={props.title}
      role="switch"
      aria-checked={props.on}
      tabindex={0}
      onClick={() => props.onChange(!props.on)}
      onKeyDown={(e) => {
        if (e.key === " " || e.key === "Enter") {
          e.preventDefault();
          props.onChange(!props.on);
        }
      }}
    >
      <i />
      {props.label}
    </span>
  );
}

/** Barre per serie 0..max; `null` è una barra vuota tratteggiata, non uno zero. */
export function Spark(props: { values: (number | null)[]; max?: number; low?: number; height?: number; class?: string }) {
  const max = () => props.max ?? Math.max(1e-9, ...props.values.map((v) => v ?? 0));
  return (
    <div class={`spark ${props.class ?? ""}`} style={{ height: `${props.height ?? 34}px` }}>
      <For each={props.values}>
        {(v) => (
          <Show when={v != null} fallback={<b class="nil" title="sconosciuto" />}>
            <b
              classList={{ lo: props.low != null && v! < props.low }}
              style={{ height: `${Math.max(3, (v! / max()) * 100)}%` }}
              title={String(v)}
            />
          </Show>
        )}
      </For>
    </div>
  );
}

/** Riga di comando con le differenze dal profilo base: ● barrato il valore base, evidenziato il nuovo. */
export function CommandLine(props: { binary: string; args: ArgGroup[] }) {
  return (
    <pre class="cmd">
      {props.binary}
      <For each={props.args}>
        {(g) => (
          <>
            {"\n  "}
            <Show when={g.status === "same"}>
              {g.flag}
              {g.value != null ? ` ${g.value}` : ""}
            </Show>
            <Show when={g.status === "changed"}>
              {g.flag} <s>{g.base ?? ""}</s> <b>{g.value ?? ""}</b>
            </Show>
            <Show when={g.status === "added"}>
              <b>
                {g.flag}
                {g.value != null ? ` ${g.value}` : ""}
              </b>
            </Show>
            <Show when={g.status === "removed"}>
              <s>
                {g.flag}
                {g.base != null ? ` ${g.base}` : ""}
              </s>
            </Show>
          </>
        )}
      </For>
    </pre>
  );
}

export async function copy(text: string) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    /* appunti non disponibili: il testo resta selezionabile */
  }
}

/**
 * Stato vuoto: al posto di una tabella senza righe, che cosa manca e come si rimedia. Una tabella
 * vuota lascia chi guarda a chiedersi se è rotto qualcosa; una frase no.
 */
export function Empty(props: { title: string; children: JSX.Element; action?: JSX.Element }) {
  return (
    <div class="empty">
      <b>{props.title}</b>
      {props.children}
      <Show when={props.action}>
        <div class="row">{props.action}</div>
      </Show>
    </div>
  );
}

/** Cornice dei dialoghi: la stessa del dialogo «Esci», così non ce ne sono due specie. */
export function Dialog(props: { title: string; children: JSX.Element; actions: JSX.Element }) {
  return (
    <div class="overlay">
      <div class="card dialog">
        <h2>{props.title}</h2>
        {props.children}
        <div class="row" style={{ "margin-top": "12px" }}>
          {props.actions}
        </div>
      </div>
    </div>
  );
}

/**
 * Chiede un nome. Non usa `window.prompt`: nella WebView di Windows non c'è, e un pulsante che
 * non fa niente è peggio di un pulsante che manca.
 */
export function AskName(props: {
  title: string;
  label: string;
  value: string;
  note?: string;
  confirmLabel?: string;
  /** Accetta anche il nome proposto così com'è (per «Salva come» di un profilo mai salvato). */
  allowSame?: boolean;
  onCancel: () => void;
  onConfirm: (value: string) => void;
}) {
  const [value, setValue] = createSignal(props.value);
  const ok = () => value().trim().length > 0 && (props.allowSame || value().trim() !== props.value);
  return (
    <Dialog
      title={props.title}
      actions={
        <>
          <button class="btn primary" disabled={!ok()} onClick={() => props.onConfirm(value().trim())}>
            {props.confirmLabel ?? "Conferma"}
          </button>
          <button class="btn" onClick={props.onCancel}>
            Annulla
          </button>
        </>
      }
    >
      <div class="field" style={{ "grid-template-columns": "110px 1fr" }}>
        <label>{props.label}</label>
        <input
          class="mono"
          autofocus
          value={value()}
          onInput={(e) => setValue(e.currentTarget.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && ok()) props.onConfirm(value().trim());
            if (e.key === "Escape") props.onCancel();
          }}
        />
      </div>
      <Show when={props.note}>
        <div class="cond" style={{ "margin-top": "6px" }}>
          {props.note}
        </div>
      </Show>
    </Dialog>
  );
}

/** Conferma di una cosa che non si disfa, con scritto prima che cosa comporta. */
export function Confirm(props: {
  title: string;
  lines: string[];
  confirmLabel: string;
  danger?: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  return (
    <Dialog
      title={props.title}
      actions={
        <>
          <button class={props.danger ? "btn danger" : "btn primary"} onClick={props.onConfirm}>
            {props.confirmLabel}
          </button>
          <button class="btn primary" onClick={props.onCancel}>
            Annulla
          </button>
        </>
      }
    >
      <For each={props.lines}>
        {(l) => (
          <p style={{ margin: "0 0 6px", color: "var(--fg2)" }}>{l}</p>
        )}
      </For>
    </Dialog>
  );
}
