---
title: Architettura e come si lavora
updated: 2026-09-15
---

# Architettura (dopo M-02)

App alla radice del repo: `src/` (SolidJS), `src-tauri/` (Rust, crate `aethera_lib`). Decisioni: [[decisioni-di-progetto]].

## Backend (`src-tauri/src/`)

| Modulo | Cosa fa |
|---|---|
| `settings.rs` | preferenze app (radice dati, comportamento all'uscita) in `%APPDATA%\Aethera\settings.toml`; `AETHERA_CONFIG_DIR` le sposta per le prove |
| `machine.rs` | radice dati (`machine.toml`, `profiles/`, `builds/`, `runs/`), build dichiarate (`[[build]] id = "b10809-vulkan"`) o in `builds/` |
| `system.rs` | letture di sistema dietro il trait `SystemProbe` (Windows: registro + `GlobalMemoryStatusEx`) |
| `profile.rs` | schema TOML v1, validazione con tutti gli errori, `diff` per gli override, `CACHE_FIELDS` |
| `import.rs` | JSON di minis-config → TOML (extra_args di spec/cache/load diventano campi, BOM tollerato) |
| `cmdline.rs` | argv nell'ordine di `serve.ps1` + `--load-mode` esplicito; confronto base/modificato |
| `launch.rs` | `prepare`: validazione, build e pesi risolti, blocchi; usato da anteprima, avvio ed esempio E2E |
| `engine.rs` + `job.rs` | processo in job object (kill-on-close, `release` per «lascia acceso»), monitor `/health` → `/props`, stati |
| `manifest.rs` | `runs/<id>/manifest.toml`, aggiornato a pronto e all'uscita, con profilo effettivo e `[memory.before]` / `[memory.after_load]` |
| `memory.rs` | `--list-devices` (VRAM libera prima), memoria dopo il caricamento, doppia copia (working set ≥ metà dei pesi) |
| `telemetry.rs` | `runs/<id>/telemetry.jsonl`: tempi da `print_timing` del log, cache da `/slots` durante l'elaborazione, riserva `/metrics`; mediane, quota cache, degradato |
| `endpoint.rs` | 127.0.0.1:8090: `/status`, `/run`, `POST`/`DELETE /lock` con TTL, `/telemetry/recent`; rifiuta richieste con `Origin` |
| `runs.rs` | storico e confronto degli avvii per la pagina Benchmark |
| `clients.rs`, `tray.rs` | riga Nonio e blocco `AETHERA_*`/`BENCH_*`; testi e voci della tray |
| `commands.rs`, `lib.rs` | comandi Tauri, tray aggiornata ogni 2 s, scansione orfani ogni 3 s, X → tray con motore acceso, eventi `aethera://ask-exit` e `aethera://notice` |

## Fonti delle misure su b10809 (fissate il 15/16-09)

- Per richiesta: righe `print_timing` (prompt eval, eval, draft acceptance) chiuse da `release`. Il log a verbosità di default non ha i token dalla cache.
- Token dalla cache: `/slots` mentre `is_processing` (`id_task`, `n_prompt_tokens_cache`); a slot libero il campo torna a 0. `llamacpp:prompt_tokens_cached_total` sale **all'avvio** della richiesta: la sua differenza vale solo se nel log non c'è un `launch_slot_` dopo il `release`.
- Memoria del processo: PDH `\GPU Process Memory(pid_<pid>_*)\Dedicated Usage` e `Shared Usage` (22,68 GiB su G1, come `serve.ps1`), working set da `GetProcessMemoryInfo`.
- «In uso»: slot attivo, richiesta negli ultimi 30 s, lock dell'endpoint o protezione manuale; `Engine::stop` rifiuta.

## Come si lancia e si verifica

- Sviluppo: `npm.cmd run tauri dev` (sulla PowerShell dell'operatore `npm.ps1` è bloccato).
- Test: `cargo test --manifest-path src-tauri/Cargo.toml` (fixture in `src-tauri/tests/fixtures/minis-config/`), `npm.cmd test`, `npm.cmd run build`.
- Prova reale M-03: `cargo run --manifest-path src-tauri/Cargo.toml --example e2e_m03 -- C:\AetheraData` (memoria, telemetria, cache, lock che rifiuta lo stop) ed `--example e2e_orphan -- C:\AetheraData` (orfano riconosciuto e terminato). Richiedono la porta 8080 e 8090 libere: non con `tauri dev` acceso (8090).
- Prova reale M-02: `cargo run --manifest-path src-tauri/Cargo.toml --example e2e_g1 -- <radice> C:\Git\minis-config\models C:\Nonio\llama-b10809-vulkan`; resta acceso finché non si crea `<radice>/stop-e2e`. Preflight: `node tasks/run.mjs --preflight nonio` in minis-config con `BENCH_MODEL=qwen3.6-35b-a3b BENCH_CONTEXT=32768 BENCH_CONVERSATION=24000` e i percorsi di Nonio in `C:\Git\Nonio`.

## Misure di riferimento (15-09, E2E M-02)

G1 pronto in 8,5 s, n_ctx servito 32.768, RAM disponibile prima 33,6 GiB su 47,65, 890M con 48 GiB dedicati dal registro.

## Note pratiche

- Nei messaggi di commit evitare testo come `/health`: un controllo dell'ambiente lo scambia per un percorso. Messaggio in un file e `git commit -F`.
