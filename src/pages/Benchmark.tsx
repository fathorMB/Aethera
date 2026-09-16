import { createEffect, createMemo, createSignal, For, on, onCleanup, onMount, Show } from "solid-js";
import * as api from "../api";
import type { Comparison, RunDetail, RunRow } from "../api";
import { Empty, Spark, Val } from "../components";
import { clock, duration, fixed, num, pct } from "../format";
import { Chips, Head } from "../ui";
import { countBy, delta, flip } from "../ui-logic";

/** Le pillole di stato di un avvio, in una colonna sola. */
function Notes(props: { row: RunRow }) {
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
        <span class="pill" title="Aethera si è chiusa senza registrare l'uscita">
          uscita non registrata
        </span>
      </Show>
    </span>
  );
}

function Delta(props: { a: number | null | undefined; b: number | null | undefined; digits?: number; lowerIsBetter?: boolean }) {
  const d = () => delta(props.a, props.b, props.digits ?? 0);
  const dir = () => (props.lowerIsBetter ? flip(d().dir) : d().dir);
  return <span class={`delta ${dir()}`}>{d().text ?? "—"}</span>;
}

function CompareCard(props: { cmp: Comparison }) {
  const a = () => props.cmp.a;
  const b = () => props.cmp.b;
  const metric = (label: string, pick: (r: RunRow) => number | null, digits: number, lowerIsBetter = false) => (
    <tr>
      <td>{label}</td>
      <td class="r">
        <Val v={fixed(pick(a()), digits)} />
      </td>
      <td class="r">
        <Val v={fixed(pick(b()), digits)} />
      </td>
      <td class="r">
        <Delta a={pick(a())} b={pick(b())} lowerIsBetter={lowerIsBetter} />
      </td>
    </tr>
  );
  return (
    <div class="card" id="confronto">
      <h2>
        Confronto{" "}
        <span class="r mono">
          A {a().id} · B {b().id}
        </span>
      </h2>
      <Show when={props.cmp.not_comparable.length}>
        <div class="note warn mb">
          <b>A e B non differiscono solo per il profilo:</b> {props.cmp.not_comparable.join(" · ")}. Su questa macchina il solo
          driver GPU ha spostato il prefill del 17 % (M-08): il Δ qui sotto non è attribuibile al profilo.
        </div>
      </Show>
      <div class="grid g3" style={{ "align-items": "start" }}>
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
            {metric("VRAM misurata (GiB)", (r) => r.vram_dedicated_gib, 2, true)}
            {metric("Pronto in (s)", (r) => (r.load_ms == null ? null : r.load_ms / 1000), 1, true)}
          </tbody>
        </table>
        <div>
          <h3>Differenze di profilo</h3>
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
        </div>
        <div>
          <h3>Condizioni</h3>
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
        </div>
      </div>
      <div class="note warn" style={{ "margin-top": "8px" }}>
        Il confronto mostra, non giudica: carico, tempo di accensione e numero di richieste cambiano la misura quanto il
        profilo.
      </div>
    </div>
  );
}

function DetailCard(props: { detail: RunDetail }) {
  const d = () => props.detail;
  const row = () => d().row;
  const s = () => row().summary;
  const decode = () => d().records.map((r) => r.decode_tps ?? null);
  const low = () => (s().decode_median == null ? undefined : s().decode_median! * 0.7);
  return (
    <div class="card">
      <h2>
        <span class="mono" style={{ "text-transform": "none", "letter-spacing": "0" }}>
          {row().id}
        </span>
        <span class="r">
          dettaglio · profilo {row().profile} · {row().build}
        </span>
      </h2>
      <Show when={d().records.length} fallback={<div class="cond">Nessuna richiesta registrata in questo avvio.</div>}>
        <Spark values={decode().slice(-120)} low={low()} height={64} />
        <div class="cond" style={{ margin: "4px 0 8px" }}>
          Decode per richiesta, ultime {Math.min(120, decode().length)}; rosso = sotto il 70 % della mediana dell'avvio.
        </div>
      </Show>
      <dl class="kv">
        <dt>Token proposti / accettati</dt>
        <dd class="num">
          {num(s().draft_n)} / {num(s().draft_accepted)}{" "}
          <span class="cond">
            accettazione <Val v={pct(s().acceptance)} unit="%" />
          </span>
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
        <dt>Pronto in</dt>
        <dd>
          <Val v={row().load_ms == null ? null : fixed(row().load_ms! / 1000, 1)} unit="s" />
        </dd>
        <dt>Contesto</dt>
        <dd>
          <span class="num">{num(row().ctx_declared)}</span> <span class="cond">dichiarato · servito</span>{" "}
          <Val v={num(row().ctx_served)} />
        </dd>
        <dt>Load mode · speculazione</dt>
        <dd class="mono">
          {row().load_mode} · {row().speculative}
        </dd>
        <dt>Build · commit</dt>
        <dd class="mono">
          {row().build} · <Val v={row().commit} />
        </dd>
        <dt>RAM dopo il caricamento</dt>
        <dd>
          <Val v={fixed(row().ram_available_after_gib, 2)} unit="GiB disponibili" />
        </dd>
        <dt>Condizioni</dt>
        <dd class="mono mini">
          {row().conditions_short ?? "condizioni sconosciute"}
          <Show when={row().reference}>
            {(ref) => (
              <div class="cond">
                mediana di riferimento {fixed(ref().decode_median, 1)} tok/s su {ref().runs} avvii con le stesse condizioni
              </div>
            )}
          </Show>
        </dd>
      </dl>
      <For each={row().degraded}>
        {(m) => (
          <div class="note warn" style={{ "margin-top": "6px" }}>
            {m}
          </div>
        )}
      </For>
      <details style={{ "margin-top": "10px" }}>
        <summary>
          Manifest{" "}
          <span class="mono cond" style={{ "overflow-wrap": "anywhere" }}>
            runs\{row().id}\manifest.toml
          </span>
        </summary>
        <div class="body">
          <pre class="log" style={{ "max-height": "320px", "margin-top": "8px" }}>
            {d().manifest_text}
          </pre>
        </div>
      </details>
    </div>
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
  const [cmpOpen, setCmpOpen] = createSignal(false);
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

  const profiles = createMemo(() =>
    countBy(rows(), (r) => r.profile).sort((x, y) => x.id.localeCompare(y.id)),
  );
  const machines = createMemo(() =>
    countBy(rows(), (r) => r.machine).sort((x, y) => x.id.localeCompare(y.id)),
  );
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
      if (ids.length !== 2) {
        setCmpOpen(false);
        return setCmp(null);
      }
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

  const openCompare = () => {
    setCmpOpen(true);
    requestAnimationFrame(() => document.getElementById("confronto")?.scrollIntoView({ behavior: "smooth", block: "start" }));
  };

  return (
    <section>
      <Head
        title="Benchmark"
        sub="Storico degli avvii e delle loro prestazioni, misurate da Aethera mentre il motore serviva. Aethera non lancia banchi: ogni riga è un avvio, con le sue condizioni accanto."
      >
        <button class="btn sm" onClick={load}>
          Rileggi runs/
        </button>
      </Head>
      <Show when={error()}>
        <div class="note err mb">{error()}</div>
      </Show>

      <div class="row mb">
        <span class="mini">Profilo</span>
        <Chips
          label="filtra per profilo"
          value={profile()}
          onChange={setProfile}
          items={[{ id: "", label: "tutti", count: rows().length }, ...profiles().map((p) => ({ id: p.id, label: p.id, count: p.count }))]}
        />
        <Show when={machines().length > 0}>
          <span class="mini" style={{ "margin-left": "10px" }}>
            Macchina
          </span>
          <Chips
            label="filtra per macchina"
            value={machine()}
            onChange={setMachine}
            items={[{ id: "", label: "tutte" }, ...machines().map((m) => ({ id: m.id, label: m.id, count: m.count }))]}
          />
        </Show>
        <span class="right cond">spunta due avvii per confrontarli · clic su una riga per il dettaglio</span>
      </div>

      <div class="md wide">
        <div class="card flush">
          <table>
            <thead>
              <tr>
                <th aria-label="confronta" />
                <th>Avvio</th>
                <th>Profilo · differenze</th>
                <th>Build</th>
                <th class="r">Acceso</th>
                <th class="r">Rich.</th>
                <th class="r">Prefill</th>
                <th class="r">Decode</th>
                <th class="r">Riuso</th>
                <th class="r">VRAM</th>
                <th>Note</th>
              </tr>
            </thead>
            <tbody>
              <For
                each={visible()}
                fallback={
                  <tr>
                    <td colspan={11}>
                      <Empty title="Nessun avvio registrato.">
                        <div>
                          Ogni volta che accendi il motore da Aethera resta qui una riga con le sue condizioni: build e commit,
                          contesto dichiarato e servito, memoria misurata dopo il caricamento, velocità e quota di cache.
                          Servono a confrontare due avvii — non sono un banco: i banchi li lancia chi li possiede.
                        </div>
                      </Empty>
                    </td>
                  </tr>
                }
              >
                {(r) => (
                  <>
                    <tr classList={{ sel: checked().includes(r.id) || focus() === r.id }} class="click" onClick={() => setFocus(r.id)}>
                      <td onClick={(e) => e.stopPropagation()}>
                        <input
                          type="checkbox"
                          aria-label={`confronta ${r.id}`}
                          checked={checked().includes(r.id)}
                          onChange={() => toggle(r.id)}
                        />
                      </td>
                      <td title={r.conditions_short ?? "condizioni sconosciute"}>
                        <div class="mono">{r.id}</div>
                        <div class="mini num">{clock(r.started)}</div>
                      </td>
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
                        <Show when={r.summary.decode_series.length}>
                          <Spark
                            class="inline"
                            values={r.summary.decode_series.slice(-12)}
                            low={r.summary.decode_median == null ? undefined : r.summary.decode_median * 0.7}
                            height={12}
                          />
                        </Show>
                      </td>
                      <td class="r">
                        <Val v={pct(r.summary.cache_share)} unit="%" />
                        <Show when={r.summary.cache_series.length}>
                          <Spark class="inline" values={r.summary.cache_series.slice(-12)} max={1} low={0.5} height={12} />
                        </Show>
                      </td>
                      <td class="r">
                        <Val v={fixed(r.vram_dedicated_gib, 2)} />
                      </td>
                      <td>
                        <Notes row={r} />
                      </td>
                    </tr>
                    <Show when={r.conditions_changed.length}>
                      <tr class="sep">
                        <td colspan={11}>
                          ▲ da qui in su: {r.conditions_changed.join(" · ")}. La mediana di riferimento per «degradato» riparte,
                          e le righe sotto non si confrontano con quelle sopra senza dirlo.
                        </td>
                      </tr>
                    </Show>
                  </>
                )}
              </For>
            </tbody>
          </table>
        </div>

        <Show when={detail()} fallback={<div class="card cond">Clic su una riga per decode nel tempo e manifest.</div>}>
          {(d) => <DetailCard detail={d()} />}
        </Show>
      </div>

      <Show when={checked().length > 0}>
        <div class="cmpbar">
          <Show
            when={cmp()}
            fallback={
              <span class="cond">
                <span class="mono">{checked()[0]}</span> spuntato: spunta un secondo avvio per il confronto.
              </span>
            }
          >
            {(c) => (
              <>
                <span class="mono">{c().a.id}</span>
                <span class="cond">contro</span>
                <span class="mono">{c().b.id}</span>
                <span style={{ width: "1px", height: "20px", background: "var(--line)" }} />
                <span>
                  prefill <Delta a={c().a.summary.prefill_median} b={c().b.summary.prefill_median} />
                </span>
                <span>
                  decode <Delta a={c().a.summary.decode_median} b={c().b.summary.decode_median} />
                </span>
                <span>
                  VRAM <Delta a={c().a.vram_dedicated_gib} b={c().b.vram_dedicated_gib} digits={1} lowerIsBetter />
                </span>
                <Show when={c().not_comparable.length}>
                  <span class="badge warn tight" title={c().not_comparable.join(" · ")}>
                    <i />
                    non differiscono solo per il profilo
                  </span>
                </Show>
                <button class="btn sm primary right" onClick={() => (cmpOpen() ? setCmpOpen(false) : openCompare())}>
                  {cmpOpen() ? "Chiudi il confronto" : "Apri il confronto"}
                </button>
              </>
            )}
          </Show>
          <button class="btn sm" classList={{ right: !cmp() }} onClick={() => setChecked([])}>
            Togli le spunte
          </button>
        </div>
      </Show>

      <Show when={cmpOpen() && cmp()}>
        {(c) => (
          <div style={{ "margin-top": "12px" }}>
            <CompareCard cmp={c()} />
          </div>
        )}
      </Show>
    </section>
  );
}
