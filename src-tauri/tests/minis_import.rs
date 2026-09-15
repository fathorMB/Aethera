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
        model: PathBuf::from(r"C:\Git\minis-config\models\Qwen_Qwen3.6-35B-A3B-Q4_K_M.gguf"),
        slot_dir: Some(PathBuf::from(r"C:\Git\minis-config\manifests\slots")),
        draft_model: None,
    };
    let binary = PathBuf::from(r"C:\Nonio\llama-b10809-vulkan\llama-server.exe");
    let mut ours = cmdline::split_line(&cmdline::render_line(&binary, &cmdline::build_args(&p, &paths)));

    // Unica differenza attesa: Aethera dichiara il modo di caricamento, serve.ps1 lo lasciava al default.
    let at = ours.iter().position(|a| a == "--load-mode").expect("--load-mode esplicito");
    assert_eq!(ours[at + 1], "auto");
    ours.drain(at..at + 2);

    assert_eq!(ours, expected);
}

#[test]
fn manifest_roundtrips_through_toml() {
    let p = import_known(G1).profile;
    let m = manifest::Manifest {
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
            binary: r"C:\Nonio\llama-b10809-vulkan\llama-server.exe".into(),
            version_text: Some("version: 0.4.0-dev (build 10809, commit 5266f24da)".into()),
        },
        model: manifest::ModelSection {
            file: p.model.file.clone(),
            path: r"C:\Git\minis-config\models\Qwen_Qwen3.6-35B-A3B-Q4_K_M.gguf".into(),
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
        overrides: vec![profile::Override { field: "server.ubatch".into(), base: Some("4096".into()), value: Some("2048".into()) }],
        exit: Some(manifest::ExitSection { at: "2026-09-15T19:44:31+02:00".into(), code: None, by_user: true, left_running: false }),
        effective_profile: p,
    };
    let text = toml::to_string_pretty(&m).unwrap();
    assert_eq!(toml::from_str::<manifest::Manifest>(&text).unwrap(), m, "{text}");
}
