/**
 * Componenti della v2 (design/aethera-v2-ui): testa di pagina, avvisi, KPI, barra impilata, «?»,
 * righe «come si legge», filtri a bottoni, campo con etichetta umana. La logica sta in ui-logic.ts.
 */
import { For, JSX, Show } from "solid-js";
import { Spark, Unknown } from "./components";
import { fixed } from "./format";
import type { ChipItem, StackPart, Tone } from "./ui-logic";
import { stackSegments, stackTitle } from "./ui-logic";

/** Titolo, una riga di spiegazione, e le azioni della pagina a destra. */
export function Head(props: { title: string; sub: JSX.Element; children?: JSX.Element }) {
  return (
    <div class="head">
      <div>
        <h1>{props.title}</h1>
        <p class="sub">{props.sub}</p>
      </div>
      <Show when={props.children}>
        <div class="acts">{props.children}</div>
      </Show>
    </div>
  );
}

/** Il «?»: la spiegazione lunga sta nel title. Raggiungibile da tastiera. */
export function Q(props: { title: string }) {
  return (
    <span class="q" title={props.title} aria-label={props.title} role="note" tabindex={0}>
      ?
    </span>
  );
}

/** Riga «come si legge» a scomparsa. */
export function Help(props: { summary?: string; children: JSX.Element; class?: string; style?: JSX.CSSProperties }) {
  return (
    <details class={`help ${props.class ?? ""}`} style={props.style}>
      <summary>{props.summary ?? "come si legge"}</summary>
      <div class="body">{props.children}</div>
    </details>
  );
}

export interface AlertItem {
  key: string;
  tone: Tone;
  title: string;
  detail: JSX.Element;
  actions?: JSX.Element;
}

/** Una fascia sola di avvisi: una riga per avviso, con il verbo a destra. */
export function Alerts(props: { items: AlertItem[]; class?: string }) {
  return (
    <Show when={props.items.length}>
      <div class={`alerts ${props.class ?? ""}`}>
        <For each={props.items}>
          {(a) => (
            <div class={`alert ${a.tone}`} role={a.tone === "err" ? "alert" : undefined}>
              <span class="t">{a.title}</span>
              <span class="d">{a.detail}</span>
              <Show when={a.actions}>
                <span class="a">{a.actions}</span>
              </Show>
            </div>
          )}
        </For>
      </div>
    </Show>
  );
}

/** Un numero grande con unità, un giudizio in una riga e, se c'è, la sua scintilla. */
export function Kpi(props: {
  label: string;
  help?: string;
  value: string | null | undefined;
  unit?: string;
  tone?: Tone;
  detail?: JSX.Element;
  spark?: { values: (number | null)[]; max?: number; low?: number };
}) {
  return (
    <div class="kpi">
      <div class="l">
        {props.label}
        <Show when={props.help}>
          <Q title={props.help!} />
        </Show>
      </div>
      <div class={`v ${props.value != null ? (props.tone ?? "") : ""}`}>
        <Show when={props.value != null && props.value !== ""} fallback={<Unknown />}>
          {props.value}
          <Show when={props.unit}>
            <small>{props.unit}</small>
          </Show>
        </Show>
      </div>
      <Show when={props.detail}>
        <div class="d">{props.detail}</div>
      </Show>
      <Show when={props.spark && props.spark.values.length}>
        <Spark values={props.spark!.values} max={props.spark!.max} low={props.spark!.low} height={22} />
      </Show>
    </div>
  );
}

/**
 * Barra impilata con la sua legenda. I pezzi sconosciuti non si disegnano e nella legenda restano
 * «sconosciuto»; senza totale la barra resta vuota e tratteggiata.
 */
export function Stack(props: {
  parts: StackPart[];
  total: number | null | undefined;
  unit?: string;
  digits?: number;
  /** Colori della legenda per classe: gli stessi della barra. */
  legend?: boolean;
  after?: JSX.Element;
}) {
  const segs = () => stackSegments(props.parts, props.total);
  const unit = () => props.unit ?? "GiB";
  const color: Record<string, string> = {
    ded: "var(--acc)",
    sha: "var(--busy)",
    kv2: "var(--busy)",
    win: "var(--fg3)",
    oth: "var(--fg3)",
    vfr: "var(--acc-bg)",
  };
  return (
    <>
      <div
        class="stack"
        classList={{ nil: segs().length === 0 }}
        title={stackTitle(props.parts, props.digits ?? 2, unit())}
        role="img"
        aria-label={stackTitle(props.parts, props.digits ?? 2, unit())}
      >
        <For each={segs()}>{(s) => <span class={s.cls} style={{ width: `${s.pct}%` }} />}</For>
      </div>
      <Show when={props.legend !== false}>
        <div class="legend" style={{ "margin-top": "6px" }}>
          <For each={props.parts}>
            {(p) => (
              <span>
                <i style={color[p.cls] ? { background: color[p.cls] } : { border: "1px dashed var(--line)" }} />
                {p.label}{" "}
                <Show when={p.value != null} fallback={<Unknown />}>
                  <span class="num">{fixed(p.value, props.digits ?? 2)}</span>
                </Show>
              </span>
            )}
          </For>
          {props.after}
        </div>
      </Show>
    </>
  );
}

/** Filtri a bottoni con il conteggio. */
export function Chips(props: { items: ChipItem[]; value: string; onChange: (id: string) => void; label?: string }) {
  return (
    <div class="chips" role="group" aria-label={props.label}>
      <For each={props.items}>
        {(c) => (
          <button class="chip" classList={{ on: props.value === c.id }} aria-pressed={props.value === c.id} onClick={() => props.onChange(c.id)}>
            {c.label}
            <Show when={c.count != null}>
              <span class="n">{c.count}</span>
            </Show>
          </button>
        )}
      </For>
    </div>
  );
}

/** Scelta fra poche opzioni, come bottoni affiancati. */
export function Seg(props: { options: { id: string; label: string }[]; value: string; onChange: (id: string) => void; label?: string }) {
  return (
    <span class="seg" role="group" aria-label={props.label}>
      <For each={props.options}>
        {(o) => (
          <button classList={{ on: props.value === o.id }} aria-pressed={props.value === o.id} onClick={() => props.onChange(o.id)}>
            {o.label}
          </button>
        )}
      </For>
    </span>
  );
}

/**
 * Un campo: etichetta in italiano, il nome della leva in piccolo sotto, il controllo, e a destra
 * il valore di prima (se modificato), il suggerimento e i verdetti.
 */
export function Field(props: {
  label: string;
  lever?: string;
  mod?: boolean;
  bad?: boolean;
  hint?: JSX.Element;
  children: JSX.Element;
}) {
  return (
    <div class="f" classList={{ mod: !!props.mod, bad: !!props.bad }}>
      <label>
        {props.label}
        <Show when={props.lever}>
          <small>{props.lever}</small>
        </Show>
      </label>
      <div class="row nw">{props.children}</div>
      <span class="h">{props.hint}</span>
    </div>
  );
}
