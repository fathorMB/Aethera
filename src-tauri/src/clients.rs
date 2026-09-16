//! Righe per i client, derivate dall'avvio acceso: `profile.toml` di Nonio, `opencode.json`, il
//! blocco d'ambiente per Claude Code e quello per banchi e Diorama; più il budget di contesto di
//! ogni client. Quello che il profilo non dice non si inventa.
//!
//! Il campionamento consigliato viaggia con le righe perché è il client a mandarlo in ogni
//! richiesta: se resta nel profilo del launcher non arriva a nessuno. Viaggia con la fonte e la
//! data di lettura, così chi incolla la riga sa se sta copiando un dato o un ricordo.

use crate::profile::{ClientBudget, Sampling};
use crate::telemetry::{Turn, TurnKind};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ClientSnippets {
    pub toml: String,
    pub env: String,
    pub powershell: String,
    pub opencode: String,
    pub claude_code_powershell: String,
    pub claude_code_bash: String,
    /// Il template di chat dell'avvio, se ne ha uno al posto di quello del modello.
    pub chat_template: Option<String>,
    /// L'avvio passa un template che accetta messaggi di sistema a metà conversazione.
    pub claude_code_ready: bool,
    pub budgets: Vec<Budget>,
}

/// Prompt fisso di un client (istruzioni di sistema e strumenti), misurato alla sua prima
/// richiesta a freddo, con da dove viene il numero.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FixedPrompt {
    pub client: String,
    pub tokens: u32,
    pub source: String,
}

/// Quanto spazio resta a un client per lavorare prima di compattare.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Budget {
    pub client: String,
    pub fixed_prompt: Option<u32>,
    pub fixed_source: Option<String>,
    pub reserved_output: Option<u32>,
    /// Contesto servito − prompt fisso − output riservato.
    pub workspace: Option<u32>,
    pub share: Option<f64>,
    /// Spazio di lavoro sotto `TIGHT_SHARE` del contesto: il client compatterà presto.
    pub tight: Option<bool>,
    /// Il contesto con cui lo spazio di lavoro arriva a `TIGHT_SHARE`.
    pub ctx_needed: Option<u32>,
}

/// Sotto questa quota del contesto lo spazio di lavoro di un client è stretto (soglia approvata
/// dall'operatore con il mockup di M-09).
pub const TIGHT_SHARE: f64 = 0.3;

/// Prompt fissi misurati su questo motore (M-08 T-10 e l'E2E di M-09). Valgono per quella versione
/// del client e per il tokenizer di Qwen3.6, e comprendono la prima domanda, breve: sono un dato con
/// la sua fonte, non una stima.
pub fn measured_fixed_prompts() -> Vec<FixedPrompt> {
    vec![
        FixedPrompt {
            client: "Claude Code".into(),
            tokens: 16_822,
            source: "M-08 T-10 · Claude Code 2.1.273 · Qwen3.6-35B-A3B · prima richiesta a freddo (16.816 nell'E2E di M-09)".into(),
        },
        FixedPrompt {
            client: "OpenCode".into(),
            tokens: 7_474,
            source: "E2E di M-09 (avvio r-20260916-173342) · OpenCode 1.18.31 · Qwen3.6-35B-A3B · richiesta a freddo dell'agente".into(),
        },
        FixedPrompt {
            client: "Nonio".into(),
            tokens: 2_147,
            source: "E2E di M-09 (avvio r-20260916-173342) · Nonio 0b23a16 · Qwen3.6-35B-A3B · prima richiesta a freddo (2.264 in un secondo giro)".into(),
        },
    ]
}

/// Il nome con cui un client compare nelle schede, dal nome che ha dato al lock.
pub fn display_name(lock_client: &str) -> String {
    let l = lock_client.to_ascii_lowercase();
    if l.contains("claude") {
        "Claude Code".into()
    } else if l.contains("opencode") {
        "OpenCode".into()
    } else if l.contains("nonio") {
        "Nonio".into()
    } else {
        lock_client.to_string()
    }
}

/// Prompt fissi misurati in questo avvio: per ogni client che ha preso il lock, la richiesta a
/// freddo **più grande**. La prima non basta: OpenCode apre con una richiesta breve per il titolo
/// della sessione (586 token nell'E2E di M-09), il prompt dell'agente arriva dopo (7.520).
pub fn fixed_from_turns(turns: &[Turn], run_id: &str) -> Vec<FixedPrompt> {
    let mut out: Vec<FixedPrompt> = Vec::new();
    for t in turns.iter().filter(|t| t.kind == TurnKind::Cold) {
        let (Some(client), Some(tokens)) = (&t.client, t.prompt_total) else { continue };
        let name = display_name(client);
        match out.iter_mut().find(|f| f.client == name) {
            Some(f) if f.tokens >= tokens => {}
            Some(f) => {
                f.tokens = tokens;
                f.source = format!("avvio {run_id} · richiesta a freddo più grande, alle {}", t.at);
            }
            None => out.push(FixedPrompt {
                client: name,
                tokens,
                source: format!("avvio {run_id} · richiesta a freddo più grande, alle {}", t.at),
            }),
        }
    }
    out
}

/// Il budget di ogni client noto: quelli con il prompt fisso misurato e quelli per cui c'è una riga.
pub fn budgets(ctx: u32, reserved_output: Option<u32>, fixed: &[FixedPrompt]) -> Vec<Budget> {
    let mut names: Vec<String> = vec!["Claude Code".into(), "OpenCode".into(), "Nonio".into()];
    for f in fixed {
        if !names.iter().any(|n| n.eq_ignore_ascii_case(&f.client)) {
            names.push(f.client.clone());
        }
    }
    names
        .into_iter()
        .map(|client| {
            let f = fixed.iter().find(|f| f.client.eq_ignore_ascii_case(&client));
            let fixed_prompt = f.map(|f| f.tokens);
            let workspace = match (fixed_prompt, reserved_output) {
                (Some(p), Some(o)) => Some(ctx.saturating_sub(p).saturating_sub(o)),
                _ => None,
            };
            let share = workspace.map(|w| w as f64 / ctx as f64);
            let ctx_needed = match (fixed_prompt, reserved_output) {
                (Some(p), Some(o)) => Some((((p + o) as f64 / (1.0 - TIGHT_SHARE)) / 1024.0).ceil() as u32 * 1024),
                _ => None,
            };
            Budget {
                client,
                fixed_source: f.map(|f| f.source.clone()),
                fixed_prompt,
                reserved_output,
                workspace,
                share,
                tight: share.map(|s| s < TIGHT_SHARE),
                ctx_needed,
            }
        })
        .collect()
}

pub struct RunFacts<'a> {
    pub run_id: &'a str,
    pub base_url: &'a str,
    pub alias: &'a str,
    pub ctx_declared: u32,
    pub ctx_served: Option<u32>,
    pub client: Option<&'a ClientBudget>,
    pub endpoint: Option<&'a str>,
    /// Campionamento consigliato del profilo avviato, per modalità.
    pub sampling: &'a BTreeMap<String, Sampling>,
    /// `chat_template_file` del profilo avviato.
    pub chat_template: Option<&'a str>,
    /// Cartella proposta per la configurazione separata di Claude Code.
    pub claude_config_dir: Option<String>,
    pub fixed_prompts: &'a [FixedPrompt],
}

/// Un template accetta i messaggi di sistema a metà conversazione se è uno di quelli tolleranti di Aethera.
pub fn is_tolerant(template: Option<&str>) -> bool {
    template.is_some_and(|t| t.to_ascii_lowercase().contains("tollerante"))
}

fn opencode_json(f: &RunFacts, ctx: u32) -> String {
    let mut limit = serde_json::json!({ "context": ctx });
    if let Some(c) = f.client {
        limit["output"] = c.reserved_output_tokens.into();
    }
    let v = serde_json::json!({
        "$schema": "https://opencode.ai/config.json",
        "provider": {
            "aethera": {
                "npm": "@ai-sdk/openai-compatible",
                "name": "Aethera",
                "options": { "baseURL": format!("{}/v1", f.base_url) },
                "models": { f.alias: { "name": f.alias, "limit": limit } }
            }
        },
        "model": format!("aethera/{}", f.alias)
    });
    serde_json::to_string_pretty(&v).unwrap_or_default()
}

/// Variabili per Claude Code, in ordine, con il commento che le accompagna.
fn claude_code_vars(f: &RunFacts) -> Vec<(&'static str, String, &'static str)> {
    let mut v = vec![
        ("ANTHROPIC_BASE_URL", f.base_url.to_string(), ""),
        ("ANTHROPIC_API_KEY", "aethera-locale".to_string(), "segnaposto: il motore non la controlla"),
        ("ANTHROPIC_MODEL", f.alias.to_string(), ""),
        ("ANTHROPIC_SMALL_FAST_MODEL", f.alias.to_string(), "un motore alla volta: anche i lavori brevi vanno qui"),
        ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1".to_string(), ""),
    ];
    if let Some(c) = f.client {
        v.push(("CLAUDE_CODE_MAX_OUTPUT_TOKENS", c.reserved_output_tokens.to_string(), "output riservato del profilo"));
    }
    v
}

fn claude_code(f: &RunFacts, powershell: bool) -> String {
    let line = |k: &str, val: &str, note: &str| {
        let base = if powershell { format!("$env:{k} = \"{val}\"") } else { format!("export {k}=\"{val}\"") };
        if note.is_empty() {
            base
        } else {
            format!("{base}   # {note}")
        }
    };
    let mut out: Vec<String> = claude_code_vars(f).iter().map(|(k, v, n)| line(k, v, n)).collect();
    if let Some(dir) = &f.claude_config_dir {
        let dir = if powershell { dir.clone() } else { dir.replace('\\', "/") };
        out.push(format!("# facoltativo: account, impostazioni e cronologia separati da quelli di tutti i giorni\n# {}", line("CLAUDE_CONFIG_DIR", &dir, "")));
    }
    if !is_tolerant(f.chat_template) {
        out.push(
            "# ATTENZIONE: questo avvio usa il template del modello. Con Qwen3.6 Claude Code riceve errore 500\n\
             # alla seconda richiesta: avvia un profilo con chat_template_file = \"qwen3.6-tollerante.jinja\"."
                .to_string(),
        );
    }
    out.push("claude".into());
    out.join("\n")
}

fn num(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{v:.1}")
    } else {
        format!("{v}")
    }
}

/// Le leve valorizzate di una modalità, nell'ordine in cui si leggono.
fn levers(s: &Sampling) -> Vec<(&'static str, String)> {
    let mut out: Vec<(&'static str, String)> = Vec::new();
    if let Some(v) = s.temperature {
        out.push(("temperature", num(v)));
    }
    if let Some(v) = s.top_p {
        out.push(("top_p", num(v)));
    }
    if let Some(v) = s.top_k {
        out.push(("top_k", v.to_string()));
    }
    if let Some(v) = s.min_p {
        out.push(("min_p", num(v)));
    }
    if let Some(v) = s.presence_penalty {
        out.push(("presence_penalty", num(v)));
    }
    if let Some(v) = s.repeat_penalty {
        out.push(("repeat_penalty", num(v)));
    }
    out
}

/// Fonte e data di una modalità, in una riga di commento. Senza fonte lo dice: un consiglio
/// senza provenienza vale meno di uno con la provenienza, e vale sapere quale dei due si ha.
fn provenance(s: &Sampling) -> String {
    match (&s.source, &s.verified) {
        (Some(src), Some(when)) => format!("fonte: {src} · letto il {when}"),
        (Some(src), None) => format!("fonte: {src} · data di lettura sconosciuta"),
        (None, Some(when)) => format!("fonte sconosciuta · letto il {when}"),
        (None, None) => "fonte e data sconosciute: valore scritto a mano nel profilo".into(),
    }
}

/// Blocchi `[sampling.<modalità>]` per il `profile.toml` del client.
fn sampling_toml(by_mode: &BTreeMap<String, Sampling>) -> String {
    let mut out = String::new();
    for (mode, s) in by_mode {
        let levers = levers(s);
        if levers.is_empty() {
            continue;
        }
        out.push_str(&format!("\n\n# {}\n[sampling.{mode}]\n", provenance(s)));
        out.push_str(&levers.iter().map(|(k, v)| format!("{k} = {v}")).collect::<Vec<_>>().join("\n"));
    }
    if !out.is_empty() {
        out = "\n\n# Campionamento consigliato dal modello: lo manda il client a ogni richiesta,\n\
               # llama-server non lo applica da solo."
            .to_string()
            + &out;
    }
    out
}

/// Le stesse righe come commenti, per chi incolla un blocco d'ambiente: un banco non legge TOML.
fn sampling_comments(by_mode: &BTreeMap<String, Sampling>) -> String {
    let mut out = String::new();
    for (mode, s) in by_mode {
        let levers = levers(s);
        if levers.is_empty() {
            continue;
        }
        out.push_str(&format!(
            "\n# campionamento {mode}: {}  ({})",
            levers.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(" "),
            provenance(s)
        ));
    }
    out
}

pub fn snippets(f: &RunFacts) -> ClientSnippets {
    // Nonio aggiunge da solo i percorsi (/props, /v1/chat/completions): vuole l'indirizzo nudo.
    // Con /v1 chiede /v1/props e riceve 404 (trovato dall'E2E di M-09).
    let toml = format!("[backend]\nkind = \"llamacpp\"\nbase_url = \"{}\"\n\n[model]\nexpected = \"{}\"", f.base_url, f.alias)
        + &sampling_toml(f.sampling);
    let mut vars: Vec<(&str, String)> = vec![("AETHERA_RUN_ID", f.run_id.to_string())];
    if let Some(e) = f.endpoint {
        vars.push(("AETHERA_ENDPOINT", format!("http://{e}")));
    }
    vars.push(("BENCH_MODEL", f.alias.to_string()));
    vars.push(("BENCH_CONTEXT", f.ctx_served.unwrap_or(f.ctx_declared).to_string()));
    vars.push(("BENCH_BASE_URL", f.base_url.to_string()));
    let ctx_note = match f.ctx_served {
        Some(_) => "# BENCH_CONTEXT = contesto servito su /props".to_string(),
        None => "# BENCH_CONTEXT = contesto dichiarato: quello servito non è ancora noto".to_string(),
    };
    // La parte fissa del banco (prompt di sistema, strumenti) è del banco: il launcher non la conosce.
    let conversation = match f.client {
        Some(c) => format!(
            "# BENCH_CONVERSATION = contesto − max_tokens − parte fissa del banco: non derivabile dal profilo\n#   (client.context_window {} · output riservato {})",
            c.context_window, c.reserved_output_tokens
        ),
        None => "# BENCH_CONVERSATION = contesto − max_tokens − parte fissa del banco: non derivabile dal profilo".to_string(),
    };
    let tail = format!("\n{ctx_note}\n{conversation}") + &sampling_comments(f.sampling);
    let env = vars.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("\n") + &tail;
    let powershell =
        vars.iter().map(|(k, v)| format!("$env:{k} = \"{v}\"")).collect::<Vec<_>>().join("\n") + &tail;
    let ctx = f.ctx_served.unwrap_or(f.ctx_declared);
    ClientSnippets {
        toml,
        env,
        powershell,
        opencode: opencode_json(f, ctx),
        claude_code_powershell: claude_code(f, true),
        claude_code_bash: claude_code(f, false),
        chat_template: f.chat_template.map(str::to_string),
        claude_code_ready: is_tolerant(f.chat_template),
        budgets: budgets(ctx, f.client.map(|c| c.reserved_output_tokens), f.fixed_prompts),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_code_block_follows_the_template_of_the_run() {
        let budget = ClientBudget { context_window: 28672, reserved_output_tokens: 4096 };
        let sampling = BTreeMap::new();
        let mut f = facts(&budget, &sampling);
        let s = snippets(&f);
        assert!(!s.claude_code_ready);
        assert!(s.claude_code_powershell.contains("errore 500"), "{}", s.claude_code_powershell);
        f.chat_template = Some("qwen3.6-tollerante.jinja");
        f.claude_config_dir = Some(r"C:\AetheraData\clients\claude-code".into());
        let s = snippets(&f);
        assert!(s.claude_code_ready);
        assert!(!s.claude_code_powershell.contains("errore 500"));
        assert!(s.claude_code_powershell.starts_with("$env:ANTHROPIC_BASE_URL = \"http://127.0.0.1:8080\"\n"));
        assert!(s.claude_code_powershell.contains("$env:CLAUDE_CODE_MAX_OUTPUT_TOKENS = \"4096\""));
        assert!(s.claude_code_powershell.ends_with("\nclaude"));
        assert!(s.claude_code_bash.contains("# export CLAUDE_CONFIG_DIR=\"C:/AetheraData/clients/claude-code\""), "{}", s.claude_code_bash);
        let oc: serde_json::Value = serde_json::from_str(&s.opencode).unwrap();
        let model = &oc["provider"]["aethera"]["models"]["qwen3.6-35b-a3b.q4_k_m.vulkan"];
        assert_eq!((model["limit"]["context"].as_u64(), model["limit"]["output"].as_u64()), (Some(32768), Some(4096)));
        assert_eq!(oc["provider"]["aethera"]["options"]["baseURL"], "http://127.0.0.1:8080/v1");
    }

    #[test]
    fn fixed_prompt_is_the_biggest_cold_request_of_each_client() {
        let turn = |client: Option<&str>, total: u32, kind: TurnKind| Turn {
            at: "t".into(),
            task: 0,
            client: client.map(str::to_string),
            prompt_total: Some(total),
            cache_n: Some(0),
            prompt_n: total,
            prompt_ms: 1.0,
            gen_ms: 1.0,
            decode_tps: None,
            kind,
        };
        // L'E2E di M-09: il titolo di OpenCode (586), poi il suo agente (7.520), poi un'estensione.
        let turns = vec![
            turn(Some("opencode"), 586, TurnKind::Cold),
            turn(Some("opencode"), 7520, TurnKind::Cold),
            turn(Some("opencode"), 7684, TurnKind::Extends),
            turn(Some("claude"), 16953, TurnKind::Cold),
            turn(None, 20000, TurnKind::Cold),
        ];
        let f = fixed_from_turns(&turns, "r-1");
        assert_eq!(f.iter().map(|x| (x.client.as_str(), x.tokens)).collect::<Vec<_>>(), vec![("OpenCode", 7520), ("Claude Code", 16953)]);
        let b = budgets(65536, Some(4096), &f);
        assert_eq!(b.len(), 3, "i nomi dei lock si allineano alle schede: {b:?}");
    }

    #[test]
    fn budget_is_known_only_where_it_was_measured() {
        let fixed = measured_fixed_prompts();
        let b = budgets(65536, Some(32000), &fixed);
        let cc = &b[0];
        assert_eq!((cc.client.as_str(), cc.fixed_prompt, cc.workspace), ("Claude Code", Some(16822), Some(16714)));
        assert_eq!(cc.tight, Some(true));
        // (16822 + 32000) / 0,7 = 69746, arrotondato ai 1024 token sopra: 69 × 1024 = 70656.
        assert_eq!(cc.ctx_needed, Some(70656));
        assert_eq!((b[1].client.as_str(), b[1].fixed_prompt), ("OpenCode", Some(7474)));
        // Un client che nessuno ha misurato resta sconosciuto.
        let b = budgets(65536, Some(32000), &fixed[..1]);
        assert_eq!((b[2].client.as_str(), b[2].fixed_prompt, b[2].tight), ("Nonio", None, None));
        let b = budgets(65536, Some(4096), &fixed);
        assert_eq!(b[0].tight, Some(false));
        assert_eq!(budgets(65536, None, &fixed)[0].workspace, None, "senza output riservato non si sa");
    }

    fn facts<'a>(budget: &'a ClientBudget, sampling: &'a BTreeMap<String, Sampling>) -> RunFacts<'a> {
        RunFacts {
            run_id: "r-20260915-233520",
            base_url: "http://127.0.0.1:8080",
            alias: "qwen3.6-35b-a3b.q4_k_m.vulkan",
            ctx_declared: 32768,
            ctx_served: Some(32768),
            client: Some(budget),
            endpoint: Some("127.0.0.1:8090"),
            sampling,
            chat_template: None,
            claude_config_dir: None,
            fixed_prompts: &[],
        }
    }

    #[test]
    fn env_block_uses_served_context() {
        let budget = ClientBudget { context_window: 28672, reserved_output_tokens: 4096 };
        let s = snippets(&facts(&budget, &BTreeMap::new()));
        assert!(s.toml.starts_with("[backend]\nkind = \"llamacpp\"\nbase_url = \"http://127.0.0.1:8080\"\n"), "{}", s.toml);
        assert!(s.env.starts_with("AETHERA_RUN_ID=r-20260915-233520\nAETHERA_ENDPOINT=http://127.0.0.1:8090\nBENCH_MODEL="));
        assert!(s.env.contains("BENCH_CONTEXT=32768"));
        assert!(!s.env.contains("\nBENCH_CONVERSATION="), "la conversazione non si inventa");
        assert!(s.powershell.contains("$env:BENCH_BASE_URL = \"http://127.0.0.1:8080\""));
        assert!(!s.toml.contains("[sampling"), "senza campionamento non compare una sezione vuota");
    }

    #[test]
    fn recommended_sampling_travels_with_its_source_and_date() {
        let budget = ClientBudget { context_window: 28672, reserved_output_tokens: 4096 };
        let mut by_mode = BTreeMap::new();
        by_mode.insert(
            "thinking".to_string(),
            Sampling {
                temperature: Some(0.6),
                top_p: Some(0.95),
                top_k: Some(20),
                min_p: Some(0.0),
                presence_penalty: None,
                repeat_penalty: None,
                source: Some("huggingface.co/prova · model card".into()),
                verified: Some("2026-09-16".into()),
            },
        );
        // Una modalità senza nessuna leva non produce una sezione: sarebbe un blocco vuoto.
        by_mode.insert("vuota".to_string(), Sampling::default());
        let s = snippets(&facts(&budget, &by_mode));

        assert!(s.toml.contains("[sampling.thinking]"), "{}", s.toml);
        assert!(s.toml.contains("temperature = 0.6\ntop_p = 0.95\ntop_k = 20\nmin_p = 0.0"), "{}", s.toml);
        assert!(s.toml.contains("# fonte: huggingface.co/prova · model card · letto il 2026-09-16"));
        assert!(!s.toml.contains("[sampling.vuota]"));
        assert!(s.env.contains("# campionamento thinking: temperature=0.6 top_p=0.95 top_k=20 min_p=0.0"));
        assert!(s.powershell.contains("# campionamento thinking:"));
    }

    #[test]
    fn a_hand_written_sampling_says_that_it_has_no_source() {
        let budget = ClientBudget { context_window: 1024, reserved_output_tokens: 128 };
        let mut by_mode = BTreeMap::new();
        by_mode.insert("default".to_string(), Sampling { temperature: Some(0.8), ..Sampling::default() });
        let s = snippets(&facts(&budget, &by_mode));
        assert!(s.toml.contains("# fonte e data sconosciute: valore scritto a mano nel profilo"), "{}", s.toml);
    }
}
