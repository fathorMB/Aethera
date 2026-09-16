//! Preparazione di un avvio: validazione, differenze dal profilo base, build e pesi risolti sulla
//! macchina, riga di comando. La usano l'anteprima della pagina Avvio e l'avvio vero.

use crate::cmdline::{self, LaunchPaths};
use crate::machine::{self, DataRoot, MachineConfig, ResolvedBuild};
use crate::profile::{self, Issue, Override, Profile};
use std::fs;
use std::path::{Path, PathBuf};

pub struct Prepared {
    pub issues: Vec<Issue>,
    pub overrides: Vec<Override>,
    pub invalidates_cache: bool,
    /// Motivi per cui l'avvio non può partire: vuoto significa avviabile.
    pub blockers: Vec<String>,
    pub build: Option<ResolvedBuild>,
    pub model_path: Option<PathBuf>,
    pub model_size: Option<u64>,
    pub args: Vec<String>,
    pub base_args: Option<Vec<String>>,
    pub base_file: Option<PathBuf>,
}

/// `base` è il nome del profilo su disco da cui si parte (vuoto se non c'è), `edited` il profilo
/// con le modifiche; `slot_dir` la cartella degli slot dell'avvio.
pub fn prepare(root: &DataRoot, m: &MachineConfig, base: &str, edited: &Profile, slot_dir: &Path) -> Prepared {
    let issues = profile::validate(edited);
    let base_file = (!base.is_empty()).then(|| root.profiles().join(format!("{base}.toml"))).filter(|p| p.is_file());
    let base_profile =
        base_file.as_ref().and_then(|p| fs::read_to_string(p).ok()).and_then(|text| profile::load(&text, base).0);
    let overrides = base_profile.as_ref().map(|b| profile::diff(b, edited)).unwrap_or_default();
    let invalidates_cache = overrides.iter().any(|o| profile::CACHE_FIELDS.contains(&o.field.as_str()));

    let mut blockers = Vec::new();
    if !issues.is_empty() {
        blockers.push(format!("il profilo ha {} errori di validazione", issues.len()));
    }
    if edited.runtime.kind != "llama.cpp" {
        blockers.push(format!("runtime «{}» non avviabile: questa versione avvia solo llama.cpp", edited.runtime.kind));
    }
    let builds = root.builds_available(m);
    let build = machine::resolve_build(&builds, &edited.runtime.build, &edited.runtime.backend).cloned();
    if build.is_none() {
        // Una build dichiarata in machine.toml ma sparita dal disco è un guasto diverso da una
        // build mai dichiarata, e si ripara in un altro modo: dirlo risparmia una caccia.
        let declared: Vec<&machine::BuildEntry> = m
            .builds
            .iter()
            .filter(|b| {
                let parts: Vec<&str> = b.id.split('-').collect();
                parts.contains(&edited.runtime.build.as_str()) && parts.contains(&edited.runtime.backend.as_str())
            })
            .collect();
        blockers.push(match declared.first() {
            Some(b) => format!(
                "la build {} · {} è dichiarata in machine.toml come «{}» ma in {} non c'è {}: la cartella è stata spostata o cancellata. Ridichiarala in Impostazioni o rimettila lì.",
                edited.runtime.build,
                edited.runtime.backend,
                b.id,
                b.path.display(),
                machine::server_binary_name()
            ),
            None => format!(
                "nessuna build {} · {} su questa macchina: dichiarala in Impostazioni o mettila in {}",
                edited.runtime.build,
                edited.runtime.backend,
                root.builds().display()
            ),
        });
    }

    let in_models = |file: &str| m.models_dir.as_ref().map(|d| d.join(file));
    let model_path = in_models(&edited.model.file);
    let mut model_size = None;
    match &model_path {
        None => blockers.push("cartella dei pesi non impostata (Impostazioni → machine.toml)".into()),
        Some(p) => match fs::metadata(p) {
            Ok(md) if md.is_file() => {
                model_size = Some(md.len());
                if let Some(declared) = edited.model.size_gb {
                    let gb = md.len() as f64 / 1e9;
                    if declared > 0.0 && (gb - declared).abs() / declared > 0.05 {
                        blockers.push(format!(
                            "i pesi misurano {gb:.2} GB, il profilo ne dichiara {declared}: download parziale o file diverso"
                        ));
                    }
                }
            }
            _ => blockers.push(format!(
                "pesi non trovati: {}. Il file è stato rinominato o spostato: il Catalogo dice quali file ci sono davvero in {} e permette di ricollegare la voce.",
                p.display(),
                m.models_dir.as_ref().map(|d| d.display().to_string()).unwrap_or_default()
            )),
        },
    }
    if let Some(draft) = edited.speculative.draft_model.as_deref().and_then(in_models) {
        if !draft.is_file() {
            blockers.push(format!("modello draft non trovato: {}", draft.display()));
        }
    }
    if let Some(t) = &edited.server.chat_template_file {
        let file = root.templates().join(t);
        if !file.is_file() {
            blockers.push(format!(
                "template di chat non trovato: {}. Aethera riscrive i suoi template (qwen3.6-tollerante.jinja) alla prossima apertura se mancano; gli altri vanno messi lì a mano.",
                file.display()
            ));
        }
    }

    let paths_for = |p: &Profile| LaunchPaths {
        model: in_models(&p.model.file).unwrap_or_else(|| PathBuf::from(&p.model.file)),
        slot_dir: p.server.slot_save.then(|| slot_dir.to_path_buf()),
        draft_model: p.speculative.draft_model.as_deref().map(|f| in_models(f).unwrap_or_else(|| PathBuf::from(f))),
        chat_template: p.server.chat_template_file.as_deref().map(|t| root.templates().join(t)),
    };
    let args = cmdline::build_args(edited, &paths_for(edited));
    let base_args = base_profile.as_ref().map(|b| cmdline::build_args(b, &paths_for(b)));

    Prepared { issues, overrides, invalidates_cache, blockers, build, model_path, model_size, args, base_args, base_file }
}
