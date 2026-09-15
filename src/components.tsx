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
        return s.divergences.length ? { cls: "warn", text: "DIVERGENTE" } : { cls: "ok", text: "PRONTO" };
      case "exited":
        return { cls: "err", text: "USCITO CON ERRORE" };
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
