//! Telemetria per richiesta in `runs/<id>/telemetry.jsonl`.
//!
//! Fonte fissata su b10809 (15-09): i tempi di ogni richiesta vengono dalle righe `print_timing` del
//! log del server (prefill, decode, token proposti e accettati), chiuse dalla riga `release`. I token
//! riusati dalla cache non sono nel log: si leggono da `/slots` mentre la richiesta è in corso
//! (`id_task`, `n_prompt_tokens_cache`; a richiesta rilasciata il campo torna a zero). Per le richieste
//! più brevi di una lettura resta la differenza di `llamacpp:prompt_tokens_cached_total`, valida solo
//! se nessuna richiesta nuova è partita nel frattempo: quel contatore sale all'avvio della richiesta.
//! Altrimenti i token dalla cache restano sconosciuti.

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
}

impl Timing {
    pub fn into_record(self, at: String, cache_n: Option<u32>) -> Record {
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
}

/// `window` = ultime N richieste; `usize::MAX` per l'intero avvio.
pub fn summarize(records: &[Record], window: usize) -> Summary {
    let from = records.len().saturating_sub(window);
    let w = &records[from..];
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
    }
}

/// Motivi per cui un motore acceso è «degradato»: tempo di accensione o decode recente crollato.
pub fn degraded(uptime_s: u64, records: &[Record]) -> Vec<String> {
    let mut out = Vec::new();
    if uptime_s > DEGRADED_UPTIME_S {
        out.push(format!("acceso da più di {} h: prima di una misura conviene riavviare", DEGRADED_UPTIME_S / 3600));
    }
    let decode: Vec<f64> = records.iter().filter_map(|r| r.decode_tps).collect();
    if decode.len() >= DEGRADED_MIN_REQUESTS {
        let all = median(&decode);
        let last = median(&decode[decode.len() - DEGRADED_LAST..]);
        if let (Some(all), Some(last)) = (all, last) {
            if last < all * DEGRADED_DECODE_RATIO {
                out.push(format!(
                    "decode delle ultime {DEGRADED_LAST} richieste {last:.1} tok/s, sotto il {:.0} % della mediana dell'avvio ({all:.1})",
                    DEGRADED_DECODE_RATIO * 100.0
                ));
            }
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
        let recs = vec![t[0].clone().into_record("a".into(), Some(0)), t[1].clone().into_record("b".into(), Some(14034))];
        let s = summarize(&recs, RECENT);
        assert_eq!(s.prefill_median, Some(311.56), "il prefill da 23 token non entra nella mediana");
        assert_eq!(s.last_prompt, Some(14057));
        assert!((s.cache_share.unwrap() - 14034.0 / 28114.0).abs() < 1e-9);
        assert_eq!(s.acceptance, Some(4.0 / 6.0));
    }

    #[test]
    fn degraded_by_uptime_and_decode() {
        let rec = |d: f64| Record {
            at: String::new(),
            task: 0,
            prompt_n: 10,
            prompt_ms: 1.0,
            prompt_tps: None,
            cache_n: None,
            gen_n: 10,
            gen_ms: 1.0,
            decode_tps: Some(d),
            draft_n: None,
            draft_accepted: None,
        };
        let mut recs: Vec<Record> = (0..10).map(|_| rec(23.0)).collect();
        assert!(degraded(3600, &recs).is_empty());
        assert_eq!(degraded(25 * 3600, &recs).len(), 1);
        recs.extend((0..5).map(|_| rec(12.0)));
        assert_eq!(degraded(3600, &recs).len(), 1);
    }
}
