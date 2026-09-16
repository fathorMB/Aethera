//! Stima di memoria prima dell'avvio, dai metadati del GGUF e dalle leve del profilo.
//!
//! Regola del progetto: una stima si dichiara stima, e quello che non si sa resta sconosciuto.
//! Qui si **calcolano** i pesi (dal file, esatti), la cache KV al contesto scelto e lo stato
//! ricorrente degli ibridi. Il **buffer di calcolo** non è derivabile in modo onesto da una
//! formula: llama.cpp lo alloca secondo il grafo, il backend e `ubatch`. Invece di inventarlo,
//! lo si prende dagli avvii già misurati (`runs/`) con lo stesso ubatch e backend; se non c'è,
//! resta sconosciuto e il totale vale come **minimo**, non come previsione.
//!
//! Dopo il caricamento la stima non serve più: la sostituisce la misura (`memory::MemoryAfter`).

use crate::gguf::ModelInfo;
use crate::profile::Server;
use serde::Serialize;

/// Byte per valore di una cache: i quantizzati hanno il loro blocco (32 valori) più le scale.
pub fn cache_type_bytes(name: &str) -> Option<f64> {
    Some(match name {
        "f32" => 4.0,
        "f16" | "bf16" => 2.0,
        "q8_0" => 34.0 / 32.0,
        "q5_1" => 24.0 / 32.0,
        "q5_0" => 22.0 / 32.0,
        "q4_1" => 20.0 / 32.0,
        "q4_0" | "iq4_nl" => 18.0 / 32.0,
        _ => return None,
    })
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Estimate {
    /// Pesi: la dimensione del file, non una stima.
    pub weights_bytes: Option<u64>,
    /// Cache KV al contesto dichiarato, con i tipi di cache del profilo.
    pub kv_bytes: Option<u64>,
    /// Stato dei blocchi ricorrenti: non cresce col contesto.
    pub state_bytes: Option<u64>,
    /// Buffer di calcolo: misurato su un avvio precedente confrontabile, oppure sconosciuto.
    pub compute_bytes: Option<u64>,
    /// Da quale avvio viene il buffer di calcolo.
    pub compute_from: Option<String>,
    /// Somma delle voci note.
    pub total_bytes: Option<u64>,
    /// Senza buffer di calcolo il totale è un minimo, non una previsione.
    pub total_is_lower_bound: bool,
    /// Blocchi ad attenzione piena: sugli ibridi solo questi tengono la KV.
    pub full_attention_blocks: Option<u64>,
    /// Che cosa non si è potuto calcolare e perché.
    pub notes: Vec<String>,
}

/// Buffer di calcolo misurato su un avvio passato, già confrontabile per ubatch e backend.
#[derive(Debug, Clone, PartialEq)]
pub struct MeasuredCompute {
    pub run_id: String,
    pub bytes: u64,
}

/// `info` assente significa GGUF non letto: si stima solo quello che si sa.
pub fn estimate(
    info: Option<&ModelInfo>,
    server: &Server,
    weights_bytes: Option<u64>,
    compute: Option<MeasuredCompute>,
) -> Estimate {
    let mut e = Estimate { weights_bytes, ..Default::default() };
    if weights_bytes.is_none() {
        e.notes.push("pesi non trovati sul disco: dimensione sconosciuta".into());
    }

    let Some(i) = info else {
        e.notes.push("metadati GGUF non letti: cache KV e stato non calcolabili".into());
        e.total_bytes = weights_bytes;
        e.total_is_lower_bound = true;
        return e;
    };

    // Cache KV: per ogni blocco ad attenzione piena, per ogni token, le teste KV con le loro chiavi e valori.
    let (kb, vb) = (cache_type_bytes(&server.cache_type_k), cache_type_bytes(&server.cache_type_v));
    if kb.is_none() {
        e.notes.push(format!("cache_type_k «{}» sconosciuto: KV non calcolabile", server.cache_type_k));
    }
    if vb.is_none() {
        e.notes.push(format!("cache_type_v «{}» sconosciuto: KV non calcolabile", server.cache_type_v));
    }
    // Sugli ibridi l'attenzione piena è un blocco ogni N: gli altri tengono uno stato, non la KV.
    let blocks = i.block_count;
    e.full_attention_blocks = match (blocks, i.full_attention_interval) {
        (Some(b), Some(n)) if n > 0 => Some(b / n),
        (b, _) => b,
    };
    match (e.full_attention_blocks, i.head_count_kv, i.key_length, i.value_length, kb, vb) {
        (Some(blk), Some(heads), Some(kl), Some(vl), Some(kb), Some(vb)) => {
            let per_token = heads as f64 * (kl as f64 * kb + vl as f64 * vb);
            e.kv_bytes = Some((per_token * server.ctx as f64 * blk as f64) as u64);
        }
        _ => e.notes.push("il GGUF non dichiara teste KV o lunghezze di chiave e valore: cache KV sconosciuta".into()),
    }

    // Stato ricorrente degli ibridi: conv + stato SSM per blocco ricorrente, in f32, indipendente dal contesto.
    if let (Some(blocks), Some(interval), Some(inner), Some(state), Some(conv)) =
        (blocks, i.full_attention_interval, i.ssm_inner_size, i.ssm_state_size, i.ssm_conv_kernel)
    {
        let recurrent = blocks.saturating_sub(blocks / interval.max(1));
        let per_block = inner as f64 * (state as f64 + conv.saturating_sub(1) as f64) * 4.0;
        e.state_bytes = Some((per_block * recurrent as f64 * server.n_parallel.max(1) as f64) as u64);
    }

    match compute {
        Some(m) => {
            e.compute_bytes = Some(m.bytes);
            e.compute_from = Some(m.run_id);
        }
        None => {
            e.total_is_lower_bound = true;
            e.notes.push(format!(
                "buffer di calcolo sconosciuto: nessun avvio misurato con ubatch {} su questo backend, il totale è un minimo",
                server.ubatch
            ));
        }
    }

    let parts = [e.weights_bytes, e.kv_bytes, e.state_bytes, e.compute_bytes];
    if parts.iter().any(Option::is_some) {
        e.total_bytes = Some(parts.iter().flatten().sum());
    }
    if e.kv_bytes.is_none() {
        e.total_is_lower_bound = true;
    }
    e
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Valori letti davvero dal GGUF del G1 (`gguf_dump`, 16-09), non inventati.
    fn qwen36() -> ModelInfo {
        ModelInfo {
            arch: Some("qwen35moe".into()),
            block_count: Some(41),
            context_train: Some(262_144),
            embedding_length: Some(2048),
            head_count: Some(16),
            head_count_kv: Some(2),
            key_length: Some(256),
            value_length: Some(256),
            expert_count: Some(256),
            expert_used_count: Some(8),
            full_attention_interval: Some(4),
            mtp_layers: Some(1),
            ssm_conv_kernel: Some(4),
            ssm_state_size: Some(128),
            ssm_inner_size: Some(4096),
            ssm_group_count: Some(16),
            vocab_size: Some(248_320),
            ..Default::default()
        }
    }

    fn server(ctx: u32, k: &str, v: &str) -> Server {
        Server {
            host: "127.0.0.1".into(),
            port: 8081,
            ctx,
            n_parallel: 1,
            n_gpu_layers: Some(999),
            fit: None,
            fit_target: None,
            flash_attn: "on".into(),
            cache_type_k: k.into(),
            cache_type_v: v.into(),
            ubatch: 2048,
            batch: 4096,
            load_mode: "auto".into(),
            threads: None,
            threads_batch: None,
            n_cpu_moe: None,
            tensor_overrides: Vec::new(),
            lazy_mode: None,
            chat_template_file: None,
            metrics: true,
            jinja: true,
            slot_save: true,
            extra_args: Vec::new(),
        }
    }

    const G1_WEIGHTS: u64 = 22_285_080_192;

    #[test]
    fn kv_counts_only_full_attention_blocks() {
        let e = estimate(Some(&qwen36()), &server(32_768, "f16", "f16"), Some(G1_WEIGHTS), None);
        // 41 blocchi con attenzione piena ogni 4 → 10 blocchi tengono la KV, non 41.
        assert_eq!(e.full_attention_blocks, Some(10));
        // 10 blocchi × 32.768 token × 2 teste KV × (256+256) × 2 byte = 0,67 GB
        assert_eq!(e.kv_bytes, Some(671_088_640));
        // Lo stato ricorrente dei 31 blocchi non ad attenzione piena non cresce col contesto.
        assert_eq!(e.state_bytes, Some(4096 * (128 + 3) * 4 * 31));
        // Senza un avvio misurato il buffer resta sconosciuto e il totale è un minimo.
        assert_eq!(e.compute_bytes, None);
        assert!(e.total_is_lower_bound);
        assert!(e.notes.iter().any(|n| n.contains("buffer di calcolo")), "{:?}", e.notes);
    }

    #[test]
    fn quantised_cache_costs_less_and_context_scales_it() {
        let f16 = estimate(Some(&qwen36()), &server(32_768, "f16", "f16"), None, None).kv_bytes.unwrap();
        let q8 = estimate(Some(&qwen36()), &server(32_768, "q8_0", "q8_0"), None, None).kv_bytes.unwrap();
        let f16_64k = estimate(Some(&qwen36()), &server(65_536, "f16", "f16"), None, None).kv_bytes.unwrap();
        assert!(q8 < f16 && q8 > f16 / 2, "q8_0 costa poco più della metà di f16: {q8} vs {f16}");
        assert_eq!(f16_64k, f16 * 2);
    }

    #[test]
    fn measured_compute_makes_the_total_a_real_estimate() {
        let m = MeasuredCompute { run_id: "r-20260916-001040".into(), bytes: 2_200_000_000 };
        let e = estimate(Some(&qwen36()), &server(32_768, "f16", "f16"), Some(G1_WEIGHTS), Some(m));
        assert!(!e.total_is_lower_bound);
        assert_eq!(e.compute_from.as_deref(), Some("r-20260916-001040"));
        let total = e.total_bytes.unwrap();
        // Pesi + KV + buffer: intorno ai 25 GB, come il totale del mockup.
        assert!((25.0..26.0).contains(&(total as f64 / 1e9)), "totale {total}");
        assert!(e.notes.is_empty(), "{:?}", e.notes);
    }

    #[test]
    fn without_metadata_only_the_weights_are_known() {
        let e = estimate(None, &server(32_768, "f16", "f16"), Some(G1_WEIGHTS), None);
        assert_eq!(e.total_bytes, Some(G1_WEIGHTS));
        assert_eq!(e.kv_bytes, None);
        assert!(e.total_is_lower_bound);
    }

    #[test]
    fn unknown_cache_type_is_not_guessed() {
        let e = estimate(Some(&qwen36()), &server(32_768, "q3_k", "f16"), Some(G1_WEIGHTS), None);
        assert_eq!(e.kv_bytes, None);
        assert!(e.notes.iter().any(|n| n.contains("q3_k")), "{:?}", e.notes);
        assert_eq!(cache_type_bytes("f16"), Some(2.0));
        assert_eq!(cache_type_bytes("inventato"), None);
    }
}
