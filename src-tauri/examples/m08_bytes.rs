//! M-08 T-02: quanti byte di pesi il motore deve **leggere per ogni token generato**.
//!
//! Serve a trasformare i tok/s in GB/s: la banda utile è `tok/s × byte per token`. Il numero non si
//! stima dalla dimensione del file — su un MoE il file è tutto, ma per token si leggono solo gli
//! esperti scelti — e neanche dai parametri: si somma tensore per tensore dall'intestazione GGUF.
//!
//! Uso: `cargo run --release --example m08_bytes -- <file.gguf> [altri.gguf …]`
//!
//! Regole dichiarate, perché il risultato dipende da queste e non da altro:
//! - `token_embd` **non** entra: per token se ne legge una riga, non la tabella; e non entra
//!   nemmeno `per_layer_token_embd`, la tabella per-layer di Gemma «E», che e' anch'essa
//!   indicizzata per token;
//! - `output.weight` (la testa) entra tutta: si legge a ogni token;
//! - i tensori degli esperti (`ffn_*_exps`) entrano in proporzione `expert_used / expert_count`;
//! - i tensori MTP (blocco oltre `block_count`) sono contati a parte: li legge solo chi specula.

use aethera_lib::gguf::{self, TensorInfo};
use serde_json::json;

/// (elementi per blocco, byte per blocco) dei tipi ggml usati da questi pesi.
fn block(kind: &str) -> Option<(u64, u64)> {
    Some(match kind {
        "F32" => (1, 4),
        "F16" | "BF16" => (1, 2),
        "Q4_0" | "IQ4_NL" => (32, 18),
        "Q4_1" => (32, 20),
        "Q5_0" => (32, 22),
        "Q5_1" => (32, 24),
        "Q8_0" => (32, 34),
        "MXFP4" => (32, 17),
        "Q2_K" => (256, 84),
        "Q3_K" => (256, 110),
        "Q4_K" => (256, 144),
        "Q5_K" => (256, 176),
        "Q6_K" => (256, 210),
        "Q8_K" => (256, 292),
        "IQ4_XS" => (256, 136),
        _ => return None,
    })
}

fn bytes_of(t: &TensorInfo) -> Option<u64> {
    let (per_block, size) = block(&t.kind)?;
    Some(t.elements / per_block * size)
}

/// Numero di blocco dal nome `blk.<n>.…`, quando c'è.
fn blk(name: &str) -> Option<u64> {
    name.strip_prefix("blk.")?.split('.').next()?.parse().ok()
}

fn main() -> Result<(), String> {
    for path in std::env::args().skip(1) {
        let g = gguf::read(std::path::Path::new(&path))?;
        let info = gguf::info(&g);
        let file_bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let blocks = info.block_count.unwrap_or(0);
        let used = info.expert_used_count.unwrap_or(0);
        let experts = info.expert_count.unwrap_or(0);

        let (mut total, mut unknown) = (0u64, Vec::new());
        let (mut embd, mut head, mut dense, mut exps, mut mtp) = (0u64, 0u64, 0u64, 0u64, 0u64);
        for t in &g.tensors {
            let Some(b) = bytes_of(t) else {
                unknown.push(format!("{} ({})", t.name, t.kind));
                continue;
            };
            total += b;
            let is_mtp = blk(&t.name).is_some_and(|n| blocks > 0 && n >= blocks);
            if is_mtp {
                mtp += b;
            // `per_layer_token_embd` e' la tabella per-layer di Gemma «E» (3n, 4E): ha una riga
            // per token del vocabolario, esattamente come `token_embd`, e per token se ne legge
            // una riga sola. Contarla come densa gonfia il conto di piu' del doppio — sul
            // gemma-4-E4B q4_0 dava 4,589 GB per token invece di 2,277 (trovato il 20-09).
            } else if t.name.starts_with("token_embd") || t.name.contains("per_layer_token_embd") {
                embd += b;
            } else if t.name.starts_with("output.") {
                head += b;
            } else if t.name.contains("_exps") {
                exps += b;
            } else {
                dense += b;
            }
        }
        // Gli esperti letti per token: la frazione usata dal router, dichiarata dal GGUF.
        let exps_per_token = if experts > 0 { exps * used / experts } else { exps };
        let per_token = dense + head + exps_per_token;

        println!("\n=== {path} ===");
        println!(
            "{} · {} blocchi · esperti {}/{} · tipo dominante {:?}",
            info.name.clone().unwrap_or_default(),
            blocks,
            used,
            experts,
            info.dominant_type
        );
        println!(
            "somma dei tensori {:.3} GB · file {:.3} GB · differenza {:.1} MB (intestazione e allineamento)",
            total as f64 / 1e9,
            file_bytes as f64 / 1e9,
            (file_bytes as f64 - total as f64) / 1e6
        );
        if !unknown.is_empty() {
            println!("ATTENZIONE, tipi non in tabella (esclusi dal conto): {unknown:?}");
        }
        println!(
            "  tabella di embedding {:.3} GB (esclusa: per token se ne legge una riga)\n  testa di uscita {:.3} GB\n  denso (attenzione, norme, ffn condiviso) {:.3} GB\n  esperti {:.3} GB, di cui per token {:.3} GB\n  tensori MTP {:.3} GB (contati a parte)",
            embd as f64 / 1e9,
            head as f64 / 1e9,
            dense as f64 / 1e9,
            exps as f64 / 1e9,
            exps_per_token as f64 / 1e9,
            mtp as f64 / 1e9
        );
        println!("BYTE PER TOKEN: {:.4} GB → banda = tok/s × {:.4}", per_token as f64 / 1e9, per_token as f64 / 1e9);
        println!(
            "{}",
            json!({
                "file": path,
                "file_bytes": file_bytes,
                "tensor_bytes": total,
                "embedding_bytes": embd,
                "head_bytes": head,
                "dense_bytes": dense,
                "expert_bytes": exps,
                "expert_bytes_per_token": exps_per_token,
                "mtp_bytes": mtp,
                "bytes_per_token": per_token,
                "expert_used": used,
                "expert_count": experts,
                "block_count": blocks,
            })
        );
    }
    Ok(())
}
