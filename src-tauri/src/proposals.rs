//! Che cosa le misure di M-08 suggeriscono di cambiare in un profilo, e gli avvisi che ne seguono.
//!
//! Una proposta non tocca il file: la pagina Avvio la applica come modifica sopra il profilo, da
//! guardare nella riga di comando e salvare a mano. Ogni proposta dice da quale misura viene.

use crate::machine::ResolvedBuild;
use crate::profile::Profile;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Proposal {
    /// Campo del profilo, con il punto: `server.ubatch`.
    pub field: String,
    pub value: Value,
    pub current: Value,
    pub reason: String,
}

/// La build che M-08 T-07 ha messo contro b10809: +6-7% di prefill, decode uguale.
pub const MEASURED_BUILD: &str = "b10991";
/// Sopra questa quota della memoria dedicata, `mmap` raddoppia i pesi in RAM (M-08 T-09).
pub const MMAP_FILL: f64 = 0.9;

fn is_qwen36_35b(p: &Profile) -> bool {
    p.model.file.to_ascii_lowercase().contains("qwen3.6-35b-a3b")
}

fn is_coder_next(p: &Profile) -> bool {
    p.model.file.to_ascii_lowercase().contains("qwen3-coder-next")
}

fn push(out: &mut Vec<Proposal>, field: &str, value: Value, current: Value, reason: &str) {
    if value != current {
        out.push(Proposal { field: field.into(), value, current, reason: reason.into() });
    }
}

/// Le proposte per un profilo sui modelli misurati. `builds` sono quelle installate: una build che
/// non c'è non si propone.
pub fn for_profile(p: &Profile, builds: &[ResolvedBuild]) -> Vec<Proposal> {
    let mut out = Vec::new();
    let has_build = |b: &str| builds.iter().any(|x| crate::machine::build_id_matches(&x.id, b, &p.runtime.backend));
    if is_qwen36_35b(p) && p.runtime.build == "b10809" && has_build(MEASURED_BUILD) {
        push(
            &mut out,
            "runtime.build",
            MEASURED_BUILD.into(),
            p.runtime.build.clone().into(),
            "M-08 T-07: b10991 porta il prefill da 353 a 377 tok/s sul prompt da 7k (+6,7%) e da 303 a 321 su quello da 21k; il decode non cambia",
        );
    }
    if is_coder_next(p) {
        push(
            &mut out,
            "server.n_gpu_layers",
            999.into(),
            p.server.n_gpu_layers.map_or(Value::Null, Value::from),
            "M-08 T-08: con tutti i layer sulla GPU il Coder-Next da 48,5 GB sta in 46,1 GiB dedicati e fa 17,75 tok/s; --n-cpu-moe peggiora sempre",
        );
        if p.server.ubatch < 2048 {
            push(
                &mut out,
                "server.ubatch",
                2048.into(),
                p.server.ubatch.into(),
                "M-08 T-08b: -ub 2048 invece di 512 alza il prefill del 21%",
            );
        }
        if p.server.batch < 2048 {
            push(&mut out, "server.batch", 2048.into(), p.server.batch.into(), "ubatch non può superare batch");
        }
    }
    out
}

/// Avvisi che non bloccano l'avvio. `size_bytes` sono i pesi su disco, `dedicated_gib` la memoria
/// dedicata della GPU (su APU: la VGM).
pub fn warnings(p: &Profile, size_bytes: Option<u64>, dedicated_gib: Option<f64>) -> Vec<String> {
    let mut out = Vec::new();
    if let (Some(size), Some(vgm)) = (size_bytes, dedicated_gib) {
        let gib = size as f64 / (1u64 << 30) as f64;
        if p.server.load_mode.contains("mmap") && vgm > 0.0 && gib >= vgm * MMAP_FILL {
            out.push(format!(
                "load_mode = {} con pesi da {gib:.1} GiB su {vgm:.0} GiB dedicati: M-08 T-09 ha misurato che così il processo tiene in RAM una seconda copia delle pagine (39,2 GiB sul Coder-Next), a Windows restano 0,4 GiB e il caricamento passa da 27 a 70 s. Con auto o none non succede.",
                p.server.load_mode
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::template;
    use std::path::PathBuf;

    fn build(id: &str) -> ResolvedBuild {
        ResolvedBuild { id: id.into(), dir: PathBuf::new(), binary: PathBuf::new(), source: "builds/" }
    }

    #[test]
    fn g1_moves_to_the_measured_build_only_if_it_is_installed() {
        let p = template("g1", "Qwen_Qwen3.6-35B-A3B-Q4_K_M.gguf", "b10809", "vulkan", 8080);
        assert!(for_profile(&p, &[build("b10809-vulkan")]).is_empty());
        let props = for_profile(&p, &[build("b10809-vulkan"), build("b10991-win-vulkan-x64")]);
        assert_eq!(props.len(), 1);
        assert_eq!((props[0].field.as_str(), props[0].value.as_str()), ("runtime.build", Some("b10991")));
        let cpu = for_profile(&p, &[build("b10991-win-cpu-x64")]);
        assert!(cpu.is_empty(), "una build di un altro backend non vale");
    }

    #[test]
    fn coder_next_gets_all_layers_and_a_bigger_ubatch() {
        let mut p = template("g3", "Qwen3-Coder-Next-Q4_K_M.gguf", "b10809", "vulkan", 8080);
        p.server.n_gpu_layers = Some(49);
        p.server.ubatch = 512;
        p.server.batch = 512;
        let fields: Vec<String> = for_profile(&p, &[]).into_iter().map(|x| x.field).collect();
        assert_eq!(fields, vec!["server.n_gpu_layers", "server.ubatch", "server.batch"]);
        p.server.n_gpu_layers = Some(999);
        p.server.ubatch = 2048;
        p.server.batch = 2048;
        assert!(for_profile(&p, &[]).is_empty(), "un profilo già allineato non riceve proposte");
    }

    #[test]
    fn mmap_warning_only_when_the_weights_fill_the_vgm() {
        let mut p = template("g3", "Qwen3-Coder-Next-Q4_K_M.gguf", "b10991", "vulkan", 8080);
        let coder_next = 48_530_000_000u64;
        assert!(warnings(&p, Some(coder_next), Some(48.0)).is_empty());
        p.server.load_mode = "mmap".into();
        assert_eq!(warnings(&p, Some(coder_next), Some(48.0)).len(), 1);
        assert!(warnings(&p, Some(22_290_000_000), Some(48.0)).is_empty(), "il 35B non riempie la VGM");
        assert!(warnings(&p, Some(coder_next), None).is_empty(), "VGM sconosciuta: non si avvisa a caso");
    }
}
