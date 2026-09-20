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
| `machine.rs` | radice dati (`machine.toml`, `profiles/`, `builds/`, `runs/`, `templates/`), build dichiarate (`[[build]] id = "b10809-vulkan"`) o in `builds/`; scrive i template di Aethera solo se mancano |
| `system.rs` | letture di sistema dietro il trait `SystemProbe` (Windows: registro + `GlobalMemoryStatusEx`; condizioni dell'avvio: classi di dispositivi, `PowerSchemes`, `IOCTL_STORAGE_QUERY_PROPERTY`) |
| `conditions.rs` | driver GPU/NPU, AMD Software, overlay di alimentazione, VGM, disco dei pesi; quali condizioni rendono due avvii non confrontabili |
| `proposals.rs` | modifiche che M-08 suggerisce per un profilo (build, `-ngl`, ubatch) e avvisi non bloccanti (`mmap` su pesi che riempiono la VGM) |
| `profile.rs` | schema TOML v1, validazione con tutti gli errori, `diff` per gli override, `CACHE_FIELDS` |
| `import.rs` | JSON di minis-config → TOML (extra_args di spec/cache/load diventano campi, BOM tollerato) |
| `cmdline.rs` | argv nell'ordine di `serve.ps1` + `--load-mode` esplicito; confronto base/modificato |
| `launch.rs` | `prepare`: validazione, build e pesi risolti, blocchi; usato da anteprima, avvio ed esempio E2E |
| `engine.rs` + `job.rs` | processo in job object (kill-on-close, `release` per «lascia acceso»), monitor `/health` → `/props`, stati |
| `manifest.rs` | `runs/<id>/manifest.toml`, aggiornato a pronto e all'uscita, con profilo effettivo e `[memory.before]` / `[memory.after_load]` |
| `memory.rs` | `--list-devices` (VRAM libera prima), memoria dopo il caricamento, doppia copia (working set ≥ metà dei pesi) |
| `telemetry.rs` | `runs/<id>/telemetry.jsonl`: tempi da `print_timing`, cache dalla riga `release` del log (riserve `/slots` e `/metrics`); come ogni richiesta tratta la conversazione, compattazioni, mediane, degradato contro la mediana di riferimento |
| `endpoint.rs` | 127.0.0.1:8090: `/status`, `/run`, `POST`/`DELETE /lock` con TTL, `/telemetry/recent`, `/services`; rifiuta richieste con `Origin` |
| `service.rs` | schema dei **profili di servizio** (M-20): un tipo suo, con meno campi del profilo principale; `role = "service"` li distingue |
| `services.rs` | ciclo di vita dei motori di servizio: avvio in job object, `/health`, VRAM per processo, test di sanità del reranker. Nessun manifest, nessuna telemetria, nessuno stato «in uso» |
| `runs.rs` | storico e confronto degli avvii per la pagina Benchmark |
| `gguf.rs` | intestazione di un GGUF senza caricare i pesi (array lunghi saltati): architettura, blocchi, MTP, esperti, parametri della KV, tipi dei tensori |
| `estimate.rs` | stima prima dell'avvio: pesi + KV + stato ricorrente calcolati, buffer di calcolo **misurato** (vedi sotto) |
| `catalog.rs` | `catalog.toml` + cartella dei pesi: stati, metadati in cache, hard link, profili che usano un file, buffer misurati da `runs/` |
| `hash.rs` | SHA-256 a blocchi con avanzamento e annullamento; confronto che ignora maiuscole e il prefisso `sha256:` |
| `download.rs` | oid LFS da `/api/models/<repo>/tree/main`, download riprendibile in `.part`, rinomina solo dopo la verifica |
| `builds.rs` | release `b<numero>` di ggml-org (sono **prerelease**), asset per backend, digest verificato prima di estrarre, estrazione in `builds/llama-<tag>-<backend>` |
| `tasks.rs` | lavori lunghi (hash, download, installazioni) con avanzamento e annullamento; da non confondere con `job.rs`, che è il job object di Windows |
| `clients.rs`, `tray.rs` | righe per Nonio, OpenCode, Claude Code e blocco `AETHERA_*`/`BENCH_*`, budget di contesto per client; testi e voci della tray |
| `commands.rs`, `lib.rs` | comandi Tauri, tray aggiornata ogni 2 s, scansione orfani ogni 3 s, X → tray con motore acceso, eventi `aethera://ask-exit` e `aethera://notice` |

## Fonti delle misure su b10809 (fissate il 15/16-09)

- Per richiesta: righe `print_timing` (prompt eval, eval, draft acceptance) chiuse da `release`. Il log a verbosità di default non ha i token dalla cache.
- Token dalla cache (M-09): dalla riga `release … stop processing: n_tokens = N` meno generati e rielaborati; coincide con `/slots` a meno di 2 token (18 richieste di Claude Code). Riserve: `/slots` mentre `is_processing` (`id_task`, `n_prompt_tokens_cache`; a slot libero torna a 0) e `llamacpp:prompt_tokens_cached_total`, che sale **all'avvio** della richiesta e vale solo se nel log non c'è un `launch_slot_` dopo il `release`.
- Compattazioni: una richiesta «estende» se tiene ≥ 90% del contesto precedente dello slot o tutto tranne l'ultimo ubatch; «contesto ricostruito» se il prompt è < 70% del contesto precedente, e la richiesta che riscriveva subito prima è il riassunto. Il riuso da solo non basta: Claude Code tiene il suo prompt fisso anche quando compatta.
- Il client di una richiesta si sa solo dal lock sull'endpoint: il log non lo dice, e Aethera non sta fra client e motore.
- Memoria del processo: PDH `\GPU Process Memory(pid_<pid>_*)\Dedicated Usage` e `Shared Usage` (22,68 GiB su G1, come `serve.ps1`), working set da `GetProcessMemoryInfo`.
- «In uso»: slot attivo, richiesta negli ultimi 30 s, lock dell'endpoint o protezione manuale; `Engine::stop` rifiuta.
- **I motori di servizio non entrano in «in uso»** e non bloccano arresto né riavvio del principale: sono servi, non padroni. Vedi [[motori-di-servizio]].
- «In uso» ha una finestra di 30 s: il 20-09 `/status` diceva `in_use: false` con una sessione di coding aperta ma ferma da 21 minuti. Chi lavora deve prendere un **lock**, altrimenti fra un turno e l'altro il motore sembra libero.

## Stima di memoria (M-04): che cosa è calcolato e che cosa è misurato

- **Calcolati** dai metadati GGUF: i pesi (dimensione del file), la cache KV e lo stato ricorrente.
  Sul Qwen3.6 la KV va contata solo sui blocchi ad **attenzione piena** (`full_attention_interval` 4 →
  10 blocchi su 41), non su tutti: gli altri sono ricorrenti e tengono uno stato che non cresce col contesto.
- **Misurato, non stimato**: il buffer di calcolo dipende dal grafo, dal backend e da `ubatch`, e non si
  deriva onestamente da una formula. Si ricava dagli avvii in `runs/` con lo stesso ubatch e backend,
  come VRAM misurata meno pesi, KV e stato. Senza un avvio confrontabile resta sconosciuto e il totale
  è dichiarato **minimo**, non previsione.
- Attenzione a leggere il confronto stima/misura: per lo stesso avvio da cui viene il buffer, la somma
  torna **per costruzione**. È un'identità, non una previsione; quello che verifica è che le formule di
  KV e stato siano coerenti con la misura (sul G1: 22,29 + 0,67 + 0,07 + 1,33 = 24,35 GB misurati).

## Fonti verificate il 16-09 (M-04)

- Hugging Face: `GET /api/models/<repo>/tree/main` dà `lfs.oid` (= SHA-256) e `lfs.size`; il link
  `resolve/main/<file>` risponde **206 Partial Content** al `Range`, quindi la ripresa funziona.
  PowerShell 5.1 non sa mandare l'header `Range` con `Invoke-WebRequest`: per provarlo a mano serve `curl.exe`.
- GitHub: le build di llama.cpp sono **prerelease**, quindi `releases/latest` risponde un'altra cosa
  (`v0.4.1`): si legge `/releases` e si filtrano i tag `^b\d+$`. Ogni asset porta `digest: "sha256:…"`;
  `content_type` è inaffidabile (dice `application/json` su un `.tar.gz`), quindi l'asset si sceglie per nome.
- Sul disco dell'operatore `Qwen_Qwen3.6-35B-A3B-Q4_K_M.gguf` ha **2 hard link** (import di un altro
  strumento): la protezione «non cancellare senza dirlo» non è teorica.

## Come si lancia e si verifica

- Sviluppo: `npm.cmd run tauri dev` (sulla PowerShell dell'operatore `npm.ps1` è bloccato).
- Test: `cargo test --manifest-path src-tauri/Cargo.toml` (fixture in `src-tauri/tests/fixtures/minis-config/`), `npm.cmd test`, `npm.cmd run build`.
- Prova reale M-03: `cargo run --manifest-path src-tauri/Cargo.toml --example e2e_m03 -- <radice>` (memoria, telemetria, cache, lock che rifiuta lo stop) ed `--example e2e_orphan -- <radice>` (orfano riconosciuto e terminato). Richiedono la porta 8080 e 8090 libere: non con `tauri dev` acceso (8090).
- Prova reale M-09: `cargo run --manifest-path src-tauri/Cargo.toml --example e2e_m09 -- <radice> <cartella di lavoro> [nonio opencode claude]`: G1 con le proposte e il template tollerante, i tre client con le righe di Aethera e un lock ciascuno. `--example m09_replay -- <radice> <id avvio>` rilegge il log di un avvio con il parser nuovo e lo confronta con `/slots`; `--example conditions` stampa le condizioni della macchina.
- Prova reale M-04: `cargo run --manifest-path src-tauri/Cargo.toml --example e2e_m04 -- <radice>`
  (metadati, stima contro la misura, catalogo, oid LFS, ripresa del download sul file vero, release e digest).
  `--hash` aggiunge il ricalcolo completo dei 22 GB; `--install` installa davvero una build in `%TEMP%`
  (non nella radice dati). `--example gguf_dump -- <file.gguf>` stampa tutte le chiavi di un GGUF.
- Prova reale M-02: `cargo run --manifest-path src-tauri/Cargo.toml --example e2e_g1 -- <radice> <pesi> <nonio>\llama-b10809-vulkan`; resta acceso finché non si crea `<radice>/stop-e2e`. Preflight: `node tasks/run.mjs --preflight nonio` in minis-config con `BENCH_MODEL=qwen3.6-35b-a3b BENCH_CONTEXT=32768 BENCH_CONVERSATION=24000` e i percorsi di Nonio in `<nonio>`.

## Misure di riferimento (15-09, E2E M-02)

G1 pronto in 8,5 s, n_ctx servito 32.768, RAM disponibile prima 33,6 GiB su 47,65, 890M con 48 GiB dedicati dal registro.

## Note pratiche

- Nei messaggi di commit evitare testo come `/health`: un controllo dell'ambiente lo scambia per un percorso. Messaggio in un file e `git commit -F`.
