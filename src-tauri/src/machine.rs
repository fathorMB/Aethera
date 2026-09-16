//! Radice dati scelta dall'utente e `machine.toml`: percorsi e build sono della macchina, non del profilo.

use crate::system::SystemReport;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub const MACHINE_FILE: &str = "machine.toml";
pub const MACHINE_SCHEMA: u32 = 1;

fn default_margin() -> f64 {
    16.0
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MachineConfig {
    pub schema_version: u32,
    pub name: String,
    /// Cartella dei pesi: i profili citano solo il nome del file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub models_dir: Option<PathBuf>,
    /// Soglia di margine: GiB che devono restare disponibili a Windows al picco.
    #[serde(default = "default_margin")]
    pub ram_margin_gib: f64,
    /// Build installate fuori da `builds/` (per esempio quelle già usate da minis-config).
    #[serde(default, rename = "build", skip_serializing_if = "Vec::is_empty")]
    pub builds: Vec<BuildEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BuildEntry {
    /// Identificativo con build e backend separati da trattini, per esempio `b10809-vulkan`.
    pub id: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResolvedBuild {
    pub id: String,
    pub dir: PathBuf,
    pub binary: PathBuf,
    pub source: &'static str,
}

pub fn server_binary_name() -> &'static str {
    if cfg!(windows) {
        "llama-server.exe"
    } else {
        "llama-server"
    }
}

#[derive(Debug, Clone)]
pub struct DataRoot {
    pub path: PathBuf,
}

impl DataRoot {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn profiles(&self) -> PathBuf {
        self.path.join("profiles")
    }

    pub fn builds(&self) -> PathBuf {
        self.path.join("builds")
    }

    pub fn runs(&self) -> PathBuf {
        self.path.join("runs")
    }

    /// Template di chat che un profilo può passare al motore con `chat_template_file`.
    pub fn templates(&self) -> PathBuf {
        self.path.join("templates")
    }

    pub fn machine_file(&self) -> PathBuf {
        self.path.join(MACHINE_FILE)
    }

    /// La radice si può raggiungere e ci si può scrivere. Un disco esterno staccato, una cartella
    /// di rete caduta o una cartella di sola lettura si vedono solo provando: `is_dir()` mente su un
    /// percorso a cui non si ha accesso, e accorgersene al primo salvataggio vuol dire accorgersene
    /// quando c'è già qualcosa da perdere.
    pub fn check_writable(&self) -> Result<(), String> {
        let probe = self.path.join(".aethera-prova-scrittura");
        fs::write(&probe, b"aethera").map_err(|e| crate::diagnose::write_error("scrittura", &self.path, &e))?;
        let _ = fs::remove_file(&probe);
        Ok(())
    }

    /// Crea la struttura se manca e scrive un `machine.toml` iniziale al primo avvio.
    pub fn ensure(&self, system: &SystemReport) -> Result<MachineConfig, String> {
        for dir in [self.path.clone(), self.profiles(), self.builds(), self.runs(), self.templates()] {
            fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        // Solo se manca: un template cambiato a mano resta com'è.
        for (name, text) in BUILTIN_TEMPLATES {
            let file = self.templates().join(name);
            if !file.exists() {
                fs::write(&file, text).map_err(|e| format!("{}: {e}", file.display()))?;
            }
        }
        if !self.machine_file().exists() {
            let name = system.hostname.clone().unwrap_or_else(|| "macchina".into()).to_lowercase();
            self.save_machine(&MachineConfig {
                schema_version: MACHINE_SCHEMA,
                name,
                models_dir: None,
                ram_margin_gib: default_margin(),
                builds: Vec::new(),
            })?;
        }
        self.load_machine()
    }

    pub fn load_machine(&self) -> Result<MachineConfig, String> {
        let file = self.machine_file();
        let text = fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let m: MachineConfig = toml::from_str(&text).map_err(|e| format!("{}: {e}", file.display()))?;
        if m.schema_version != MACHINE_SCHEMA {
            return Err(format!(
                "{}: schema_version {} non supportata (attesa {MACHINE_SCHEMA})",
                file.display(),
                m.schema_version
            ));
        }
        Ok(m)
    }

    pub fn save_machine(&self, m: &MachineConfig) -> Result<(), String> {
        let file = self.machine_file();
        let text = toml::to_string_pretty(m).map_err(|e| e.to_string())?;
        fs::write(&file, text).map_err(|e| format!("{}: {e}", file.display()))
    }

    /// Build con un `llama-server` presente: prima quelle dichiarate in `machine.toml`, poi `builds/`.
    pub fn builds_available(&self, m: &MachineConfig) -> Vec<ResolvedBuild> {
        let mut out: Vec<ResolvedBuild> = m
            .builds
            .iter()
            .filter_map(|b| {
                let binary = b.path.join(server_binary_name());
                binary.is_file().then(|| ResolvedBuild {
                    id: b.id.clone(),
                    dir: b.path.clone(),
                    binary,
                    source: "machine.toml",
                })
            })
            .collect();
        if let Ok(entries) = fs::read_dir(self.builds()) {
            let mut dirs: Vec<PathBuf> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.join(server_binary_name()).is_file())
                .collect();
            dirs.sort();
            for dir in dirs {
                let name = dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                out.push(ResolvedBuild {
                    id: name.strip_prefix("llama-").unwrap_or(&name).to_string(),
                    binary: dir.join(server_binary_name()),
                    dir,
                    source: "builds/",
                });
            }
        }
        out
    }
}

/// Template che Aethera scrive in `templates/` se mancano.
pub const BUILTIN_TEMPLATES: &[(&str, &str)] =
    &[("qwen3.6-tollerante.jinja", include_str!("../templates/qwen3.6-tollerante.jinja"))];

/// Backend di llama.cpp riconosciuti dentro l'id di una build.
pub const BACKENDS: &[&str] = &["vulkan", "cuda", "hip", "sycl", "musa", "cann", "opencl", "metal", "blas", "cpu"];

/// Etichetta di una build letta da un pezzo del suo id: `b10991` è ggml-org, `b10991+moro1` è la
/// serie 1 del fork locale (M-14) sopra quel tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildTag {
    /// Il tag di ggml-org, per esempio `b10991`.
    pub base: String,
    /// La serie di patch del fork locale: `None` per una build di ggml-org.
    pub series: Option<u32>,
}

fn is_tag_number(s: &str) -> bool {
    s.len() > 1 && s.starts_with('b') && s[1..].chars().all(|c| c.is_ascii_digit())
}

/// `b10991` → base senza serie; `b10991+moro1` → base e serie 1; tutto il resto → `None`.
pub fn parse_build_tag(piece: &str) -> Option<BuildTag> {
    let (base, rest) = match piece.split_once('+') {
        Some((b, r)) => (b, Some(r)),
        None => (piece, None),
    };
    if !is_tag_number(base) {
        return None;
    }
    let series = match rest {
        None => None,
        Some(r) => {
            let n = r.strip_prefix("moro")?;
            if n.is_empty() || !n.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
            Some(n.parse().ok()?)
        }
    };
    Some(BuildTag { base: base.to_string(), series })
}

/// Da `b10809-win-vulkan-x64` a («b10809», «vulkan»): la build è il pezzo `b<numero>` (con la serie
/// del fork se c'è: `b10991+moro1`), il backend il primo pezzo riconosciuto. Serve a proporre valori
/// a un profilo nuovo, non a risolvere.
pub fn split_build_id(id: &str) -> (Option<String>, Option<String>) {
    let parts: Vec<&str> = id.split('-').collect();
    let build = parts.iter().find(|p| parse_build_tag(p).is_some()).map(|s| s.to_string());
    let backend = parts.iter().find(|p| BACKENDS.contains(&p.to_ascii_lowercase().as_str())).map(|s| s.to_ascii_lowercase());
    (build, backend)
}

/// Un id di build soddisfa `build` e `backend` di un profilo se fra i pezzi separati da trattini ci
/// sono esattamente l'una e l'altro. Il confronto è esatto di proposito: `b10991+moro1-vulkan` non
/// soddisfa un profilo che chiede `b10991` (la build patchata va chiesta per nome), e
/// `b10991-vulkan` non soddisfa un profilo che chiede `b10991+moro1`.
pub fn build_id_matches(id: &str, build: &str, backend: &str) -> bool {
    let parts: Vec<&str> = id.split('-').collect();
    parts.contains(&build) && parts.contains(&backend)
}

/// La prima build che soddisfa il profilo (`b10809-vulkan`, `b10809-win-vulkan-x64`,
/// `b10991+moro1-vulkan`): vedi [`build_id_matches`].
pub fn resolve_build<'a>(builds: &'a [ResolvedBuild], build: &str, backend: &str) -> Option<&'a ResolvedBuild> {
    builds.iter().find(|b| build_id_matches(&b.id, build, backend))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rb(id: &str) -> ResolvedBuild {
        ResolvedBuild { id: id.into(), dir: PathBuf::new(), binary: PathBuf::new(), source: "test" }
    }

    #[test]
    fn resolves_build_by_parts() {
        let builds = [rb("b10809-win-cpu-x64"), rb("b10809-win-vulkan-x64"), rb("b10985-vulkan")];
        assert_eq!(resolve_build(&builds, "b10809", "vulkan").unwrap().id, "b10809-win-vulkan-x64");
        assert_eq!(resolve_build(&builds, "b10985", "vulkan").unwrap().id, "b10985-vulkan");
        assert!(resolve_build(&builds, "b1080", "vulkan").is_none());
    }

    #[test]
    fn build_id_splits_into_build_and_backend() {
        assert_eq!(split_build_id("b10809-win-vulkan-x64"), (Some("b10809".into()), Some("vulkan".into())));
        assert_eq!(split_build_id("b10985-vulkan"), (Some("b10985".into()), Some("vulkan".into())));
        assert_eq!(split_build_id("llama-b10809-win-cpu-x64"), (Some("b10809".into()), Some("cpu".into())));
        // Una cartella che non dice né build né backend non li inventa.
        assert_eq!(split_build_id("mia-build"), (None, None));
        // La serie del fork resta attaccata alla build: un profilo nuovo la chiede per nome.
        assert_eq!(split_build_id("b10991+moro1-win-vulkan-x64"), (Some("b10991+moro1".into()), Some("vulkan".into())));
        assert_eq!(split_build_id("b10991+patch-vulkan"), (None, Some("vulkan".into())));
    }

    #[test]
    fn build_tags_with_and_without_series() {
        assert_eq!(parse_build_tag("b10991"), Some(BuildTag { base: "b10991".into(), series: None }));
        assert_eq!(parse_build_tag("b10991+moro0"), Some(BuildTag { base: "b10991".into(), series: Some(0) }));
        assert_eq!(parse_build_tag("b10991+moro12"), Some(BuildTag { base: "b10991".into(), series: Some(12) }));
        for bad in ["b", "b10991+", "b10991+moro", "b10991+moroX", "b10991+altro1", "x10991+moro1", "b10991+moro1+moro2"] {
            assert_eq!(parse_build_tag(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_patched_build_is_used_only_when_the_profile_asks_for_it() {
        let builds = [rb("b10991+moro1-vulkan"), rb("b10991+moro0-win-vulkan-x64"), rb("b10991-win-vulkan-x64")];
        // Il profilo che chiede la build liscia prende quella di ggml-org, anche se le patchate vengono prima.
        assert_eq!(resolve_build(&builds, "b10991", "vulkan").unwrap().id, "b10991-win-vulkan-x64");
        assert_eq!(resolve_build(&builds, "b10991+moro1", "vulkan").unwrap().id, "b10991+moro1-vulkan");
        assert_eq!(resolve_build(&builds, "b10991+moro0", "vulkan").unwrap().id, "b10991+moro0-win-vulkan-x64");
        assert!(resolve_build(&builds, "b10991+moro2", "vulkan").is_none());
        assert!(resolve_build(&builds, "b10991+moro1", "cpu").is_none());
        // Senza la build di ggml-org, un profilo su b10991 non ricade in silenzio su una patchata.
        assert!(resolve_build(&builds[..2], "b10991", "vulkan").is_none());
    }

    #[test]
    fn an_unusable_data_root_says_which_path_and_what_to_do() {
        let root = DataRoot::new(std::env::temp_dir().join("aethera-radice-mai-creata-12345"));
        let e = root.check_writable().unwrap_err();
        assert!(e.contains("aethera-radice-mai-creata-12345"), "{e}");
        assert!(e.contains("non esiste") || e.contains("permessi"), "deve dire che farne: {e}");

        // Una radice che c'è ed è scrivibile non lascia in giro il file di prova.
        let ok = DataRoot::new(std::env::temp_dir().join("aethera-radice-buona-12345"));
        fs::create_dir_all(&ok.path).unwrap();
        assert!(ok.check_writable().is_ok());
        assert_eq!(fs::read_dir(&ok.path).unwrap().count(), 0, "la prova di scrittura non lascia tracce");
        let _ = fs::remove_dir_all(&ok.path);
    }

    #[test]
    fn machine_toml_roundtrip() {
        let m = MachineConfig {
            schema_version: 1,
            name: "moro-ai".into(),
            models_dir: Some(PathBuf::from(r"X:\pesi")),
            ram_margin_gib: 16.0,
            builds: vec![BuildEntry { id: "b10809-vulkan".into(), path: PathBuf::from(r"X:\llama\b10809-vulkan") }],
        };
        let text = toml::to_string_pretty(&m).unwrap();
        assert_eq!(toml::from_str::<MachineConfig>(&text).unwrap(), m);
    }
}
