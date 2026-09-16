---
updated: 2026-09-16
by: lead
---
**Sessione del 16-09 pomeriggio.** Tutto inviato a GitHub su `main` (push del 16-09 approvato dall'operatore). `gh` installata e autenticata (account fathorMB).

## Fatto oggi
- **Tema scuro:** `--fg3` #8a919b e `--err` #f26b6b, sopra 4,5:1 anche su `--bg3`.
- **M-08 T-10:** OpenCode conserva il prefisso al 100%, lo rompe solo compattando. Claude Code **si può** collegare (`/v1/messages`), manca solo la CLI.
- **M-08 T-11 chiuso:** con la NPU che genera, il 35B perde ~40% (prefill e decode). Gli embedding di FastFlowLM bloccano la NPU (`ipustack.sys`, evento 141; col driver vecchio schermata blu): rapporto in [`reports/npu-blocco-2026-09-16.md`](reports/npu-blocco-2026-09-16.md).
- **Driver iGPU 32.0.31041.1004** (Adrenalin 26.8.1): +17% prefill, +14-21% decode sul server; il denso a 74,8 GB/s. Le misure della notte sono col driver vecchio.
- Sintesi per l'app: [`knowledge/considerazioni-aethera-dalle-misure.md`](knowledge/considerazioni-aethera-dalle-misure.md).

## Cosa resta
- **M-08 T-08** (VGM 64 GB) e **T-10** (Claude Code: serve la CLI).
- **M-06 T-09**, **M-07 T-02** (VM) e **T-07** (tag).
- Tre milestone attivi insieme (M-06, M-07, M-08): il kit ne vuole uno.

Motori spenti. WinDbg installato; dump in `C:\AetheraData\m08\dumps`.
