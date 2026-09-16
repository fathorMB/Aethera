import { createEffect, createMemo, createSignal, For, on, onCleanup, onMount, Show } from "solid-js";
import * as api from "../api";
import type { Comparison, RunDetail, RunRow } from "../api";
import { Empty, Spark, Val } from "../components";
import { clock, duration, fixed, num, pct } from "../format";

const selectStyle = {
  font: "inherit",
  background: "var(--bg2)",
  color: "var(--fg)",
  border: "1px solid var(--line)",
  "border-radius": "3px",
  padding: "3px 6px",
};

function Conditions(props: { row: RunRow }) {
  const r = () => props.row;
  return (
    <span class="row" style={{ gap: "4px" }}>
      <Show when={r().running}>
        <span class="badge ok tight">
          <i />
          acceso
        </span>
      </Show>
      <Show when={r().degraded.length}>
        <span class="badge warn tight" title={r().degraded.join("\n")}>
          <i />
          degradato
        </span>
      </Show>
      <Show when={r().double_copy}>
        <span class="badge err tight">
          <i />
          doppia copia
        </span>
      </Show>
      <Show when={r().margin_ok === false}>
        <span class="badge warn tight">
          <i />
          sotto il margine RAM
        </span>
      </Show>
      <Show when={r().invalidates_cache}>
        <span class="pill cache">invalida la cache</span>
      </Show>
      <Show when={r().by_user === false}>
        <span class="badge err tight">
          <i />
          uscito {r().exit_code ?? "?"}
        </span>
      </Show>
      <Show when={r().left_running}>
        <span class="pill">lasciato acceso</span>
      </Show>
      <Show when={!r().running && r().ended == null}>
        <span class="pill" title="Aethera si è chiusa senza registrare l'uscita">uscita non registrata</span>
      </Show>
    </span>
  );
}

function delta(a: number | null, b: number | null, digits = 0): { text: string | null; color?: string } {
  if (a == null || b == null || b === 0) return { text: null };
  const d = ((a - b) / b) * 100;
  return { text: `${d >= 0 ? "+" : ""}${fixed(d, digits)} %`, color: Math.abs(d) < 3 ? undefined : d > 0 ? "var(--ok)" : "var(--err)" };
}

function CompareCard(props: { cmp: Comparison }) {
  const a = () => props.cmp.a;
  const b = () => props.cmp.b;
  const metric = (label: string, pick: (r: RunRow) => number | null, digits: number, higherIsBetter = true) => {
    const d = delta(pick(a()), pick(b()));
    const color = higherIsBetter ? d.color : d.color === "var(--ok)" ? "var(--err)" : d.color === "var(--err)" ? "var(--ok)" : undefined;
    return (
      <tr>
        <td>{label}</td>
        <td class="r">
          <Val v={fixed(pick(a()), digits)} />
        </td>
        <td class="r">
          <Val v={fixed(pick(b()), digits)} />
        </td>
        <td class="r num" style={{ color }}>
          {d.text ?? ""}
        </td>
      </tr>
    );
  };
  return (
    <div class="card">
      <h2>
        Confronto{" "}
        <span class="r mono">
          {a().id} vs {b().id}
        </span>
      </h2>
      <table style={{ "font-size": "12px" }}>
        <thead>
          <tr>
            <th />
            <th class="r mono">A</th>
            <th class="r mono">B</th>
            <th class="r">Δ A su B</th>
          </tr>
        </thead>
        <tbody>
          {metric("Prefill mediana (tok/s)", (r) => r.summary.prefill_median, 0)}
          {metric("Decode mediana (tok/s)", (r) => r.summary.decode_median, 1)}
          {metric("Decode p10 (tok/s)", (r) => r.summary.decode_p10, 1)}
          {metric("Decode p90 (tok/s)", (r) => r.summary.decode_p90, 1)}
          {metric("Accettazione (%)", (r) => (r.summary.acceptance == null ? null : r.summary.acceptance * 100), 0)}
          {metric("Quota cache (%)", (r) => (r.summary.cache_share == null ? null : r.summary.cache_share * 100), 0)}
          {metric("VRAM misurata (GiB)", (r) => r.vram_dedicated_gib, 2, false)}
          {metric("Pronto in (s)", (r) => (r.load_ms == null ? null : r.load_ms / 1000), 1, false)}
        </tbody>
      </table>
      <hr />
      <h2>Differenze di profilo</h2>
      <Show when={props.cmp.profile_diff.length} fallback={<div class="cond">Stesso profilo effettivo.</div>}>
        <table style={{ "font-size": "12px" }}>
          <tbody>
            <For each={props.cmp.profile_diff}>
              {(o) => (
                <tr>
                  <td class="mono">{o.field}</td>
                  <td class="mono r">{o.base ?? "—"}</td>
                  <td class="mono r">{o.value ?? "—"}</td>
                </tr>
              )}
            </For>
          </tbody>
        </table>
      </Show>
      <hr />
      <h2>Condizioni</h2>
      <table style={{ "font-size": "12px" }}>
        <tbody>
          <For each={props.cmp.conditions}>
            {(c) => (
              <tr>
                <td style={{ color: c.a !== c.b ? "var(--warn)" : undefined }}>{c.label}</td>
                <td class="mono r">{c.a ?? "—"}</td>
                <td class="mono r">{c.b ?? "—"}</td>
              </tr>
            )}
          </For>
        </tbody>
      </table>
      <div class="note warn" style={{ "margin-top": "8px" }}>
        Il confronto mostra, non giudica: carico, tempo di accensione e numero di richieste cambiano la misura quanto il
        profilo.
      </div>
    </div>
  );
}

function DetailCard(props: { detail: RunDetail }) {
  const d = () => props.detail;
  const s = () => d().row.summary;
  const decode = () => d().records.map((r) => r.decode_tps ?? null);
  const low = () => (s().decode_median == null ? undefined : s().decode_median! * 0.7);
  return (
    <>
      <div class="card">
        <h2>
          Decode nel tempo <span class="r mono">{d().row.id} · per richiesta</span>
        </h2>
        <Show when={d().records.length} fallback={<div class="cond">Nessuna richiesta registrata in questo avvio.</div>}>
          <Spark values={decode().slice(-120)} low={low()} height={70} />
          <div class="cond" style={{ "margin-top": "4px" }}>
            Ultime {Math.min(120, decode().length)} richieste; rosso = sotto il 70 % della mediana dell'avvio.
          </div>
        </Show>
        <hr />
        <dl class="kv">
          <dt>Token proposti / accettati</dt>
          <dd class="num">
            {num(s().draft_n)} / {num(s().draft_accepted)}
          </dd>
          <dt>Prompt elaborati / dalla cache</dt>
          <dd class="num">
            {num(s().prompt_processed)} / {num(s().prompt_cached)}
          </dd>
          <dt>Token generati</dt>
          <dd class="num">{num(s().generated)}</dd>
          <dt>Ultimo prompt</dt>
          <dd>
            <Val v={num(s().last_prompt)} />
          </dd>
          <dt>Load mode · speculazione</dt>
          <dd class="mono">
            {d().row.load_mode} · {d().row.speculative}
          </dd>
        </dl>
        <For each={d().row.degraded}>{(m) => <div class="note warn" style={{ "margin-top": "6px" }}>{m}</div>}</For>
      </div>
      <div class="card">
        <h2>
          Manifest <span class="r mono">runs\{d().row.id}\manifest.toml</span>
        </h2>
        <pre class="log" style={{ "max-height": "420px" }}>
          {d().manifest_text}
        </pre>
      </div>
    </>
  );
}

export default function Benchmark() {
  const [rows, setRows] = createSignal<RunRow[]>([]);
  const [profile, setProfile] = createSignal("");
  const [machine, setMachine] = createSignal("");
  const [checked, setChecked] = createSignal<string[]>([]);
  const [focus, setFocus] = createSignal<string | null>(null);
  const [detail, setDetail] = createSignal<RunDetail | null>(null);
  const [cmp, setCmp] = createSignal<Comparison | null>(null);
  const [error, setError] = createSignal<string | null>(null);

  const load = async () => {
    try {
      setRows(await api.runsList());
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  };
  onMount(() => {
    load();
    const t = setInterval(load, 10000);
    onCleanup(() => clearInterval(t));
  });

  const profiles = createMemo(() => [...new Set(rows().map((r) => r.profile))].sort());
  const machines = createMemo(() => [...new Set(rows().map((r) => r.machine))].sort());
  const visible = () => rows().filter((r) => (!profile() || r.profile === profile()) && (!machine() || r.machine === machine()));

  createEffect(
    on([focus, rows], async ([id]) => {
      if (!id) return setDetail(null);
      try {
        setDetail(await api.runDetail(id));
      } catch (e) {
        setError(String(e));
      }
    }),
  );

  createEffect(
    on([checked, rows], async ([ids]) => {
      if (ids.length !== 2) return setCmp(null);
      try {
        setCmp(await api.runsCompare(ids[0], ids[1]));
      } catch (e) {
        setError(String(e));
      }
    }),
  );

  const toggle = (id: string) => {
    const c = checked();
    setChecked(c.includes(id) ? c.filter((x) => x !== id) : [...c, id].slice(-2));
  };

  return (
    <section>
      <h1>Benchmark</h1>
      <p class="sub">
        Storico degli avvii e delle loro prestazioni, misurate da Aethera mentre il motore serviva. Aethera non lancia
        banchi: ogni riga è un avvio, con le sue condizioni accanto.
      </p>
      <Show when={error()}>
        <div class="note err mb">{error()}</div>
      </Show>

      <div class="row mb">
        <span class="mini">Profilo</span>
        <select style={selectStyle} value={profile()} onChange={(e) => setProfile(e.currentTarget.value)}>
          <option value="">tutti</option>
          <For each={profiles()}>{(p) => <option value={p}>{p}</option>}</For>
        </select>
        <span class="mini">Macchina</span>
        <select style={selectStyle} value={machine()} onChange={(e) => setMachine(e.currentTarget.value)}>
          <option value="">tutte</option>
          <For each={machines()}>{(m) => <option value={m}>{m}</option>}</For>
        </select>
        <button class="btn sm" onClick={load}>
          Rileggi runs/
        </button>
        <span class="right cond">
          {checked().length === 2 ? "2 avvii selezionati → confronto sotto" : "seleziona due avvii per confrontarli · clic su una riga per il dettaglio"}
        </span>
      </div>

      <div style={{ "overflow-x": "auto" }}>
        <table>
          <thead>
            <tr>
              <th />
              <th>Avvio</th>
              <th>Data</th>
              <th>Profilo · differenze</th>
              <th>Build</th>
              <th class="r">Acceso</th>
              <th class="r">Richieste</th>
              <th class="r">Prefill</th>
              <th class="r">Decode</th>
              <th class="r">Accett.</th>
              <th class="r">Cache</th>
              <th class="r">VRAM</th>
              <th>Cond.</th>
            </tr>
          </thead>
          <tbody>
            <For
              each={visible()}
              fallback={
                <tr>
                  <td colspan={13}>
                    <Empty title="Nessun avvio registrato.">
                      <div>
                        Ogni volta che accendi il motore da Aethera resta qui una riga con le sue condizioni: build e
                        commit, contesto dichiarato e servito, memoria misurata dopo il caricamento, velocità e quota di
                        cache. Servono a confrontare due avvii — non sono un banco: i banchi li lancia chi li possiede.
                      </div>
                    </Empty>
                  </td>
                </tr>
              }
            >
              {(r) => (
                <tr classList={{ sel: checked().includes(r.id) || focus() === r.id }} class="click" onClick={() => setFocus(r.id)}>
                  <td onClick={(e) => e.stopPropagation()}>
                    <input type="checkbox" checked={checked().includes(r.id)} onChange={() => toggle(r.id)} />
                  </td>
                  <td class="mono">{r.id}</td>
                  <td class="num">{clock(r.started)}</td>
                  <td class="mono mini">
                    {r.profile}{" "}
                    <For each={r.overrides}>
                      {(o) => (
                        <span class="pill mod" title={`${o.base ?? "—"} → ${o.value ?? "—"}`}>
                          {o.field.split(".").pop()} {o.value ?? "—"}
                        </span>
                      )}
                    </For>
                  </td>
                  <td class="mono">{r.build}</td>
                  <td class="r num">
                    <Val v={r.uptime_s == null ? null : duration(r.uptime_s)} />
                  </td>
                  <td class="r num">{num(r.summary.requests)}</td>
                  <td class="r">
                    <Val v={fixed(r.summary.prefill_median, 0)} />
                  </td>
                  <td class="r">
                    <Val v={fixed(r.summary.decode_median, 1)} />
                  </td>
                  <td class="r">
                    <Val v={pct(r.summary.acceptance)} unit="%" />
                  </td>
                  <td class="r">
                    <Val v={pct(r.summary.cache_share)} unit="%" />
                  </td>
                  <td class="r">
                    <Val v={fixed(r.vram_dedicated_gib, 2)} />
                  </td>
                  <td>
                    <Conditions row={r} />
                  </td>
                </tr>
              )}
            </For>
          </tbody>
        </table>
      </div>

      <div class="grid g3" style={{ "margin-top": "12px", "align-items": "start" }}>
        <Show when={cmp()} fallback={<div class="card cond">Seleziona due avvii per il confronto: differenze di profilo e condizioni.</div>}>
          {(c) => <CompareCard cmp={c()} />}
        </Show>
        <Show when={detail()} fallback={<div class="card cond">Clic su una riga per decode nel tempo e manifest.</div>}>
          {(d) => <DetailCard detail={d()} />}
        </Show>
      </div>
    </section>
  );
}
