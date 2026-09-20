import { createMemo, createSignal, For, JSX, onCleanup, onMount, Show } from "solid-js";
import * as api from "../api";
import type { Conditions, EngineStatus, MemorySection, Orphan, Overview, ReadyStatus, RunInfo, Setup, Summary, TurnKind } from "../api";
import type { EngineControls, PageId, SettingsTab } from "../App";
import { copy, Empty, StateBadge, Toggle, UsageBadge, Val } from "../components";
import { clock, duration, fixed, num, pct } from "../format";
import { Alerts, type AlertItem, Head, Help, Kpi, Q, Stack } from "../ui";
import { decodeJudgement, DEGRADED_DECODE_RATIO, engineAlerts, memoryParts, reuseJudgement, seconds, shareTone } from "../ui-logic";

type Go = (page: PageId, tab?: SettingsTab) => void;
type Loading = Extract<EngineStatus, { state: "loading" }>;
type Exited = Extract<EngineStatus, { state: "exited" }>;
type Off = Extract<EngineStatus, { state: "off" }>;

const vgmOf = (c: Conditions | null | undefined, o: Overview) =>
  c?.gpus?.find((g) => g.dedicated_gib != null)?.dedicated_gib ?? o.system.gpus.find((g) => g.dedicated_gib != null)?.dedicated_gib ?? null;

/** Memoria come barra impilata su VGM + RAM di Windows, con i numeri di prima e dopo sotto. */
function MemoryCard(props: { memory: MemorySection | null; loadMs: number | null; vgm: number | null; ramTotal: number | null }) {
  const b = () => props.memory?.before ?? null;
  const a = () => props.memory?.after_load ?? null;
  const mibToGib = (m: number | null | undefined) => (m == null ? null : m / 1024);
  const stack = () => memoryParts(a(), props.vgm, props.ramTotal);
  return (
    <div class="card">
      <h2>
        Memoria{" "}
        <Q title="VRAM dedicata: GPU Process Memory del processo. La barra sta su VGM più la RAM che Windows vede; la VRAM condivisa viene dalla RAM di Windows e non si conta due volte." />
        <span class="r">
          <Show when={a()} fallback="in attesa della lettura dopo il caricamento">
            misurata {fixed(a()!.after_ms / 1000, 1)} s dopo l'avvio
            <Show when={props.loadMs != null}> · pronto in {fixed(props.loadMs! / 1000, 1)} s</Show>
          </Show>
        </span>
      </h2>
      <Stack
        parts={stack().parts}
        total={stack().total}
        after={
          <span>
            su <Val v={fixed(stack().total, 1)} unit="GiB" /> <span class="cond">(VGM + RAM vista)</span>
          </span>
        }
      />
      <dl class="kv" style={{ "margin-top": "10px" }}>
        <dt>Prima dell'avvio</dt>
        <dd>
          <Val v={fixed(mibToGib(b()?.vram_free_mib), 2)} unit="GiB" /> di VRAM liberi ·{" "}
          <Val v={fixed(b()?.ram_available_gib, 2)} unit="GiB" /> di RAM a Windows
        </dd>
        <dt>Working set</dt>
        <dd>
          <Val v={fixed(a()?.working_set_gib, 2)} unit="GiB" />{" "}
          <Show when={a()?.double_copy != null} fallback={<span class="cond">doppia copia: <Val v={null} /></span>}>
            <span class={`badge tight ${a()!.double_copy ? "err" : "ok"}`}>
              <i />
              {a()!.double_copy ? "doppia copia dei pesi" : "nessuna doppia copia"}
            </span>{" "}
            <span class="cond">working set ≥ metà dei pesi</span>
          </Show>
        </dd>
        <dt>Margine RAM</dt>
        <dd>
          <Show when={a()} fallback={<span class="cond">soglia di margine RAM da machine.toml</span>}>
            <Show when={a()!.margin_ok != null} fallback={<Val v={null} />}>
              <span class="num" style={{ color: a()!.margin_ok ? "var(--ok)" : "var(--err)" }}>
                {fixed(a()!.ram_available_gib, 2)} {a()!.margin_ok ? "✓" : "✗ sotto la soglia"}
              </span>
            </Show>{" "}
            <span class="cond">soglia ≥ {fixed(a()!.ram_margin_gib, 0)} GiB da machine.toml</span>
          </Show>
        </dd>
        <Show when={b()?.device}>
          <dt>Dispositivo</dt>
          <dd>
            {b()!.device} · <span class="num">{num(b()!.vram_total_mib)}</span> MiB <span class="cond">(--list-devices)</span>
          </dd>
        </Show>
      </dl>
    </div>
  );
}

/** Driver, alimentazione, VGM e disco dei pesi: le condizioni che M-08 ha dovuto ricavare a mano. */
function ConditionsCard(props: { conditions: Conditions | null; changed: string[] }) {
  const c = () => props.conditions;
  const changedKey = (k: string) => props.changed.some((x) => x.startsWith(k));
  return (
    <div class="card">
      <h2>
        Condizioni dell'avvio{" "}
        <Q title="Lette all'avvio e registrate nel manifest. Cambiano i numeri: Benchmark non confronta due avvii con condizioni diverse senza dirlo." />
        <span class="r">lette all'avvio, nel manifest</span>
      </h2>
      <Show when={c()} fallback={<div class="cond">Non registrate: l'avvio è precedente a quando Aethera le legge.</div>}>
        {(k) => (
          <dl class="kv">
            <For each={k().gpus ?? []} fallback={<><dt>Driver GPU</dt><dd><Val v={null} /></dd></>}>
              {(g) => (
                <>
                  <dt>Driver GPU</dt>
                  <dd>
                    <span class="mono">
                      <Val v={g.version} />
                    </span>{" "}
                    <Show when={changedKey("driver GPU")}>
                      <span class="pill mod">cambiato</span>
                    </Show>{" "}
                    <span class="cond">
                      {g.name}
                      {g.date ? ` · ${g.date}` : ""}
                      {k().adrenalin ? ` · AMD Software ${k().adrenalin}` : ""}
                    </span>
                  </dd>
                </>
              )}
            </For>
            <For each={k().npus ?? []} fallback={<><dt>Driver NPU</dt><dd><Val v={null} /></dd></>}>
              {(n) => (
                <>
                  <dt>Driver NPU</dt>
                  <dd>
                    <span class="mono">
                      <Val v={n.version} />
                    </span>{" "}
                    <Show when={changedKey("driver NPU")}>
                      <span class="pill mod">cambiato</span>
                    </Show>{" "}
                    <span class="cond">{n.name}</span>
                  </dd>
                </>
              )}
            </For>
            <dt>Alimentazione</dt>
            <dd>
              <Val v={api.overlayName(k().power_overlay)} mono={false} />{" "}
              <Show when={changedKey("alimentazione")}>
                <span class="pill mod">cambiata</span>
              </Show>{" "}
              <span class="cond" title="overlay dal registro: powercfg mostra solo lo schema sotto">
                overlay dal registro
              </span>
            </dd>
            <dt>VGM</dt>
            <dd>
              <Val v={fixed(k().gpus?.find((g) => g.dedicated_gib != null)?.dedicated_gib, 0)} unit="GiB dedicati" />
            </dd>
            <dt>Pesi su</dt>
            <dd>
              <span class="mono">
                <Val v={k().weights_volume} />
              </span>{" "}
              · <Val v={k().weights_disk} mono={false} />{" "}
              <span class="cond">
                {k().weights_bus ?? "bus sconosciuto"} ·{" "}
                {k().weights_free_gb != null ? `${num(Math.round(k().weights_free_gb!))} GB liberi all'avvio` : "spazio libero sconosciuto"}
              </span>
            </dd>
          </dl>
        )}
      </Show>
    </div>
  );
}

const KIND: Record<TurnKind, { label: string; cls: string }> = {
  cold: { label: "a freddo", cls: "cold" },
  extends: { label: "estende", cls: "ext" },
  rewrites: { label: "riscrive", cls: "sum" },
  summary: { label: "compattazione · riassunto", cls: "sum" },
  rebuilt: { label: "compattazione · contesto ricostruito", cls: "reb" },
  unknown: { label: "non attribuibile", cls: "cold" },
};

/** Quante righe si vedono prima di «tutte». */
const TURNS_SHOWN = 12;

/** Richiesta per richiesta: quanto il motore ha riusato, quanto ha rielaborato, e le compattazioni. */
function TurnsCard(props: { summary: Summary }) {
  const [all, setAll] = createSignal(false);
  const turns = () => [...props.summary.turns].reverse();
  const shown = () => (all() ? turns() : turns().slice(0, TURNS_SHOWN));
  return (
    <div class="card">
      <h2>
        Richiesta per richiesta <span class="r">dalla più recente · quanto il motore ha riusato e rielaborato</span>
      </h2>
      <Show when={turns().length} fallback={<div class="cond">Nessuna richiesta servita da questo avvio.</div>}>
        <div style={{ "overflow-x": "auto" }}>
          <table class="ev">
            <thead>
              <tr>
                <th>ora</th>
                <th>client</th>
                <th class="r">prompt</th>
                <th class="r">riusati</th>
                <th class="r">rielaborati</th>
                <th style={{ width: "96px" }}>quota</th>
                <th class="r">prefill</th>
                <th class="r">decode</th>
                <th>lettura</th>
              </tr>
            </thead>
            <tbody>
              <For each={shown()}>
                {(t) => {
                  const share = () => (t.prompt_total ? (t.cache_n ?? 0) / t.prompt_total : null);
                  return (
                    <tr classList={{ cmp: t.kind === "summary" || t.kind === "rebuilt" }}>
                      <td class="num" title={clock(t.at)}>{clock(t.at).slice(6)}</td>
                      <td class="mono">
                        <Val v={t.client} />
                      </td>
                      <td class="r">
                        <Val v={num(t.prompt_total)} />
                      </td>
                      <td class="r">
                        <Val v={num(t.cache_n)} />
                      </td>
                      <td class="r num">{num(t.prompt_n)}</td>
                      <td>
                        <Show when={share() != null} fallback={<div class="bar nil" title="sconosciuto" />}>
                          <div class={`bar ${shareTone(share())}`} title={pct(share()) + " %"}>
                            <span style={{ width: `${Math.max(share()! * 100, 1)}%` }} />
                          </div>
                        </Show>
                      </td>
                      <td class="r num">{seconds(t.prompt_ms)}</td>
                      <td class="r num">{seconds(t.gen_ms)}</td>
                      <td>
                        <span class={`tag ${KIND[t.kind].cls}`}>{KIND[t.kind].label}</span>
                      </td>
                    </tr>
                  );
                }}
              </For>
            </tbody>
          </table>
        </div>
      </Show>
      <div class="row" style={{ "margin-top": "8px", "align-items": "flex-start" }}>
        <Show when={turns().length > TURNS_SHOWN}>
          <span class="cond">
            Mostrate {shown().length} di {turns().length} ·{" "}
            <button class="btn sm ghost" onClick={() => setAll(!all())}>
              {all() ? "solo le ultime" : "tutte"}
            </button>
          </span>
        </Show>
        <Help class="right" style={{ "max-width": "560px" }}>
          I token riusati vengono dal log del motore (contesto a fine richiesta meno generati e rielaborati, a meno di 2 token
          {props.summary.cache_sources.some((x) => x !== "log")
            ? `; alcune righe da ${props.summary.cache_sources.filter((x) => x !== "log").join(" e ")}`
            : ""}
          ). Il <b>client</b> si conosce solo se ha preso il lock sull'endpoint di Aethera. <b>Estende</b>: tiene almeno il
          90 % della conversazione di prima, o tutto tranne l'ultimo ubatch. <b>Contesto ricostruito</b>: il prompt è più
          corto del 70 % della conversazione di prima; la richiesta che riscriveva subito prima è il <b>riassunto</b>. Le
          righe ambra sono la coppia di una compattazione.
        </Help>
      </div>
    </div>
  );
}

/** Quattro numeri con un giudizio ciascuno. */
function KpiRow(props: { rd: ReadyStatus }) {
  const t = () => props.rd.telemetry;
  const decode = () => decodeJudgement(t().decode_median, props.rd.reference);
  const reuse = () => reuseJudgement(t().cache_share);
  const low = () => (t().decode_median == null ? undefined : t().decode_median! * DEGRADED_DECODE_RATIO);
  const cost = () => t().compactions.reduce((a, c) => a + c.cost_ms, 0);
  return (
    <div class="kpis mb">
      <Kpi
        label="Prefill"
        help={`Mediana delle ultime ${t().window} richieste con almeno 128 token di prompt, dal log del motore.`}
        value={fixed(t().prefill_median, 0)}
        unit="tok/s"
        detail={
          <>
            mediana · ≥ 128 token · ultimo prompt <Val v={num(t().last_prompt)} unit="tok" />{" "}
            <span title="elaborati + cache">(elaborati + cache)</span>
          </>
        }
      />
      <Kpi
        label="Decode"
        help={`Mediana delle ultime ${t().window} richieste. Rosso nella scintilla: sotto il 70 % della mediana dell'avvio. Il giudizio confronta la mediana con quella di riferimento degli avvii con le stesse condizioni.`}
        value={fixed(t().decode_median, 1)}
        unit="tok/s"
        tone={decode().tone}
        detail={
          <>
            p10 <Val v={fixed(t().decode_p10, 1)} /> · p90 <Val v={fixed(t().decode_p90, 1)} /> · accettazione draft{" "}
            <Val v={pct(t().acceptance)} unit="%" />
            <br />
            {decode().text}
          </>
        }
        spark={{ values: t().decode_series, low: low() }}
      />
      <Kpi
        label="Riuso del prompt"
        help="Token dalla cache / token di prompt, dal log del motore. Quota riusata per richiesta nella scintilla: rossa sotto il 50 %, il client ha cambiato il prompt prima della coda. Tratteggio = non attribuibile."
        value={pct(t().cache_share)}
        unit="% riusato"
        tone={reuse().tone}
        detail={reuse().text}
        spark={{ values: t().cache_series, max: 1, low: 0.5 }}
      />
      <Kpi
        label="Compattazioni"
        help="Richiesta con il prompt più corto del 70 % del contesto precedente, preceduta da una che riscriveva: la coppia riassunto + contesto ricostruito. Il costo è il tempo di quelle richieste."
        value={String(t().compactions.length)}
        unit={`nelle ultime ${t().window}`}
        tone={t().compactions.length ? "warn" : ""}
        detail={
          <Show when={t().compactions.length} fallback="nessuna nella finestra">
            {t().compactions.length === 1 ? "costata" : "costate"} {seconds(cost())}
            <Show when={t().compactions[t().compactions.length - 1]?.client}> · {t().compactions[t().compactions.length - 1].client}</Show>
          </Show>
        }
      />
    </div>
  );
}

/** Un riquadro di stato solo, per tutti gli stati in cui c'è un avvio da raccontare. */
function StatusCard(props: { s: EngineStatus; run: RunInfo; engine: EngineControls }) {
  const ready = () => (props.s.state === "ready" ? props.s : null);
  const loading = () => (props.s.state === "loading" ? (props.s as Loading) : null);
  const exited = () => (props.s.state === "exited" ? (props.s as Exited).finished : null);
  const last = () => (props.s.state === "off" ? (props.s as Off).last : null);
  const usage = () => api.usageOf(props.s);
  const r = () => props.run;
  return (
    <div class="card mb">
      <div class="row" style={{ gap: "14px", "align-items": "flex-start", "flex-wrap": "nowrap" }}>
        <div style={{ display: "flex", "flex-direction": "column", gap: "6px", "align-items": "flex-start" }}>
          <StateBadge status={props.s} big />
          <UsageBadge status={props.s} detail={false} big />
        </div>
        <dl class="kv k3">
          <dt>Profilo</dt>
          <dd>
            <span class="mono">{r().profile}</span>{" "}
            <Show when={ready()}>
              {(rd) => (
                <span class="cond">
                  alias servito{" "}
                  <Show when={rd().alias_served != null} fallback={<Val v={null} />}>
                    <span class="mono">{rd().alias_served}</span>
                  </Show>
                </span>
              )}
            </Show>
          </dd>

          <Show when={ready()}>
            {(rd) => (
              <>
                <dt>Acceso da</dt>
                <dd>
                  <span class="num">{duration(rd().uptime_s)}</span>{" "}
                  <span class="cond">
                    dal {clock(r().started_at)} · pronto in {fixed(rd().load_ms / 1000, 1)} s
                  </span>
                </dd>
              </>
            )}
          </Show>
          <Show when={loading()}>
            {(l) => (
              <>
                <dt>In caricamento da</dt>
                <dd>
                  <span class="num">{duration(l().elapsed_s)}</span> <span class="cond">/health</span> <Val v={l().health} />
                </dd>
              </>
            )}
          </Show>
          <Show when={exited()}>
            {(f) => (
              <>
                <dt>Uscito</dt>
                <dd>
                  <span class="num">{clock(f().ended_at)}</span> <span class="cond">codice</span> <Val v={f().code} />
                </dd>
              </>
            )}
          </Show>
          <Show when={last()}>
            {(f) => (
              <>
                <dt>Fermato</dt>
                <dd>
                  <span class="num">{clock(f().ended_at)}</span>
                  <Show when={f().left_running}>
                    {" "}
                    <span class="cond">lasciato acceso all'uscita: non più gestito</span>
                  </Show>
                </dd>
              </>
            )}
          </Show>

          <dt>Contesto</dt>
          <dd>
            <span class="num">{num(r().ctx_declared)}</span> <span class="cond">dichiarato · servito</span>{" "}
            <Show when={ready()} fallback={<Val v={null} />}>
              {(rd) => (
                <>
                  <Val v={num(rd().ctx_served)} />{" "}
                  <Show when={rd().ctx_served === r().ctx_declared}>
                    <span class="badge ok tight">
                      <i />
                      coincide
                    </span>
                  </Show>
                </>
              )}
            </Show>
          </dd>

          <dt>Indirizzo</dt>
          <dd>
            <span class="mono">{r().base_url}</span>{" "}
            <span class="cond">
              {r().build} · PID {r().pid}
            </span>
          </dd>

          <dt>Richieste</dt>
          <dd>
            <Show when={ready()} fallback={<Val v={null} />}>
              {(rd) => (
                <>
                  <span class="num">{num(rd().telemetry.requests)}</span>{" "}
                  <span class="cond" title="/metrics, cache inclusa">
                    prompt <Val v={num(rd().counters ? rd().counters!.prompt_tokens + rd().counters!.cached_tokens : null)} /> ·
                    generati <Val v={num(rd().counters?.predicted_tokens)} /> · /metrics, cache inclusa
                  </span>
                </>
              )}
            </Show>
          </dd>

          <dt>Slot</dt>
          <dd>
            <span class="pill mono">{r().n_parallel}</span>{" "}
            <Show when={usage()?.slot_processing != null} fallback={<Val v={null} />}>
              <span class="cond">is_processing = {String(usage()!.slot_processing)}</span>
            </Show>
          </dd>

          <dt>Avvio</dt>
          <dd>
            <span class="mono">{r().run_id}</span>{" "}
            <span class="cond">
              {r().overrides.length
                ? `${r().overrides.length} ${r().overrides.length === 1 ? "differenza" : "differenze"} dal profilo`
                : "come il profilo"}
            </span>
          </dd>

          <For each={usage()?.locks ?? []}>
            {(l) => (
              <>
                <dt>Client dichiarato</dt>
                <dd>
                  <span class="mono">
                    {l.client}
                    {l.label ? ` · ${l.label}` : ""}
                  </span>{" "}
                  <span class="cond">lock · scade fra {duration(l.expires_in_s)}</span>
                </dd>
              </>
            )}
          </For>
        </dl>
        <Show when={usage()}>
          {(u) => (
            <Toggle
              on={u().protected}
              label="Proteggi il motore"
              title="Rifiuta arresto e riavvio finché è attiva"
              onChange={(on) => props.engine.protect(on)}
            />
          )}
        </Show>
      </div>
      <Show when={usage()?.in_use}>
        <div class="note busy" style={{ "margin-top": "10px" }}>
          Arresto e riavvio sono rifiutati finché lo stato è <b>IN USO</b> ({usage()!.reasons.join(", ")}): riavviare svuota la
          cache del prefisso del client che sta lavorando.
        </div>
      </Show>
    </div>
  );
}

function OrphanCard(props: { orphan: Orphan }) {
  const [confirm, setConfirm] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const terminate = async () => {
    setError(null);
    try {
      await api.orphanTerminate(props.orphan.pid);
    } catch (e) {
      setError(String(e));
    }
    setConfirm(false);
  };
  return (
    <div class="card mb">
      <div class="row" style={{ gap: "14px", "align-items": "flex-start", "flex-wrap": "nowrap" }}>
        <div style={{ display: "flex", "flex-direction": "column", gap: "6px", "align-items": "flex-start" }}>
          <StateBadge status={{ state: "orphan", orphan: props.orphan, last: null }} big />
          <span class="cond">sola lettura</span>
        </div>
        <dl class="kv k3">
          <dt>Indirizzo</dt>
          <dd class="mono">{props.orphan.base_url}</dd>
          <dt>Processo</dt>
          <dd class="mono">
            {props.orphan.process} · PID {props.orphan.pid}
          </dd>
          <dt>Alias servito</dt>
          <dd class="mono">
            <Val v={props.orphan.alias} />
          </dd>
          <dt>Contesto servito</dt>
          <dd>
            <Val v={num(props.orphan.ctx)} />
          </dd>
          <dt>Slot</dt>
          <dd>
            <Show when={props.orphan.slot_processing != null} fallback={<Val v={null} />}>
              {props.orphan.slot_processing ? "al lavoro" : "libero"}
            </Show>
          </dd>
          <Show when={props.orphan.left_by_aethera}>
            <dt>Origine</dt>
            <dd>
              lasciato acceso all'uscita dall'avvio <span class="mono">{props.orphan.left_by_aethera}</span>
            </dd>
          </Show>
        </dl>
      </div>
      <Show when={error()}>
        <div class="note err" style={{ "margin-top": "8px" }}>
          {error()}
        </div>
      </Show>
      <div class="row" style={{ "margin-top": "10px" }}>
        <span class="cond">
          Un llama-server su questa porta non è stato avviato da questo Aethera. Aethera non ha il suo log né la sua
          telemetria: può solo leggerlo o terminarlo.
        </span>
        <Show
          when={confirm()}
          fallback={
            <button class="btn sm danger right" disabled={props.orphan.slot_processing === true} onClick={() => setConfirm(true)}>
              Termina…
            </button>
          }
        >
          <span class="right">Terminare il PID {props.orphan.pid}?</span>
          <button class="btn sm danger" onClick={terminate}>
            Termina
          </button>
          <button class="btn sm" onClick={() => setConfirm(false)}>
            Annulla
          </button>
        </Show>
      </div>
    </div>
  );
}

/**
 * La prima configurazione, in ordine. Non è una procedura guidata che prende in ostaggio la
 * finestra: è un elenco che dice a che punto si è e dove si fa il passo che manca. Ogni passo si
 * può fare anche per conto proprio, e questo elenco se ne accorge.
 */
function PrimoAvvio(props: { setup: Setup; onGo: Go }) {
  const next = () => props.setup.steps.find((s) => !s.done);
  return (
    <div class="card mb">
      <h2>
        Primo avvio <span class="r">dalla cartella dei dati al motore acceso</span>
      </h2>
      <div class="steps">
        <For each={props.setup.steps}>
          {(s, i) => (
            <div class="step" classList={{ done: s.done, now: s.id === next()?.id }}>
              <span class="n">{s.done ? "✓" : i() + 1}</span>
              <span>
                <span class="t">{s.title}</span>
                <span class="d"> — {s.done ? s.detail : s.what}</span>
              </span>
              <Show when={s.id === next()?.id} fallback={<span />}>
                <button class="btn sm primary" onClick={() => props.onGo(s.page as PageId)}>
                  Vai
                </button>
              </Show>
            </div>
          )}
        </For>
      </div>
    </div>
  );
}


/** I motori di servizio (M-20): piccoli, accanto al principale, senza misure.
 *
 * Perche' stanno in un riquadro a parte e non nella riga del motore: non sono lo stesso mestiere.
 * Il principale si misura e si confronta con gli avvii di ieri; un servizio o risponde o no. E non
 * rende il motore «in uso»: se un embedding notturno impedisse di riavviare il motore, avremmo
 * scambiato il servo col padrone. */
function ServicesCard() {
  const [rows, setRows] = createSignal<api.ServiceEntry[] | null>(null);
  const [busy, setBusy] = createSignal<string | null>(null);
  const [error, setError] = createSignal<string | null>(null);

  const refresh = async () => setRows(await api.servicesList().catch(() => []));
  onMount(() => {
    refresh();
    const t = setInterval(refresh, 3000);
    onCleanup(() => clearInterval(t));
  });

  const act = async (name: string, start: boolean) => {
    setBusy(name);
    setError(null);
    try {
      await (start ? api.serviceStart(name) : api.serviceStop(name));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
      await refresh();
    }
  };

  const quali = (k: api.ServiceKind) => (k === "embedding" ? "embedding" : k === "rerank" ? "rerank" : "chat");

  return (
    <Show when={rows()?.length}>
      <div class="card mb">
        <h2>
          Motori di servizio{" "}
          <span class="r">
            {rows()!.filter((r) => r.running).length} accesi · non rendono il motore «in uso»
          </span>
          <Help summary="che cosa sono">
            Sono <code>llama-server</code> piccoli accesi accanto al motore principale, per chi ha bisogno di un
            endpoint che il modello grosso non dà: embedding, rerank, o una chat leggera per lavori che non sono
            codice. Non lasciano manifest né telemetria, perché non sono il soggetto di una misura. Stanno su porte
            loro e non bloccano arresto né riavvio del motore principale.
          </Help>
        </h2>
        <table>
          <thead>
            <tr>
              <th>Profilo</th>
              <th>Serve</th>
              <th class="r">Porta</th>
              <th class="r">VRAM</th>
              <th>Stato</th>
              <th />
            </tr>
          </thead>
          <tbody>
            <For each={rows()!}>
              {(r) => (
                <tr>
                  <td>
                    <b>{r.profile.name}</b>
                    <div class="mini">{r.profile.model.file}</div>
                  </td>
                  <td>
                    {quali(r.profile.service.kind)}
                    <Show when={r.profile.service.embed_dim}>
                      <span class="mini"> · {r.profile.service.embed_dim} dim</span>
                    </Show>
                  </td>
                  <td class="r">{r.profile.server.port}</td>
                  <td class="r">
                    {r.running?.vram_dedicated_gib != null ? `${fixed(r.running.vram_dedicated_gib, 2)} GiB` : "—"}
                  </td>
                  <td>
                    <Show when={r.running} fallback={<span class="badge">spento</span>}>
                      {(v) => (
                        <>
                          <span class={v().state === "exited" ? "badge err" : "badge ok"}>
                            {v().state === "exited" ? "caduto" : "pronto"}
                          </span>
                          <Show when={v().sane === false}>
                            <div class="note warn" style={{ margin: "4px 0 0" }}>
                              Risponde, ma il test di sanità dice che i punteggi non sono attendibili: questo GGUF del
                              reranker è probabilmente convertito male.
                            </div>
                          </Show>
                        </>
                      )}
                    </Show>
                    <Show when={!r.running && r.blockers.length}>
                      <div class="note warn" style={{ margin: "4px 0 0" }}>
                        {r.blockers[0]}
                      </div>
                    </Show>
                    <Show when={r.issues.length}>
                      <div class="note warn" style={{ margin: "4px 0 0" }}>
                        {r.issues[0].field}: {r.issues[0].message}
                      </div>
                    </Show>
                  </td>
                  <td style={{ "text-align": "right" }}>
                    <button
                      class="btn"
                      disabled={busy() === r.profile.name || (!r.running && r.blockers.length > 0)}
                      onClick={() => act(r.profile.name, !r.running)}
                    >
                      {busy() === r.profile.name ? "…" : r.running ? "Ferma" : "Avvia"}
                    </button>
                  </td>
                </tr>
              )}
            </For>
          </tbody>
        </table>
        <Show when={error()}>
          <div class="note err">{error()}</div>
        </Show>
      </div>
    </Show>
  );
}

export default function Motore(props: { status: EngineStatus | null; overview: Overview; onGo: Go; engine: EngineControls }) {
  const [setup, setSetup] = createSignal<Setup | null>(null);
  const [log, setLog] = createSignal("");
  /** Le righe del log che spiegano un'uscita con errore: un codice di uscita da solo non dice niente. */
  const [failure, setFailure] = createSignal<api.Reason[]>([]);
  const [error, setError] = createSignal<string | null>(null);
  const run = () => api.runOf(props.status);
  const s = () => props.status;
  const ready = () => (s()?.state === "ready" ? (s() as ReadyStatus) : null);
  const exited = () => s()?.state === "exited";

  onMount(() => {
    const tick = async () => {
      if (!run()) return;
      try {
        setLog(await api.engineLog(200));
      } catch (e) {
        setLog(String(e));
      }
      // Solo dopo un'uscita con errore: a motore sano non c'è niente da spiegare.
      setFailure(s()?.state === "exited" ? await api.engineFailure().catch(() => []) : []);
    };
    tick();
    const t = setInterval(tick, 2000);

    // La guida si aggiorna da sola: un passo fatto in un'altra pagina si vede tornando qui.
    const setupTick = async () => setSetup(await api.setup().catch(() => null));
    setupTick();
    const t2 = setInterval(setupTick, 3000);
    onCleanup(() => {
      clearInterval(t);
      clearInterval(t2);
    });
  });

  const copyClient = async () => {
    setError(null);
    try {
      const sn = await api.clientSnippets();
      if (sn) await copy(sn.toml);
    } catch (e) {
      setError(String(e));
    }
  };

  // Lo stato arriva ogni secondo come oggetto nuovo: la fascia si rifà solo se cambia il contenuto,
  // altrimenti i pulsanti perderebbero fuoco e title a ogni giro.
  const specs = createMemo(() => engineAlerts(s()), [], { equals: (x, y) => JSON.stringify(x) === JSON.stringify(y) });
  const alerts = createMemo((): AlertItem[] =>
    specs().map((a) => {
      let actions: JSX.Element | undefined;
      if (a.action === "restart")
        actions = (
          <button
            class="btn sm"
            disabled={!props.engine.canRestart}
            title={props.engine.inUseTitle || "Stesso profilo e stesse differenze, nuovo avvio"}
            onClick={() => props.engine.restart()}
          >
            Riavvia con la stessa riga
          </button>
        );
      if (a.action === "benchmark")
        actions = (
          <button class="btn sm" onClick={() => props.onGo("benchmark")}>
            Vedi in Benchmark
          </button>
        );
      if (a.action === "budget")
        actions = (
          <button class="btn sm" onClick={() => props.onGo("impostazioni", "client")}>
            Budget di contesto
          </button>
        );
      return { key: a.key, tone: a.tone, title: a.title, detail: a.detail, actions };
    }),
  );

  return (
    <section>
      <Head
        title="Motore"
        sub={
          <>
            Il <code>llama-server</code> acceso da Aethera, misurato mentre serve. Tutto quello che vedi qui è misurato dopo
            l'avvio; quello che non è misurato è «sconosciuto».
          </>
        }
      >
        <button class="btn sm" disabled={!api.isEngineOn(s())} onClick={copyClient} title="Il profile.toml di Nonio, derivato dall'avvio acceso">
          Copia riga per i client
        </button>
        <button class="btn sm" disabled={!run()} onClick={() => copy(run()!.command_line)}>
          Copia riga di comando
        </button>
        <Show when={run()}>
          <button
            class="btn sm"
            classList={{ primary: exited() }}
            disabled={!props.engine.canRestart}
            title={props.engine.inUseTitle || "Stesso profilo e stesse differenze, nuovo avvio"}
            onClick={() => props.engine.restart()}
          >
            Riavvia con la stessa riga
          </button>
        </Show>
        <Show when={api.isEngineOn(s())}>
          <button class="btn sm danger" disabled={!props.engine.canStop} title={props.engine.inUseTitle} onClick={() => props.engine.stop()}>
            Ferma
          </button>
        </Show>
      </Head>

      <Show when={error()}>
        <div class="note err mb">{error()}</div>
      </Show>

      <Show when={setup() && !setup()!.complete}>
        <PrimoAvvio setup={setup()!} onGo={props.onGo} />
      </Show>

      <Show when={s()?.state === "orphan" && s()}>
        {(o) => <OrphanCard orphan={(o() as Extract<EngineStatus, { state: "orphan" }>).orphan} />}
      </Show>

      <Show
        when={run()}
        fallback={
          <Show when={s()?.state !== "orphan"}>
            <Empty
              title="Nessun motore acceso da Aethera."
              action={
                <button class="btn sm primary" onClick={() => props.onGo("avvio")}>
                  Vai ad Avvio
                </button>
              }
            >
              <div>
                Questa pagina misura un <code>llama-server</code> mentre gira: memoria occupata davvero, velocità recenti,
                quota di cache del prefisso, chi lo sta usando. Finché non ne accendi uno non c'è niente da misurare — e
                quello che non è misurato resta «sconosciuto», non zero.
              </div>
            </Empty>
          </Show>
        }
      >
        {(r) => (
          <>
            <Show when={s()?.state !== "orphan"}>
              <StatusCard s={s()!} run={r()} engine={props.engine} />
            </Show>

            <ServicesCard />

            <Show when={exited()}>
              <div class="card mb">
                <h2>
                  Perché è uscito <span class="r">dalle righe del log, non dal codice di uscita</span>
                </h2>
                <Show
                  when={failure().length}
                  fallback={
                    <div class="cond">
                      Il log non contiene nessuna riga riconoscibile come causa. Resta il codice di uscita qui sopra e il log
                      per intero in fondo alla pagina: meglio dire che non si sa, che indicare una causa sbagliata.
                    </div>
                  }
                >
                  <For each={failure()}>
                    {(x) => (
                      <div style={{ "margin-bottom": "8px" }}>
                        <pre class="cmd" style={{ margin: "0 0 4px" }}>
                          {x.line}
                        </pre>
                        <Show when={x.hint}>
                          <div class="note warn">{x.hint}</div>
                        </Show>
                      </div>
                    )}
                  </For>
                </Show>
              </div>
            </Show>

            <Alerts items={alerts()} />

            <Show when={ready()}>
              {(rd) => (
                <>
                  <KpiRow rd={rd()} />
                  <div class="grid g21 lw mb" style={{ "align-items": "start" }}>
                    <TurnsCard summary={rd().telemetry} />
                    <div class="grid" style={{ "align-items": "start" }}>
                      <MemoryCard
                        memory={rd().memory}
                        loadMs={rd().load_ms}
                        vgm={vgmOf(rd().conditions, props.overview)}
                        ramTotal={props.overview.system.ram_total_gib}
                      />
                      <ConditionsCard conditions={rd().conditions} changed={rd().conditions_changed} />
                    </div>
                  </div>
                </>
              )}
            </Show>

            <div class="grid mb" style={{ gap: "6px" }}>
              <details>
                <summary>
                  Riga di comando <span class="cond">esatta, come lanciata</span>
                  <span class="r row" style={{ gap: "4px" }}>
                    <Show when={r().overrides.length} fallback="uguale al profilo">
                      {r().overrides.length} {r().overrides.length === 1 ? "differenza" : "differenze"} dal profilo
                      <For each={r().overrides.slice(0, 3)}>
                        {(o) => (
                          <span class="pill mod">
                            {o.field.split(".").pop()} {o.base ?? "—"} → {o.value ?? "—"}
                          </span>
                        )}
                      </For>
                    </Show>
                    <Show when={r().invalidates_cache}>
                      <span class="pill cache">invalida la cache</span>
                    </Show>
                  </span>
                </summary>
                <div class="body">
                  <pre class="cmd" style={{ "margin-top": "8px" }}>
                    {r().command_line}
                  </pre>
                  <div class="row" style={{ "margin-top": "8px" }}>
                    <button class="btn sm" onClick={() => copy(r().command_line)}>
                      Copia riga
                    </button>
                    <button class="btn sm" onClick={() => copy(r().manifest_path)} title={r().manifest_path}>
                      Copia percorso manifest
                    </button>
                    <span class="cond mono right" style={{ "overflow-wrap": "anywhere" }}>
                      {r().manifest_path}
                    </span>
                  </div>
                  <Show when={r().overrides.length}>
                    <hr />
                    <h2>Differenze dal profilo</h2>
                    <table>
                      <tbody>
                        <For each={r().overrides}>
                          {(o) => (
                            <tr>
                              <td class="mono">{o.field}</td>
                              <td class="mono">
                                <s>{o.base ?? "—"}</s>
                              </td>
                              <td class="mono">{o.value ?? "—"}</td>
                            </tr>
                          )}
                        </For>
                      </tbody>
                    </table>
                  </Show>
                </div>
              </details>
              <details open={exited()}>
                <summary>
                  Log del motore{" "}
                  <span class="mono cond" style={{ "overflow-wrap": "anywhere" }}>
                    {r().log_path}
                  </span>
                  <span class="r">si apre da solo dopo un'uscita con errore</span>
                </summary>
                <div class="body">
                  <pre class="log" style={{ "margin-top": "8px" }}>
                    {log() || "…"}
                  </pre>
                </div>
              </details>
            </div>
          </>
        )}
      </Show>
    </section>
  );
}
