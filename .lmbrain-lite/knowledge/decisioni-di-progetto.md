---
title: Decisioni di progetto
updated: 2026-09-15
---

# Decisioni di progetto (analisi del 2026-09-15)

Fonte dei requisiti: `<minis-config>\docs\13-launcher.md`. Ogni voce risponde a una domanda posta all'operatore.

| # | Domanda | Decisione |
|---|---|---|
| 1 | Stack | Tauri 2 + SolidJS, allineato alla finestra di Nonio. |
| 2 | Formato profili | TOML con schema proprio di Aethera. I JSON di minis-config si importano, non dettano il formato. Principio: l'app gestisce il motore, non è legata al metodo dei test. |
| 3 | Confine banchi | Aethera non lancia banchi. Profili di avvio multipli + configurazione manuale libera da UI. Pagina Benchmark che riporta, non lancia. |
| 4 | Storico Benchmark | Telemetria propria per ogni avvio: prefill/decode, token proposti e accettati, quota cache del prefisso, memoria misurata, tempo acceso. Nessun import esterno nella v1. |
| 5 | Config vs profili | Profilo come base, campi modificati evidenziati in sovrapposizione, riga di comando live; avvio con differenze nel manifest o «salva come nuovo profilo». |
| 6 | Stato «in uso» | Osservazione di `/slots` e `/metrics` + interruttore manuale «proteggi il motore», più endpoint proprio (stato, id avvio, lock) per i client che si dichiarano. |
| 7 | Dati su disco | Radice scelta dall'utente al primo avvio: `machine.toml`, `profiles/`, `builds/`, `runs/<id>/`, telemetria. Profili senza percorsi assoluti: pesi per hash e nome file. |
| 8 | Ciclo di vita | X = tray con motore acceso; «Esci» chiede se fermare o lasciare orfano (preferenza ricordata). Un motore alla volta nella v1. |
| 9 | Portabilità | Windows prima; letture di sistema dietro un'interfaccia Rust, altrove «sconosciuto». |
| 10 | Download | Build da release ggml-org (SHA-256 dal digest GitHub); pesi da Hugging Face (`resolve/main`, riprendibile, SHA-256 contro oid LFS); import da disco per hash, hard link riconosciuti. |
| 11 | Schermate v1 | Motore, Avvio, Catalogo, Benchmark, Impostazioni; più tray e riga da copiare per i client. |
| 12 | Linguaggio visivo | Strumento tecnico scuro e denso (tema chiaro secondario), monospazio per riga di comando, hash e numeri, badge di stato; ogni misura con unità e condizione, «sconosciuto» esplicito. |

## Questioni ancora aperte (dal documento sorgente)

- `load_mode` e `--no-mmap` su UMA: da misurare sullo stesso build prima di fissare la regola. Default `auto`.
- Speculazione di default per il profilo G1: si decide a misure finite.
- Qwen3-Coder-Next (48,5 GB) con 48 GB di VGM.

## Soglie confermate con i mockup (M-01, 15-09)

- «Degradato»: motore acceso da più di 24 h, oppure decode sotto il 70 % della mediana dell'avvio.
- «In uso» osservato: slot attivo o richieste negli ultimi 30 s; in più lock dichiarato o protezione manuale.
- Endpoint proprio di Aethera su `127.0.0.1:8090`: `GET /status`, `GET /run`, `POST /lock`, `DELETE /lock`, `GET /telemetry/recent`.
- «Orfano»: un `llama-server` sulla porta non avviato da Aethera si adotta in sola lettura o si termina su richiesta.
- Mockup: `.lmbrain-lite/design/aethera-v1/index.html`.
