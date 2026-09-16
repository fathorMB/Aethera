//! Telemetria per richiesta in `runs/<id>/telemetry.jsonl`.
//!
//! Fonte fissata su b10809 (15-09): i tempi di ogni richiesta vengono dalle righe `print_timing` del
//! log del server (prefill, decode, token proposti e accettati), chiuse dalla riga `release`.
//!
//! I token riusati dalla cache si ricavano dal log stesso (M-09): la riga `release` dice quanti token
//! occupa lo slot a richiesta chiusa (`n_tokens`), cioè prompt intero più generati; tolti i generati e
//! quelli rielaborati resta la parte presa dalla cache. Verificato su b10809 e b10991 contro `/slots`:
//! coincide a meno di due token (su 18 richieste di Claude Code, avvio r-20260916-161015): l'ultimo
//! token generato entra nello slot solo se la richiesta finisce per lunghezza, e con la speculazione
//! lo slot può tenere un token proposto in più. Se il log non basta restano le due fonti di prima:
//! `/slots` letto mentre la richiesta è in corso, e la differenza di
//! `llamacpp:prompt_tokens_cached_total` quando nessuna richiesta nuova è partita nel frattempo.
//! Altrimenti i token dalla cache restano sconosciuti.
//!
//! Dalla stessa riga si legge quanto contesto aveva lo slot, e quindi quanto della conversazione
//! precedente una richiesta ha tenuto: è così che si riconosce una compattazione del client.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

pub const FILE: &str = "telemetry.jsonl";
/// Richieste considerate per le mediane «recenti».
pub const RECENT: usize = 20;
/// Sotto questa soglia di token elaborati il prefill misura l'avvio della richiesta, non la velocità.
pub const PREFILL_MIN_TOKENS: u32 = 128;
pub const DEGRADED_UPTIME_S: u64 = 24 * 3600;
pub const DEGRADED_DECODE_RATIO: f64 = 0.7;
const DEGRADED_MIN_REQUESTS: usize = 10;
const DEGRADED_LAST: usize = 5;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Record {
    pub at: String,
    pub task: i64,
    /// Token di prompt elaborati, esclusi quelli riusati dalla cache.
    pub prompt_n: u32,
    pub prompt_ms: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_tps: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_n: Option<u32>,
    pub gen_n: u32,
    pub gen_ms: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decode_tps: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft_n: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft_accepted: Option<u32>,
    /// Da dove viene `cache_n`: `log`, `slots` o `metrics`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<u32>,
    /// Token nello slot a richiesta chiusa: prompt intero più generati.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ctx_after: Option<u32>,
    /// Il client che teneva il lock sull'endpoint di Aethera, se era uno solo: il log non lo dice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<String>,
}

impl Record {
    pub fn prompt_total(&self) -> Option<u32> {
        self.cache_n.map(|c| c + self.prompt_n)
    }
    pub fn cache_share(&self) -> Option<f64> {
        let total = self.prompt_total()?;
        (total > 0).then(|| self.cache_n.unwrap_or(0) as f64 / total as f64)
    }
}

/// Tempi di una richiesta letti dal log, prima di attribuire la cache.
#[derive(Debug, Clone, PartialEq)]
pub struct Timing {
    pub task: i64,
    pub prompt_n: u32,
    pub prompt_ms: f64,
    pub prompt_tps: Option<f64>,
    pub gen_n: u32,
    pub gen_ms: f64,
    pub decode_tps: Option<f64>,
    pub draft_n: Option<u32>,
    pub draft_accepted: Option<u32>,
    pub slot: Option<u32>,
    pub ctx_after: Option<u32>,
}

impl Timing {
    /// Token presi dalla cache secondo il log: contesto a fine richiesta − generati − rielaborati.
    pub fn cache_from_log(&self) -> Option<u32> {
        let after = self.ctx_after? as i64;
        Some((after - self.gen_n as i64 - self.prompt_n as i64).max(0) as u32)
    }

    pub fn into_record(self, at: String, cache_n: Option<u32>, cache_from: Option<&str>, client: Option<String>) -> Record {
        Record {
            at,
            task: self.task,
            prompt_n: self.prompt_n,
            prompt_ms: self.prompt_ms,
            prompt_tps: self.prompt_tps,
            cache_n,
            gen_n: self.gen_n,
            gen_ms: self.gen_ms,
            decode_tps: self.decode_tps,
            draft_n: self.draft_n,
            draft_accepted: self.draft_accepted,
            cache_from: cache_n.and(cache_from).map(str::to_string),
            slot: self.slot,
            ctx_after: self.ctx_after,
            client,
        }
    }
}

#[derive(Default)]
struct Partial {
    prompt: Option<(f64, u32, Option<f64>)>,
    eval: Option<(f64, u32, Option<f64>)>,
    draft: Option<(u32, u32)>,
}

/// Numeri in un frammento di riga, nell'ordine: `831.87 ms / 36 tokens (...)` → 831.87, 36, ….
fn numbers(s: &str) -> Vec<f64> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in s.chars().chain(std::iter::once(' ')) {
        if c.is_ascii_digit() || (c == '.' && !cur.is_empty()) {
            cur.push(c);
        } else if !cur.is_empty() {
            if let Ok(n) = cur.trim_end_matches('.').parse() {
                out.push(n);
            }
            cur.clear();
        }
    }
    out
}

fn task_of(line: &str) -> Option<i64> {
    let i = line.find("| task ")? + "| task ".len();
    let s: String = line[i..].chars().take_while(|c| c.is_ascii_digit() || *c == '-').collect();
    s.parse().ok()
}

/// Il numero dopo `name =` in una riga: `n_tokens = 21134` → 21134.
fn field_after(line: &str, name: &str) -> Option<u32> {
    let i = line.find(name)? + name.len();
    let rest = line[i..].trim_start().strip_prefix('=')?.trim_start();
    rest.chars().take_while(char::is_ascii_digit).collect::<String>().parse().ok()
}

/// `slot      release: id  0 | task 85 | …` → 0.
fn slot_of(line: &str) -> Option<u32> {
    let i = line.find(": id ")? + ": id ".len();
    line[i..].trim_start().chars().take_while(char::is_ascii_digit).collect::<String>().parse().ok()
}

fn timing_triple(segment: &str) -> Option<(f64, u32, Option<f64>)> {
    let n = numbers(segment);
    Some((*n.first()?, *n.get(1)? as u32, n.get(3).copied()))
}

#[derive(Default)]
pub struct LogParser {
    pending: BTreeMap<i64, Partial>,
}

impl LogParser {
    /// Una riga del log; restituisce i tempi quando la richiesta è stata rilasciata.
    pub fn feed(&mut self, line: &str) -> Option<Timing> {
        if line.contains("print_timing:") {
            let task = task_of(line)?;
            let p = self.pending.entry(task).or_default();
            if let Some(i) = line.find("prompt eval time =") {
                p.prompt = timing_triple(&line[i + "prompt eval time =".len()..]);
            } else if let Some(i) = line.find("| task").and_then(|t| line[t..].find(" eval time =").map(|j| t + j)) {
                p.eval = timing_triple(&line[i + " eval time =".len()..]);
            } else if let Some(i) = line.find("draft acceptance =") {
                let n = numbers(&line[i..]);
                if let (Some(acc), Some(gen)) = (n.get(1), n.get(2)) {
                    p.draft = Some((*gen as u32, *acc as u32));
                }
            }
            return None;
        }
        if line.contains("release:") && line.contains("stop processing") {
            let task = task_of(line)?;
            let p = self.pending.remove(&task)?;
            let (prompt_ms, prompt_n, prompt_tps) = p.prompt?;
            let (gen_ms, gen_n, decode_tps) = p.eval?;
            return Some(Timing {
                task,
                prompt_n,
                prompt_ms,
                prompt_tps,
                gen_n,
                gen_ms,
                decode_tps,
                draft_n: p.draft.map(|d| d.0),
                draft_accepted: p.draft.map(|d| d.1),
                slot: slot_of(line),
                ctx_after: field_after(line, "n_tokens"),
            });
        }
        None
    }
}

/// `slot launch_slot_: id  0 | task 68 | processing task`: una richiesta è partita.
pub fn is_launch(line: &str) -> bool {
    line.contains("launch_slot_") && line.contains("processing task")
}

/// Token dalla cache delle richieste in corso, per task, da `/slots`.
pub fn slot_caches(v: &Value) -> Vec<(i64, u32)> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter(|s| s["is_processing"].as_bool() == Some(true))
        .filter_map(|s| Some((s["id_task"].as_i64().filter(|t| *t >= 0)?, s["n_prompt_tokens_cache"].as_u64()? as u32)))
        .collect()
}

/// Legge le righe nuove di un file che cresce, tenendo da parte l'ultima riga incompleta.
pub struct LogTail {
    path: PathBuf,
    offset: u64,
    carry: String,
}

impl LogTail {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into(), offset: 0, carry: String::new() }
    }

    pub fn read_lines(&mut self) -> Vec<String> {
        let Ok(mut f) = File::open(&self.path) else { return Vec::new() };
        let len = f.metadata().map(|m| m.len()).unwrap_or(0);
        if len < self.offset {
            self.offset = 0;
            self.carry.clear();
        }
        if len == self.offset || f.seek(SeekFrom::Start(self.offset)).is_err() {
            return Vec::new();
        }
        let mut buf = Vec::new();
        if f.take(4 * 1024 * 1024).read_to_end(&mut buf).is_err() {
            return Vec::new();
        }
        self.offset += buf.len() as u64;
        self.carry.push_str(&String::from_utf8_lossy(&buf));
        let mut lines: Vec<String> = self.carry.split('\n').map(|l| l.trim_end_matches('\r').to_string()).collect();
        self.carry = lines.pop().unwrap_or_default();
        lines
    }
}

/// Contatori senza etichette di `/metrics`, per nome senza il prefisso `llamacpp:`.
pub fn parse_metrics(text: &str) -> BTreeMap<String, f64> {
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.contains('{'))
        .filter_map(|l| {
            let (name, value) = l.trim().split_once(' ')?;
            Some((name.trim_start_matches("llamacpp:").to_string(), value.trim().parse().ok()?))
        })
        .collect()
}

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq)]
pub struct Counters {
    pub prompt_tokens: f64,
    pub cached_tokens: f64,
    pub predicted_tokens: f64,
}

impl Counters {
    pub fn from_metrics(m: &BTreeMap<String, f64>) -> Option<Self> {
        Some(Self {
            prompt_tokens: *m.get("prompt_tokens_total")?,
            cached_tokens: *m.get("prompt_tokens_cached_total")?,
            predicted_tokens: m.get("tokens_predicted_total").copied().unwrap_or(0.0),
        })
    }
}

/// Token dalla cache di un gruppo di richieste chiuse fra due letture di `/metrics`.
pub fn attribute_cache(timings: &[Timing], before: Option<Counters>, now: Option<Counters>) -> Vec<Option<u32>> {
    let (Some(b), Some(n)) = (before, now) else { return vec![None; timings.len()] };
    let processed: u64 = timings.iter().map(|t| t.prompt_n as u64).sum();
    let d_prompt = n.prompt_tokens - b.prompt_tokens;
    let d_cached = n.cached_tokens - b.cached_tokens;
    if timings.len() == 1 && d_prompt >= 0.0 && d_cached >= 0.0 && (d_prompt - processed as f64).abs() < 0.5 {
        vec![Some(d_cached as u32)]
    } else {
        vec![None; timings.len()]
    }
}

pub fn append(run_dir: &Path, r: &Record) -> Result<(), String> {
    let path = run_dir.join(FILE);
    let mut f = OpenOptions::new().create(true).append(true).open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let line = serde_json::to_string(r).map_err(|e| e.to_string())?;
    writeln!(f, "{line}").map_err(|e| format!("{}: {e}", path.display()))
}

/// Righe illeggibili si saltano: un file troncato da uno spegnimento non cancella lo storico.
pub fn read(run_dir: &Path) -> Vec<Record> {
    let Ok(f) = File::open(run_dir.join(FILE)) else { return Vec::new() };
    BufReader::new(f).lines().map_while(Result::ok).filter_map(|l| serde_json::from_str(&l).ok()).collect()
}

/// Una richiesta che tiene almeno questa quota del contesto precedente lo «estende».
pub const EXTEND_KEEP: f64 = 0.9;
/// Un prompt più corto di questa quota del contesto precedente è un contesto ricostruito.
pub const REBUILT_SHRINK: f64 = 0.7;
/// Margine oltre all'ultimo ubatch, che il motore può rielaborare anche a prompt invariato (M-08 T-13).
const EXTEND_SLACK: u32 = 64;

/// Come una richiesta ha trattato la conversazione che lo slot aveva già.
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TurnKind {
    /// Niente dalla cache.
    Cold,
    /// Riprende la conversazione precedente e aggiunge in coda.
    Extends,
    /// Tiene solo una parte della conversazione: il prompt è cambiato prima della coda.
    Rewrites,
    /// Il riassunto di una compattazione: una richiesta che riscrive, seguita da un contesto ricostruito.
    Summary,
    /// Il prompt dopo una compattazione: più corto della conversazione di prima.
    Rebuilt,
    /// Mancano i numeri per dirlo.
    Unknown,
}

/// Una richiesta nella tabella della pagina Motore.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Turn {
    pub at: String,
    pub task: i64,
    pub client: Option<String>,
    pub prompt_total: Option<u32>,
    pub cache_n: Option<u32>,
    pub prompt_n: u32,
    pub prompt_ms: f64,
    pub gen_ms: f64,
    pub decode_tps: Option<f64>,
    pub kind: TurnKind,
}

/// Una compattazione del client. Il tempo è prefill e generazione del riassunto più il prefill del
/// contesto ricostruito: quello che la conversazione non avrebbe pagato continuando a estendersi.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Compaction {
    pub at: String,
    pub tasks: Vec<i64>,
    pub reprocessed: u64,
    pub cost_ms: f64,
    pub client: Option<String>,
}

/// Classifica ogni richiesta rispetto alla precedente sullo stesso slot. `ubatch` è quello
/// dell'avvio: una richiesta che estende può rielaborare un ubatch intero, e non per questo riscrive.
///
/// Le soglie non sono quelle del mockup (riuso sotto il 5% dopo richieste sopra il 90%): Claude Code
/// conserva il suo prompt fisso anche quando compatta, quindi il riuso resta al 40-50%. Quello che
/// cambia davvero è quanta parte della conversazione precedente sopravvive, e se il prompt si accorcia.
pub fn classify(records: &[Record], ubatch: Option<u32>) -> Vec<TurnKind> {
    let mut last_ctx: BTreeMap<Option<u32>, u32> = BTreeMap::new();
    let mut kinds: Vec<TurnKind> = Vec::with_capacity(records.len());
    for r in records {
        let kind = match (r.cache_n, last_ctx.get(&r.slot).copied()) {
            (None, _) => TurnKind::Unknown,
            (Some(c), _) if c <= 1 => TurnKind::Cold,
            (Some(_), None) => TurnKind::Unknown,
            (Some(c), Some(prev)) => {
                let slack = ubatch.unwrap_or(0) + EXTEND_SLACK;
                if c as f64 >= prev as f64 * EXTEND_KEEP || c + slack >= prev {
                    TurnKind::Extends
                } else if ((c + r.prompt_n) as f64) < prev as f64 * REBUILT_SHRINK {
                    TurnKind::Rebuilt
                } else {
                    TurnKind::Rewrites
                }
            }
        };
        if kind == TurnKind::Rebuilt {
            if let Some(k) = kinds.last_mut().filter(|k| **k == TurnKind::Rewrites) {
                *k = TurnKind::Summary;
            }
        }
        kinds.push(kind);
        if let Some(after) = r.ctx_after {
            last_ctx.insert(r.slot, after);
        }
    }
    kinds
}

pub fn compactions(records: &[Record], kinds: &[TurnKind]) -> Vec<Compaction> {
    let mut out = Vec::new();
    for (i, (r, k)) in records.iter().zip(kinds).enumerate() {
        if *k != TurnKind::Rebuilt {
            continue;
        }
        let mut c = Compaction {
            at: r.at.clone(),
            tasks: vec![r.task],
            reprocessed: r.prompt_n as u64,
            cost_ms: r.prompt_ms,
            client: r.client.clone(),
        };
        if let Some(s) = i.checked_sub(1).filter(|j| kinds[*j] == TurnKind::Summary).map(|j| &records[j]) {
            c.at = s.at.clone();
            c.tasks.insert(0, s.task);
            c.reprocessed += s.prompt_n as u64;
            c.cost_ms += s.prompt_ms + s.gen_ms;
            c.client = c.client.or_else(|| s.client.clone());
        }
        out.push(c);
    }
    out
}

fn percentile(values: &[f64], p: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut v = values.to_vec();
    v.sort_by(|a, b| a.total_cmp(b));
    let rank = p * (v.len() - 1) as f64;
    let (lo, hi) = (rank.floor() as usize, rank.ceil() as usize);
    Some(v[lo] + (v[hi] - v[lo]) * (rank - lo as f64))
}

pub fn median(values: &[f64]) -> Option<f64> {
    percentile(values, 0.5)
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Summary {
    pub requests: usize,
    /// Richieste su cui sono calcolate mediane e quote.
    pub window: usize,
    pub prefill_median: Option<f64>,
    pub decode_median: Option<f64>,
    pub decode_p10: Option<f64>,
    pub decode_p90: Option<f64>,
    pub acceptance: Option<f64>,
    pub cache_share: Option<f64>,
    /// Quota di cache per richiesta nella finestra, dalla più vecchia.
    pub cache_series: Vec<Option<f64>>,
    pub decode_series: Vec<Option<f64>>,
    pub last_prompt: Option<u32>,
    pub draft_n: u64,
    pub draft_accepted: u64,
    pub prompt_processed: u64,
    pub prompt_cached: u64,
    pub generated: u64,
    /// Le richieste della finestra, con come hanno trattato la conversazione.
    pub turns: Vec<Turn>,
    /// Compattazioni chiuse nella finestra, con il tempo che sono costate.
    pub compactions: Vec<Compaction>,
    /// Da dove viene la cache delle richieste della finestra: `log`, `slots`, `metrics`.
    pub cache_sources: Vec<String>,
}

/// `window` = ultime N richieste; `usize::MAX` per l'intero avvio. `ubatch` è quello dell'avvio.
pub fn summarize(records: &[Record], window: usize, ubatch: Option<u32>) -> Summary {
    let from = records.len().saturating_sub(window);
    let w = &records[from..];
    // La classificazione parte dall'inizio: la prima richiesta della finestra ha una precedente.
    let kinds = classify(records, ubatch);
    let window_tasks: Vec<i64> = w.iter().map(|r| r.task).collect();
    let compactions: Vec<Compaction> = compactions(records, &kinds)
        .into_iter()
        .filter(|c| c.tasks.last().is_some_and(|t| window_tasks.contains(t)))
        .collect();
    let mut cache_sources: Vec<String> = w.iter().filter_map(|r| r.cache_from.clone()).collect();
    cache_sources.sort();
    cache_sources.dedup();
    let prefill: Vec<f64> = w.iter().filter(|r| r.prompt_n >= PREFILL_MIN_TOKENS).filter_map(|r| r.prompt_tps).collect();
    let decode: Vec<f64> = w.iter().filter_map(|r| r.decode_tps).collect();
    let draft_n: u64 = w.iter().filter_map(|r| r.draft_n).map(u64::from).sum();
    let draft_accepted: u64 = w.iter().filter_map(|r| r.draft_accepted).map(u64::from).sum();
    let known: Vec<&Record> = w.iter().filter(|r| r.cache_n.is_some()).collect();
    let cached: u64 = known.iter().filter_map(|r| r.cache_n).map(u64::from).sum();
    let processed_known: u64 = known.iter().map(|r| r.prompt_n as u64).sum();
    Summary {
        requests: records.len(),
        window: w.len(),
        prefill_median: median(&prefill),
        decode_median: median(&decode),
        decode_p10: percentile(&decode, 0.1),
        decode_p90: percentile(&decode, 0.9),
        acceptance: (draft_n > 0).then(|| draft_accepted as f64 / draft_n as f64),
        cache_share: (cached + processed_known > 0).then(|| cached as f64 / (cached + processed_known) as f64),
        cache_series: w.iter().map(Record::cache_share).collect(),
        decode_series: w.iter().map(|r| r.decode_tps).collect(),
        last_prompt: records.last().and_then(Record::prompt_total),
        draft_n,
        draft_accepted,
        prompt_processed: w.iter().map(|r| r.prompt_n as u64).sum(),
        prompt_cached: cached,
        generated: w.iter().map(|r| r.gen_n as u64).sum(),
        turns: w
            .iter()
            .zip(&kinds[from..])
            .map(|(r, k)| Turn {
                at: r.at.clone(),
                task: r.task,
                client: r.client.clone(),
                prompt_total: r.prompt_total(),
                cache_n: r.cache_n,
                prompt_n: r.prompt_n,
                prompt_ms: r.prompt_ms,
                gen_ms: r.gen_ms,
                decode_tps: r.decode_tps,
                kind: *k,
            })
            .collect(),
        compactions,
        cache_sources,
    }
}

/// Mediana del decode di riferimento, dagli avvii precedenti con stesso profilo, stessa build e
/// stesse condizioni (M-09): quando cambia un driver la mediana riparte.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Reference {
    pub decode_median: f64,
    pub runs: usize,
}

/// Motivi per cui un motore acceso è «degradato»: tempo di accensione o decode recente crollato.
/// Il decode recente si confronta con la mediana di riferimento quando ci sono avvii con le stesse
/// condizioni, altrimenti con la mediana dell'avvio stesso.
pub fn degraded(uptime_s: u64, records: &[Record], reference: Option<&Reference>) -> Vec<String> {
    let mut out = Vec::new();
    if uptime_s > DEGRADED_UPTIME_S {
        out.push(format!("acceso da più di {} h: prima di una misura conviene riavviare", DEGRADED_UPTIME_S / 3600));
    }
    let decode: Vec<f64> = records.iter().filter_map(|r| r.decode_tps).collect();
    let needed = if reference.is_some() { DEGRADED_LAST } else { DEGRADED_MIN_REQUESTS };
    if decode.len() < needed {
        return out;
    }
    let last = median(&decode[decode.len() - DEGRADED_LAST..]);
    let (base, of) = match reference {
        Some(r) => (
            Some(r.decode_median),
            format!(
                "della mediana di riferimento ({:.1}, {} {} con le stesse condizioni)",
                r.decode_median,
                r.runs,
                if r.runs == 1 { "avvio" } else { "avvii" }
            ),
        ),
        None => {
            let all = median(&decode);
            (all, format!("della mediana dell'avvio ({:.1})", all.unwrap_or(0.0)))
        }
    };
    if let (Some(base), Some(last)) = (base, last) {
        if last < base * DEGRADED_DECODE_RATIO {
            out.push(format!(
                "decode delle ultime {DEGRADED_LAST} richieste {last:.1} tok/s, sotto il {:.0} % {of}",
                DEGRADED_DECODE_RATIO * 100.0
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // Righe vere di b10809 (G1, 15-09): una richiesta senza cache e una con 14.034 token dalla cache.
    const LOG: &str = "\
5.03.785.100 I slot launch_slot_: id  0 | task 73 | processing task, is_child = 0
5.49.221.560 I slot print_timing: id  0 | task 73 | prompt eval time =   45118.68 ms / 14057 tokens (    3.21 ms per token,   311.56 tokens per second)
5.49.221.564 I slot print_timing: id  0 | task 73 |        eval time =     318.28 ms /     8 tokens (   45.47 ms per token,    21.99 tokens per second)
5.49.221.566 I slot print_timing: id  0 | task 73 |       total time =   45436.96 ms / 14065 tokens
5.49.221.573 I slot print_timing: id  0 | task 73 | draft acceptance = 0.66667 (    4 accepted /     6 generated), mean len =  3.00
5.49.221.873 I slot      release: id  0 | task 73 | stop processing: n_tokens = 14064, truncated = 0
5.49.422.855 I slot get_availabl: id  0 | task -1 | selected slot by LCP similarity, f_sim_best = 0.999 (> 0.100 thold), f_keep = 0.999
5.52.100.000 I slot print_timing: id  0 | task 85 | n_gen =    102, tg =  29.76 t/s, tg_3s =  30.05 t/s
5.52.232.436 I slot print_timing: id  0 | task 85 | prompt eval time =    2492.18 ms /    23 tokens (  108.36 ms per token,     9.23 tokens per second)
5.52.232.442 I slot print_timing: id  0 | task 85 |        eval time =     316.94 ms /     8 tokens (   45.28 ms per token,    22.09 tokens per second)
5.52.232.641 I slot      release: id  0 | task 85 | stop processing: n_tokens = 14064, truncated = 0
";

    fn parse_all() -> Vec<Timing> {
        let mut p = LogParser::default();
        LOG.lines().filter_map(|l| p.feed(l)).collect()
    }

    #[test]
    fn parses_b10809_print_timing() {
        let t = parse_all();
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].task, 73);
        assert_eq!(t[0].prompt_n, 14057);
        assert_eq!(t[0].prompt_tps, Some(311.56));
        assert_eq!((t[0].gen_n, t[0].decode_tps), (8, Some(21.99)));
        assert_eq!((t[0].draft_n, t[0].draft_accepted), (Some(6), Some(4)));
        assert_eq!((t[1].prompt_n, t[1].draft_n), (23, None));
    }

    #[test]
    fn cache_is_attributed_only_when_counters_match() {
        let t = parse_all();
        let before = Some(Counters { prompt_tokens: 14126.0, cached_tokens: 0.0, predicted_tokens: 0.0 });
        let now = Some(Counters { prompt_tokens: 14149.0, cached_tokens: 14034.0, predicted_tokens: 0.0 });
        assert_eq!(attribute_cache(&t[1..], before, now), vec![Some(14034)]);
        // Una richiesta successiva ha già elaborato token: la differenza non è attribuibile.
        let later = Some(Counters { prompt_tokens: 14200.0, cached_tokens: 28000.0, predicted_tokens: 0.0 });
        assert_eq!(attribute_cache(&t[1..], before, later), vec![None]);
        assert_eq!(attribute_cache(&t, before, now), vec![None, None]);
    }

    #[test]
    fn slot_cache_only_while_processing() {
        // Letture vere durante due richieste di seguito (16-09): a slot libero il campo è già zero.
        let busy: Value = serde_json::from_str(r#"[{"id":0,"is_processing":true,"id_task":64,"n_prompt_tokens_processed":18,"n_prompt_tokens_cache":3973}]"#).unwrap();
        let idle: Value = serde_json::from_str(r#"[{"id":0,"is_processing":false,"id_task":64,"n_prompt_tokens_processed":0,"n_prompt_tokens_cache":0}]"#).unwrap();
        let fresh: Value = serde_json::from_str(r#"[{"id":0,"n_ctx":32768,"speculative":true,"is_processing":false}]"#).unwrap();
        assert_eq!(slot_caches(&busy), vec![(64, 3973)]);
        assert!(slot_caches(&idle).is_empty() && slot_caches(&fresh).is_empty());
        assert!(is_launch("0.41.816.708 I slot launch_slot_: id  0 | task 68 | processing task, is_child = 0"));
        assert!(!is_launch("0.41.800.217 I slot      release: id  0 | task 3 | stop processing: n_tokens = 9271"));
    }

    #[test]
    fn metrics_and_summary() {
        let m = parse_metrics("# HELP x\nllamacpp:prompt_tokens_total 69\nllamacpp:prompt_tokens_cached_total 0\nllamacpp:spec_decode_num_accepted_tokens_per_pos_total{position=\"0\"} 60\n");
        assert_eq!(Counters::from_metrics(&m).unwrap().prompt_tokens, 69.0);
        let t = parse_all();
        let recs = vec![
            t[0].clone().into_record("a".into(), Some(0), Some("log"), None),
            t[1].clone().into_record("b".into(), Some(14034), Some("metrics"), None),
        ];
        let s = summarize(&recs, RECENT, Some(4096));
        assert_eq!(s.cache_sources, vec!["log".to_string(), "metrics".to_string()]);
        assert_eq!(s.prefill_median, Some(311.56), "il prefill da 23 token non entra nella mediana");
        assert_eq!(s.last_prompt, Some(14057));
        assert!((s.cache_share.unwrap() - 14034.0 / 28114.0).abs() < 1e-9);
        assert_eq!(s.acceptance, Some(4.0 / 6.0));
    }

    #[test]
    fn cache_from_the_release_line() {
        let t = parse_all();
        // 14064 − 8 − 14057 è −1: a cache vuota l'ultimo token generato non è nello slot.
        assert_eq!((t[0].slot, t[0].ctx_after, t[0].cache_from_log()), (Some(0), Some(14064), Some(0)));
        // /metrics diceva 14034: lo scarto è di un token.
        assert_eq!(t[1].cache_from_log(), Some(14033));
        let r = t[1].clone().into_record("b".into(), None, Some("log"), Some("nonio".into()));
        assert_eq!((r.cache_from, r.client.as_deref()), (None, Some("nonio")), "senza cache non c'è una fonte");
        assert_eq!(field_after("stop processing: n_tokens = 21134, truncated = 0", "n_tokens"), Some(21134));
        assert_eq!(field_after("n_tokens: nessuno", "n_tokens"), None);
    }

    fn rec(task: i64, prompt_n: u32, gen_n: u32, ctx_after: u32, prompt_ms: f64, gen_ms: f64) -> Record {
        let t = Timing {
            task,
            prompt_n,
            prompt_ms,
            prompt_tps: None,
            gen_n,
            gen_ms,
            decode_tps: None,
            draft_n: None,
            draft_accepted: None,
            slot: Some(0),
            ctx_after: Some(ctx_after),
        };
        let cache = t.cache_from_log();
        t.into_record(format!("t{task}"), cache, Some("log"), None)
    }

    /// La sessione vera di Claude Code su G1 (M-08 T-10, avvio r-20260916-161015, -ub 4096).
    fn claude_code_session() -> Vec<Record> {
        vec![
            rec(1360, 16822, 27, 16848, 47838.62, 800.0),
            rec(1630, 4141, 205, 17073, 15991.73, 7000.0),
            rec(1773, 4234, 463, 21772, 17437.62, 16000.0),
            rec(2167, 567, 83, 21954, 4938.82, 3000.0),
            rec(2221, 1463, 2348, 25580, 7500.08, 90000.0),
            rec(3479, 21366, 323, 34415, 85168.75, 12000.0),
            rec(3979, 18884, 312, 35768, 79064.60, 12954.07),
            rec(4441, 7911, 497, 21134, 29109.53, 17805.02),
            rec(4776, 1571, 358, 23063, 8536.21, 13000.0),
        ]
    }

    #[test]
    fn a_claude_code_compaction_is_recognised() {
        use TurnKind::*;
        let recs = claude_code_session();
        // 1630 rielabora un ubatch intero ma tiene tutta la conversazione: estende, non riscrive.
        // 3479 è la ripresa con --continue: tiene solo il prompt fisso, ma il prompt non si accorcia.
        assert_eq!(classify(&recs, Some(4096)), vec![Cold, Extends, Extends, Extends, Extends, Rewrites, Summary, Rebuilt, Extends]);
        // Con il riuso sotto il 5% del mockup non si sarebbe visto niente: il prompt fisso resta.
        assert!(recs[6].cache_share().unwrap() > 0.4);
        let s = summarize(&recs, RECENT, Some(4096));
        assert_eq!(s.compactions.len(), 1);
        let c = &s.compactions[0];
        assert_eq!((c.tasks.clone(), c.reprocessed, c.at.as_str()), (vec![3979, 4441], 18884 + 7911, "t3979"));
        assert!((c.cost_ms - (79064.60 + 12954.07 + 29109.53)).abs() < 1e-6);
        assert_eq!(s.turns[7].kind, Rebuilt);
        assert_eq!(s.turns[7].prompt_total, Some(12726 + 7911));
    }

    #[test]
    fn the_window_keeps_the_request_before_it() {
        let recs = claude_code_session();
        let s = summarize(&recs, 2, Some(4096));
        assert_eq!(s.turns.iter().map(|t| t.kind).collect::<Vec<_>>(), vec![TurnKind::Rebuilt, TurnKind::Extends]);
        assert_eq!(s.compactions.len(), 1, "la compattazione chiusa nella finestra si conta intera");
        let s = summarize(&recs, 1, Some(4096));
        assert!(s.compactions.is_empty());
        // Senza l'ubatch la richiesta che rielabora un ubatch intero sembra una riscrittura.
        assert_eq!(classify(&recs[..2], None)[1], TurnKind::Rewrites);
    }

    fn with_decode(d: f64) -> Record {
        let mut r = rec(0, 10, 10, 20, 1.0, 1.0);
        r.decode_tps = Some(d);
        r
    }

    #[test]
    fn degraded_by_uptime_and_decode() {
        let mut recs: Vec<Record> = (0..10).map(|_| with_decode(23.0)).collect();
        assert!(degraded(3600, &recs, None).is_empty());
        assert_eq!(degraded(25 * 3600, &recs, None).len(), 1);
        recs.extend((0..5).map(|_| with_decode(12.0)));
        assert_eq!(degraded(3600, &recs, None).len(), 1);
    }

    #[test]
    fn degraded_against_the_reference_of_the_same_conditions() {
        let recs: Vec<Record> = (0..5).map(|_| with_decode(20.0)).collect();
        assert!(degraded(3600, &recs, None).is_empty(), "cinque richieste non bastano per la mediana dell'avvio");
        let reference = Reference { decode_median: 30.9, runs: 3 };
        let d = degraded(3600, &recs, Some(&reference));
        assert_eq!(d.len(), 1);
        assert!(d[0].contains("mediana di riferimento (30.9, 3 avvii"), "{d:?}");
        assert!(degraded(3600, &recs, Some(&Reference { decode_median: 26.0, runs: 1 })).is_empty());
    }
}
