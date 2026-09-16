//! Importatore dei profili di minis-config e uguaglianza della riga di comando con serve.ps1 (M-02 T-04, T-05).

use aethera_lib::{cmdline, import, manifest, profile};
use std::path::PathBuf;

const KNOWN: [&str; 4] = [
    "qwen3.6-35b-a3b.q4_k_m.vulkan",
    "qwen3-coder-30b-a3b.q4_k_m.vulkan",
    "qwen3-coder-next.q4_k_m.vulkan",
    "gpt-oss-20b.mxfp4.vulkan",
];
const G1: &str = "qwen3.6-35b-a3b.q4_k_m.vulkan";

fn fixture(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/minis-config").join(rel);
    // I manifest di serve.ps1 sono scritti da Windows PowerShell con il BOM.
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .trim_start_matches('\u{feff}')
        .to_string()
}

fn import_known(name: &str) -> import::Imported {
    import::import_minis_json(&fixture(&format!("profiles/{name}.json")), &format!("{name}.json"), "b10809").unwrap()
}

#[test]
fn imports_the_four_known_profiles_into_valid_toml() {
    for name in KNOWN {
        let imported = import_known(name);
        assert_eq!(imported.profile.name, name);
        let issues = profile::validate(&imported.profile);
        assert!(issues.is_empty(), "{name}: {issues:?}");

        let text = toml::to_string_pretty(&imported.profile).unwrap();
        let (back, issues) = profile::load(&text, name);
        assert!(issues.is_empty(), "{name} dopo il giro in TOML: {issues:?}\n{text}");
        assert_eq!(back.unwrap(), imported.profile, "{name}: il TOML non restituisce lo stesso profilo");
    }
}

#[test]
fn g1_extra_args_become_fields() {
    let p = import_known(G1).profile;
    assert_eq!(p.speculative.kind, "draft-mtp");
    assert_eq!(p.speculative.draft_n_max, Some(3));
    assert!(p.server.extra_args.is_empty());
    assert_eq!(p.server.load_mode, "auto");
    assert!(p.server.slot_save);
    let client = p.client.unwrap();
    assert_eq!((client.context_window, client.reserved_output_tokens), (28672, 4096));
    assert_eq!(p.sampling_by_mode["declared"].presence_penalty, Some(1.5));
}

#[test]
fn g1_command_line_matches_serve_ps1() {
    let p = import_known(G1).profile;
    let recorded: serde_json::Value =
        serde_json::from_str(&fixture("manifests/serve-qwen3.6-35b-a3b.q4_k_m.vulkan-20260915-182608.json")).unwrap();
    let expected = cmdline::split_line(recorded["command_line"].as_str().unwrap());

    let paths = cmdline::LaunchPaths {
        model: PathBuf::from(r"X:\pesi\Qwen_Qwen3.6-35B-A3B-Q4_K_M.gguf"),
        slot_dir: Some(PathBuf::from(r"X:\minis-config\manifests\slots")),
        draft_model: None,
        chat_template: None,
    };
    let binary = PathBuf::from(r"X:\llama\b10809-vulkan\llama-server.exe");
    let mut ours = cmdline::split_line(&cmdline::render_line(&binary, &cmdline::build_args(&p, &paths)));

    // Unica differenza attesa: Aethera dichiara il modo di caricamento, serve.ps1 lo lasciava al default.
    let at = ours.iter().position(|a| a == "--load-mode").expect("--load-mode esplicito");
    assert_eq!(ours[at + 1], "auto");
    ours.drain(at..at + 2);

    assert_eq!(ours, expected);
}

fn sample_manifest() -> manifest::Manifest {
    let p = import_known(G1).profile;
    manifest::Manifest {
        schema_version: 1,
        run: manifest::RunSection {
            id: "r-20260915-183102".into(),
            started: "2026-09-15T18:31:02+02:00".into(),
            machine: "moro-ai".into(),
            profile: p.name.clone(),
            profile_file: Some(r"D:\Aethera\profiles\qwen3.6-35b-a3b.q4_k_m.vulkan.toml".into()),
            invalidates_cache: false,
            log: r"D:\Aethera\runs\r-20260915-183102\server.log".into(),
        },
        engine: manifest::EngineSection {
            build_declared: "b10809".into(),
            build: Some("b10809".into()),
            commit: Some("5266f24da".into()),
            backend: "vulkan".into(),
            build_id: "b10809-vulkan".into(),
            binary: r"X:\llama\b10809-vulkan\llama-server.exe".into(),
            version_text: Some("version: 0.4.0-dev (build 10809, commit 5266f24da)".into()),
            provenance: None,
            provenance_error: None,
        },
        model: manifest::ModelSection {
            file: p.model.file.clone(),
            path: r"X:\pesi\Qwen_Qwen3.6-35B-A3B-Q4_K_M.gguf".into(),
            size_bytes: 22_290_000_000,
            size_gb: 22.29,
            sha256_declared: p.model.sha256.clone(),
            sha256_verified: None,
        },
        server: manifest::ServerSection {
            host: "127.0.0.1".into(),
            port: 8080,
            alias: p.name.clone(),
            ctx_declared: 32768,
            ctx_served: Some(32768),
            alias_served: Some(p.name.clone()),
            ready_at: None,
            load_ms: Some(9200),
        },
        command: manifest::CommandSection { line: "\"llama-server.exe\" --port 8080".into(), argv: vec!["llama-server.exe".into()] },
        machine: manifest::MachineSection {
            hostname: Some("MORO-AI".into()),
            os: None,
            cpu: None,
            gpus: vec![],
            ram_total_gib: Some(47.6),
            ram_available_gib_before: None,
        },
        memory: Some(aethera_lib::memory::MemorySection {
            before: Some(aethera_lib::memory::MemoryBefore {
                device: Some("Vulkan0: AMD Radeon(TM) 890M Graphics".into()),
                vram_total_mib: Some(73548),
                vram_free_mib: Some(69870),
                ram_available_gib: Some(45.1),
            }),
            after_load: Some(aethera_lib::memory::MemoryAfter {
                at: "2026-09-15T18:31:14+02:00".into(),
                after_ms: 12200,
                vram_dedicated_gib: Some(22.68),
                vram_shared_gib: Some(0.41),
                working_set_gib: Some(1.1),
                ram_available_gib: Some(33.66),
                double_copy: Some(false),
                ram_margin_gib: 16.0,
                margin_ok: Some(true),
            }),
        }),
        conditions: Some(aethera_lib::conditions::Conditions {
            gpus: vec![aethera_lib::conditions::Driver {
                name: "AMD Radeon(TM) 890M Graphics".into(),
                version: Some("32.0.31041.1004".into()),
                date: Some("2026-08-17".into()),
                dedicated_gib: Some(48.0),
            }],
            npus: vec![aethera_lib::conditions::Driver {
                name: "NPU Compute Accelerator Device".into(),
                version: Some("32.0.20102.3930".into()),
                date: Some("2026-05-07".into()),
                dedicated_gib: None,
            }],
            adrenalin: Some("26.8.1".into()),
            power_scheme: Some("381b4222-f694-41f0-9685-ff5bb260df2e".into()),
            power_overlay: Some("ded574b5-45a0-4f42-8737-46345c09c238".into()),
            weights_volume: Some("C:".into()),
            weights_disk: Some("KINGSTON OM8TAP42048K1-A00".into()),
            weights_bus: Some("NVMe".into()),
            weights_free_gb: Some(1769.02),
            build_series: Some("ggml-org".into()),
        }),
        overrides: vec![profile::Override { field: "server.ubatch".into(), base: Some("4096".into()), value: Some("2048".into()) }],
        exit: Some(manifest::ExitSection { at: "2026-09-15T19:44:31+02:00".into(), code: None, by_user: true, left_running: false }),
        effective_profile: p,
    }
}

fn provenance() -> aethera_lib::provenance::Provenance {
    aethera_lib::provenance::Provenance {
        schema_version: 1,
        id: "b10991+moro1-vulkan".into(),
        base: "b10991".into(),
        commit_base: "930e2fa5995789efbf249a8bf61325bb626e417b".into(),
        serie: 1,
        backend: "vulkan".into(),
        commit: "0123456789abcdef0123456789abcdef01234567".into(),
        data: Some("2026-09-17T02:00:00+02:00".into()),
        durata_build_s: Some(640),
        compilatore: None,
        patch: vec![aethera_lib::provenance::PatchBranch {
            ramo: "patch/int8-coopmat".into(),
            commit: "abcdef0123456789abcdef0123456789abcdef01".into(),
            commit_della_patch: vec!["aaaa vulkan: add int8 coopmat quantized matmul shader".into()],
        }],
    }
}

#[test]
fn manifest_roundtrips_through_toml() {
    let mut m = sample_manifest();
    let text = toml::to_string_pretty(&m).unwrap();
    assert_eq!(toml::from_str::<manifest::Manifest>(&text).unwrap(), m, "{text}");
    // M-14: la provenienza di una build del fork, con i suoi rami, fa lo stesso giro.
    m.engine.provenance = Some(provenance());
    let text = toml::to_string_pretty(&m).unwrap();
    assert!(text.contains("[engine.provenance]") && text.contains("[[engine.provenance.patch]]"), "{text}");
    assert_eq!(toml::from_str::<manifest::Manifest>(&text).unwrap(), m, "{text}");
}

#[test]
fn benchmark_treats_the_patch_series_as_a_condition() {
    use aethera_lib::runs;
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("aethera-m14-runs-{n}"));
    let write = |id: &str, started: &str, m: &manifest::Manifest| {
        let d = dir.join(id);
        std::fs::create_dir_all(&d).unwrap();
        let mut m = m.clone();
        m.run.id = id.into();
        m.run.started = started.into();
        manifest::write(&d.join("manifest.toml"), &m).unwrap();
    };

    // Un avvio di prima di M-14 (nessuna serie registrata), uno liscio su b10991, uno patchato.
    let mut old = sample_manifest();
    old.conditions.as_mut().unwrap().build_series = None;
    write("r-20260916-100000", "2026-09-16T10:00:00+02:00", &old);
    let mut plain = sample_manifest();
    plain.engine.build = Some("b10991".into());
    write("r-20260917-100000", "2026-09-17T10:00:00+02:00", &plain);
    let mut patched = sample_manifest();
    patched.engine.build_declared = "b10991+moro1".into();
    patched.engine.build = Some("b10991".into()); // --version dice solo il tag
    patched.engine.provenance = Some(provenance());
    patched.conditions.as_mut().unwrap().build_series = Some(provenance().series());
    write("r-20260917-110000", "2026-09-17T11:00:00+02:00", &patched);

    let rows = runs::list(&dir, None);
    assert_eq!(rows.len(), 3);
    let (p, l, o) = (&rows[0], &rows[1], &rows[2]);
    assert_eq!(p.build, "b10991+moro1");
    assert_eq!(l.build, "b10991");
    // Senza serie registrata un avvio vecchio è di ggml-org: non ne esistevano altre.
    assert_eq!(o.conditions.as_ref().unwrap().build_series.as_deref(), Some("ggml-org"));
    // Il separatore della tabella: fra liscio e patchato è cambiata una condizione, fra vecchio e liscio no.
    assert_eq!(p.conditions_changed, vec!["serie di patch ggml-org → moro1 patch/int8-coopmat@abcdef012".to_string()]);
    assert!(l.conditions_changed.is_empty(), "{:?}", l.conditions_changed);
    assert!(p.conditions_short.as_deref().unwrap().ends_with("· moro1"));

    // Il confronto lo dice, con la riga della serie.
    let c = runs::compare(&dir, &l.id, &p.id, None).unwrap();
    assert!(c.not_comparable.iter().any(|x| x.starts_with("serie di patch ggml-org")), "{:?}", c.not_comparable);
    let row = c.conditions.iter().find(|x| x.label == "Serie di patch").unwrap();
    assert_eq!(row.a.as_deref(), Some("ggml-org"));
    assert_eq!(row.b.as_deref(), Some("moro1 patch/int8-coopmat@abcdef012"));
    let _ = std::fs::remove_dir_all(&dir);
}
