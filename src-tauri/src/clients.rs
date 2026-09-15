//! Righe per i client, derivate dall'avvio acceso: `profile.toml` di Nonio e blocco d'ambiente per
//! banchi e Diorama. Quello che il profilo non dice non si inventa.

use crate::profile::ClientBudget;
use serde::Serialize;

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
}

pub fn snippets(f: &RunFacts) -> ClientSnippets {
    let toml = format!("[backend]\nbase_url = \"{}/v1\"\n\n[model]\nexpected = \"{}\"", f.base_url, f.alias);
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
    let env = vars.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("\n") + &format!("\n{ctx_note}\n{conversation}");
    let powershell = vars.iter().map(|(k, v)| format!("$env:{k} = \"{v}\"")).collect::<Vec<_>>().join("\n")
        + &format!("\n{ctx_note}\n{conversation}");
    ClientSnippets { toml, env, powershell }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_block_uses_served_context() {
        let budget = ClientBudget { context_window: 28672, reserved_output_tokens: 4096 };
        let s = snippets(&RunFacts {
            run_id: "r-20260915-233520",
            base_url: "http://127.0.0.1:8080",
            alias: "qwen3.6-35b-a3b.q4_k_m.vulkan",
            ctx_declared: 32768,
            ctx_served: Some(32768),
            client: Some(&budget),
            endpoint: Some("127.0.0.1:8090"),
        });
        assert!(s.toml.contains("base_url = \"http://127.0.0.1:8080/v1\""));
        assert!(s.env.starts_with("AETHERA_RUN_ID=r-20260915-233520\nAETHERA_ENDPOINT=http://127.0.0.1:8090\nBENCH_MODEL="));
        assert!(s.env.contains("BENCH_CONTEXT=32768"));
        assert!(!s.env.contains("\nBENCH_CONVERSATION="), "la conversazione non si inventa");
        assert!(s.powershell.contains("$env:BENCH_BASE_URL = \"http://127.0.0.1:8080\""));
    }
}
