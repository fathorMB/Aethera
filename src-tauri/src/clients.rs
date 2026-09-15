//! Righe per i client, derivate dall'avvio acceso: `profile.toml` di Nonio e blocco d'ambiente per
//! banchi e Diorama. Quello che il profilo non dice non si inventa.
//!
//! Il campionamento consigliato viaggia con le righe perché è il client a mandarlo in ogni
//! richiesta: se resta nel profilo del launcher non arriva a nessuno. Viaggia con la fonte e la
//! data di lettura, così chi incolla la riga sa se sta copiando un dato o un ricordo.

use crate::profile::{ClientBudget, Sampling};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ClientSnippets {
    pub toml: String,
    pub env: String,
    pub powershell: String,
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
    let toml = format!("[backend]\nbase_url = \"{}/v1\"\n\n[model]\nexpected = \"{}\"", f.base_url, f.alias)
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
    ClientSnippets { toml, env, powershell }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        }
    }

    #[test]
    fn env_block_uses_served_context() {
        let budget = ClientBudget { context_window: 28672, reserved_output_tokens: 4096 };
        let s = snippets(&facts(&budget, &BTreeMap::new()));
        assert!(s.toml.contains("base_url = \"http://127.0.0.1:8080/v1\""));
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
