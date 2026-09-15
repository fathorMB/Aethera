import { For, JSX, Show } from "solid-js";
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

export function StateBadge(props: { status: EngineStatus | null }) {
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
    <span class={`badge ${look().cls}`}>
      <i />
      {look().text}
    </span>
  );
}

/** «IN USO» con il primo motivo (lock di un client, slot attivo, richiesta recente, protezione). */
export function UsageBadge(props: { status: EngineStatus | null; detail?: boolean }) {
  const u = () => (props.status?.state === "loading" || props.status?.state === "ready" ? props.status.usage : null);
  return (
    <Show when={u()?.in_use}>
      <span class="badge busy" title={u()!.reasons.join(" · ")}>
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
export function Spark(props: { values: (number | null)[]; max?: number; low?: number; height?: number }) {
  const max = () => props.max ?? Math.max(1e-9, ...props.values.map((v) => v ?? 0));
  return (
    <div class="spark" style={{ height: `${props.height ?? 34}px` }}>
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
