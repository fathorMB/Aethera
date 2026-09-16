---
updated: 2026-09-16
by: lead
---
**Sessione chiusa il 16-09 sera.** Tutto su GitHub `main` (ogni push approvato dall'operatore). Motori spenti.

## Fatto oggi
- **M-08 chiuso**, 14 su 14. Rapporto aggiornato col driver GPU nuovo, studio `design/studio-motore-2026-09/` corretto, sintesi per l'app in [`knowledge/considerazioni-aethera-dalle-misure.md`](knowledge/considerazioni-aethera-dalle-misure.md).
- **Driver iGPU 32.0.31041.1004** (Adrenalin 26.8.1): +17% di prefill e +14–21% di decode; la regressione DPM non si vede. Il denso usa 74,8 GB/s, il 35B 55,0.
- **NPU:** in parallelo costa ~40% al 35B; gli embedding di FastFlowLM la bloccano (`ipustack.sys`), la generazione regge. Rapporto in [`reports/npu-blocco-2026-09-16.md`](reports/npu-blocco-2026-09-16.md).
- **Client:** Nonio, OpenCode e Claude Code conservano il prefisso al 100%; il costo vero è la compattazione. Claude Code con Qwen3.6 vuole l'adattatore `--fold-system` del ponte.
- **Tema scuro** a norma di contrasto.

## Da decidere tu
- **Approvare M-09** «Aethera impara dalle misure» (proposto, parte dal mockup). Scelta aperta: riuso del prompt letto dal log del motore oppure ponte dentro Aethera.

## Da fare con te
- **M-06 T-09:** i guasti provocati, 15 minuti (gruppo 2 di [`reports/da-provare-operatore.md`](reports/da-provare-operatore.md)).
- **M-07 T-02** (VM pulita) e **T-07** (tag v0.1.0).
- Facoltativo: VGM a 64 GB.

M-06 e M-07 sono attivi insieme: l'avviso del kit resta finché non se ne chiude uno.
