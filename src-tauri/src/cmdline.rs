//! Composizione della riga di comando di `llama-server` da un profilo.
//!
//! Flag lunghi, nell'ordine di `serve.ps1` di minis-config; poi le leve che `serve.ps1` lasciava al
//! default del motore o in `extra_args`. `--load-mode` è sempre esplicito: nessuna leva nascosta.

use crate::profile::Profile;
use serde::Serialize;
use std::path::{Path, PathBuf};

pub struct LaunchPaths {
    pub model: PathBuf,
    /// Presente solo se il profilo chiede il salvataggio degli slot.
    pub slot_dir: Option<PathBuf>,
    pub draft_model: Option<PathBuf>,
    /// Percorso completo del template di chat, se il profilo ne chiede uno.
    pub chat_template: Option<PathBuf>,
}

fn kv(a: &mut Vec<String>, flag: &str, value: impl ToString) {
    a.push(flag.to_string());
    a.push(value.to_string());
}

pub fn build_args(p: &Profile, paths: &LaunchPaths) -> Vec<String> {
    let s = &p.server;
    let mut a = Vec::new();
    kv(&mut a, "--model", paths.model.display());
    kv(&mut a, "--host", &s.host);
    kv(&mut a, "--port", s.port);
    kv(&mut a, "--ctx-size", s.ctx);
    kv(&mut a, "--parallel", s.n_parallel);
    // Senza n_gpu_layers i layer li sceglie --fit: si scrive solo quello che il profilo dice.
    if let Some(n) = s.n_gpu_layers {
        kv(&mut a, "--n-gpu-layers", n);
    }
    if let Some(f) = &s.fit {
        kv(&mut a, "--fit", f);
    }
    if let Some(t) = &s.fit_target {
        kv(&mut a, "--fit-target", t);
    }
    kv(&mut a, "--ubatch-size", s.ubatch);
    kv(&mut a, "--batch-size", s.batch);
    kv(&mut a, "--flash-attn", &s.flash_attn);
    kv(&mut a, "--cache-type-k", &s.cache_type_k);
    kv(&mut a, "--cache-type-v", &s.cache_type_v);
    kv(&mut a, "--alias", &p.name);
    if let Some(dir) = &paths.slot_dir {
        kv(&mut a, "--slot-save-path", dir.display());
    }
    if s.metrics {
        a.push("--metrics".into());
    }
    // --jinja è il default di questa build, ma il default può cambiare: si dichiara in entrambi i sensi.
    a.push(if s.jinja { "--jinja" } else { "--no-jinja" }.into());
    kv(&mut a, "--load-mode", &s.load_mode);
    if let Some(t) = s.threads {
        kv(&mut a, "--threads", t);
    }
    if let Some(t) = s.threads_batch {
        kv(&mut a, "--threads-batch", t);
    }
    if let Some(n) = s.n_cpu_moe {
        kv(&mut a, "--n-cpu-moe", n);
    }
    if !s.tensor_overrides.is_empty() {
        kv(&mut a, "--override-tensor", s.tensor_overrides.join(","));
    }
    if let Some(l) = &s.lazy_mode {
        kv(&mut a, "--lazy-mode", l);
    }
    if let Some(t) = &paths.chat_template {
        kv(&mut a, "--chat-template-file", t.display());
    }
    let sp = &p.speculative;
    if sp.kind != "none" {
        kv(&mut a, "--spec-type", &sp.kind);
        if let Some(n) = sp.draft_n_max {
            kv(&mut a, "--spec-draft-n-max", n);
        }
        if let Some(n) = sp.draft_n_min {
            kv(&mut a, "--spec-draft-n-min", n);
        }
        if let Some(x) = sp.draft_p_min {
            kv(&mut a, "--spec-draft-p-min", x);
        }
        if let Some(m) = &paths.draft_model {
            kv(&mut a, "--spec-draft-model", m.display());
        }
    }
    if let Some(n) = p.cache.cache_reuse {
        kv(&mut a, "--cache-reuse", n);
    }
    if let Some(n) = p.cache.ctx_checkpoints {
        kv(&mut a, "--ctx-checkpoints", n);
    }
    if let Some(n) = p.cache.checkpoint_min_step {
        kv(&mut a, "--checkpoint-min-step", n);
    }
    if let Some(n) = p.cache.cache_ram {
        kv(&mut a, "--cache-ram", n);
    }
    if let Some(u) = p.cache.kv_unified {
        a.push(if u { "--kv-unified" } else { "--no-kv-unified" }.into());
    }
    a.extend(s.extra_args.iter().cloned());
    a
}

/// Riga leggibile e copiabile: gli argomenti con spazi fra virgolette, come `serve.ps1`.
pub fn render_line(binary: &Path, args: &[String]) -> String {
    let quoted: Vec<String> = args
        .iter()
        .map(|x| if x.contains(char::is_whitespace) { format!("\"{x}\"") } else { x.clone() })
        .collect();
    format!("\"{}\" {}", binary.display(), quoted.join(" "))
}

/// Divide una riga resa da `render_line` (o scritta da `serve.ps1`) negli argomenti.
pub fn split_line(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut has = false;
    for c in line.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                has = true;
            }
            c if c.is_whitespace() && !quoted => {
                if has {
                    out.push(std::mem::take(&mut cur));
                    has = false;
                }
            }
            c => {
                cur.push(c);
                has = true;
            }
        }
    }
    if has {
        out.push(cur);
    }
    out
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ArgStatus {
    Same,
    Changed,
    Added,
    Removed,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ArgGroup {
    pub flag: String,
    pub value: Option<String>,
    pub base: Option<String>,
    pub status: ArgStatus,
}

fn is_value(token: &str) -> bool {
    !token.starts_with('-') || token.parse::<f64>().is_ok()
}

/// Raggruppa gli argomenti in coppie flag → valore.
pub fn group(args: &[String]) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let flag = args[i].clone();
        match args.get(i + 1) {
            Some(next) if flag.starts_with('-') && is_value(next) => {
                out.push((flag, Some(next.clone())));
                i += 2;
            }
            _ => {
                out.push((flag, None));
                i += 1;
            }
        }
    }
    out
}

/// Confronta la riga del profilo base con quella modificata, per evidenziare le differenze.
pub fn compare(base: &[String], edited: &[String]) -> Vec<ArgGroup> {
    let mut base_groups: Vec<Option<(String, Option<String>)>> = group(base).into_iter().map(Some).collect();
    let mut out = Vec::new();
    for (flag, value) in group(edited) {
        let hit = base_groups.iter_mut().find(|g| matches!(g, Some((f, _)) if *f == flag));
        match hit {
            Some(slot) => {
                let (_, base_value) = slot.take().unwrap();
                let status = if base_value == value { ArgStatus::Same } else { ArgStatus::Changed };
                out.push(ArgGroup { flag, value, base: base_value, status });
            }
            None => out.push(ArgGroup { flag, value, base: None, status: ArgStatus::Added }),
        }
    }
    for (flag, base_value) in base_groups.into_iter().flatten() {
        out.push(ArgGroup { flag, value: None, base: base_value, status: ArgStatus::Removed });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn split_respects_quotes() {
        assert_eq!(split_line(r#""C:\a b\x.exe" --model "C:\m m\f.gguf" --metrics"#), s(&[r"C:\a b\x.exe", "--model", r"C:\m m\f.gguf", "--metrics"]));
    }

    #[test]
    fn compare_marks_changes() {
        let base = s(&["--ubatch-size", "4096", "--metrics", "--n-gpu-layers", "-1"]);
        let edited = s(&["--ubatch-size", "2048", "--metrics", "--n-gpu-layers", "-1", "--cache-reuse", "256"]);
        let st: Vec<ArgStatus> = compare(&base, &edited).into_iter().map(|g| g.status).collect();
        assert_eq!(st, vec![ArgStatus::Changed, ArgStatus::Same, ArgStatus::Same, ArgStatus::Added]);
    }

    #[test]
    fn measured_levers_reach_the_command_line() {
        let mut p = crate::profile::template("g3", "x.gguf", "b10991", "vulkan", 8080);
        let paths = LaunchPaths {
            model: PathBuf::from(r"C:\m\x.gguf"),
            slot_dir: None,
            draft_model: None,
            chat_template: Some(PathBuf::from(r"C:\AetheraData\templates\qwen3.6-tollerante.jinja")),
        };
        p.server.n_gpu_layers = None;
        p.server.fit = Some("on".into());
        p.server.fit_target = Some("1024".into());
        p.server.n_cpu_moe = Some(4);
        p.server.tensor_overrides = s(&[r"blk\.1\.ffn=CPU", "exps=Vulkan0"]);
        p.server.lazy_mode = Some("off".into());
        p.cache.checkpoint_min_step = Some(128);
        p.cache.cache_ram = Some(-1);
        p.cache.kv_unified = Some(false);
        let groups = group(&build_args(&p, &paths));
        let get = |f: &str| groups.iter().find(|(flag, _)| flag == f).map(|(_, v)| v.clone());
        assert_eq!(get("--n-gpu-layers"), None, "senza n_gpu_layers decide --fit");
        assert_eq!(get("--fit"), Some(Some("on".into())));
        assert_eq!(get("--fit-target"), Some(Some("1024".into())));
        assert_eq!(get("--n-cpu-moe"), Some(Some("4".into())));
        assert_eq!(get("--override-tensor"), Some(Some(r"blk\.1\.ffn=CPU,exps=Vulkan0".into())));
        assert_eq!(get("--lazy-mode"), Some(Some("off".into())));
        assert_eq!(get("--chat-template-file"), Some(Some(r"C:\AetheraData\templates\qwen3.6-tollerante.jinja".into())));
        assert_eq!(get("--checkpoint-min-step"), Some(Some("128".into())));
        assert_eq!(get("--cache-ram"), Some(Some("-1".into())));
        assert_eq!(get("--no-kv-unified"), Some(None));
    }
}
