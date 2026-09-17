//! Provenienza di una build compilata su questa macchina (M-14): `provenienza.toml` accanto a
//! `llama-server`, scritto da `.lmbrain-lite/fork/build.ps1`. Dice da che tag viene la build, quale
//! serie di patch porta e con che commit.
//!
//! Una build senza il file è una build di ggml-org scaricata così com'è: prima di M-14 non ne
//! esistevano altre, quindi gli avvii vecchi si leggono come «ggml-org».

use serde::{Deserialize, Serialize};
use std::path::Path;

pub const PROVENANCE_FILE: &str = "provenienza.toml";

/// Serie di una build scaricata da ggml-org, senza file di provenienza.
pub const UPSTREAM_SERIES: &str = "ggml-org";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PatchBranch {
    /// Ramo del fork, per esempio `patch/int8-coopmat`.
    pub ramo: String,
    /// Punta del ramo dopo il rebase sul tag.
    pub commit: String,
    /// Commit della patch, dal più vecchio: `<hash> <oggetto>`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commit_della_patch: Vec<String>,
}

/// I campi del file che Aethera usa; gli altri (opzioni di cmake, versioni degli strumenti…)
/// restano nel file e non si perdono, ma non entrano nel manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Provenance {
    pub schema_version: u32,
    /// Id della build, per esempio `b10991+moro1-vulkan`.
    pub id: String,
    /// Tag di ggml-org su cui poggia la serie, per esempio `b10991`.
    pub base: String,
    pub commit_base: String,
    /// Numero della serie: 0 è il tag liscio compilato qui.
    pub serie: u32,
    pub backend: String,
    /// Commit di `moro-ai` da cui è uscita la build.
    pub commit: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub durata_build_s: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compilatore: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub patch: Vec<PatchBranch>,
}

impl Provenance {
    /// L'etichetta che un profilo scrive in `runtime.build`: `b10991+moro1`.
    pub fn label(&self) -> String {
        format!("{}+moro{}", self.base, self.serie)
    }

    /// La serie come condizione dell'avvio: `moro0`, oppure `moro1 patch/int8-coopmat@1a2b3c4d5`.
    /// Porta i commit dei rami, così una serie ricompilata con una patch cambiata non passa per
    /// la stessa.
    pub fn series(&self) -> String {
        let mut out = format!("moro{}", self.serie);
        for p in &self.patch {
            let short: String = p.commit.chars().take(9).collect();
            out.push_str(&format!(" {}@{short}", p.ramo));
        }
        out
    }
}

/// Il file di provenienza di una build: `None` se non c'è (build di ggml-org), un errore se c'è
/// ma non si legge.
pub fn read(build_dir: &Path) -> Option<Result<Provenance, String>> {
    let path = build_dir.join(PROVENANCE_FILE);
    if !path.is_file() {
        return None;
    }
    Some(
        std::fs::read_to_string(&path)
            .map_err(|e| format!("{}: {e}", path.display()))
            .and_then(|text| toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))),
    )
}

/// La serie di patch da registrare fra le condizioni di un avvio.
pub fn series_of(provenance: Option<&Result<Provenance, String>>) -> String {
    match provenance {
        None => UPSTREAM_SERIES.to_string(),
        Some(Ok(p)) => p.series(),
        // Una build che ha il file ma non lo fa leggere non è di ggml-org: non si sa quale serie sia.
        Some(Err(_)) => "moro? (provenienza illeggibile)".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // La forma che scrive build.ps1, campi in più compresi.
    const FILE: &str = r#"
schema_version = 1
id = "b10991+moro1-vulkan"
base = "b10991"
commit_base = "930e2fa5995789efbf249a8bf61325bb626e417b"
serie = 1
backend = "vulkan"
ramo = "moro-ai"
commit = "0123456789abcdef0123456789abcdef01234567"
data = "2026-09-17T02:00:00+02:00"
durata_configurazione_s = 20
durata_build_s = 640
durata_totale_s = 700
incrementale = false
compilatore = "Microsoft (R) C/C++ Optimizing Compiler Version 19.44"
opzioni = ["-DGGML_VULKAN=ON"]
versione = "version: 0.0.0 (build 10991, commit 012345678)"

[[patch]]
ramo = "patch/int8-coopmat"
commit = "abcdef0123456789abcdef0123456789abcdef01"
commit_prima_del_rebase = "9ff173ddb605602156b7e583cfe00de79b37d1e3"
commit_della_patch = ["aaaa vulkan: add int8 coopmat quantized matmul shader", "bbbb vulkan: add IQ4_XS support"]
"#;

    fn tmp(name: &str) -> std::path::PathBuf {
        let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("aethera-prov-{name}-{n}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn reads_what_the_build_script_writes() {
        let p: Provenance = toml::from_str(FILE).unwrap();
        assert_eq!(p.label(), "b10991+moro1");
        assert_eq!(p.series(), "moro1 patch/int8-coopmat@abcdef012");
        assert_eq!(p.durata_build_s, Some(640));
        assert_eq!(p.patch[0].commit_della_patch.len(), 2);
    }

    #[test]
    fn a_plain_local_build_is_moro0_and_a_download_is_ggml_org() {
        let mut p: Provenance = toml::from_str(FILE).unwrap();
        p.serie = 0;
        p.patch.clear();
        assert_eq!(p.series(), "moro0");
        assert_eq!(series_of(None), "ggml-org");
        assert_eq!(series_of(Some(&Ok(p))), "moro0");
    }

    #[test]
    fn missing_file_is_none_and_a_broken_one_is_an_error() {
        let dir = tmp("file");
        assert!(read(&dir).is_none());
        std::fs::write(dir.join(PROVENANCE_FILE), FILE).unwrap();
        assert_eq!(read(&dir).unwrap().unwrap().id, "b10991+moro1-vulkan");
        std::fs::write(dir.join(PROVENANCE_FILE), "serie = \"uno\"").unwrap();
        let broken = read(&dir).unwrap();
        assert!(broken.is_err());
        assert!(series_of(Some(&broken)).starts_with("moro?"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
