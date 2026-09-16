---
updated: 2026-09-16
by: lead
---
**Notte del 16-09 — VGM a 64 GB, banco in corso** (`C:\AetheraData\m10\VGM64.out`).

- **M-10**: T-01, T-02, T-04, T-06 chiusi. T-07b fatto: IQ3_XXS a VGM 64 con mmap + lazy auto, prefill 137, decode 9,3 tok/s, Windows senza memoria disponibile per tutta la sessione.
- **Ora gira** (scelta dell'operatore): T-07c Q3_K_XL, poi T-08 (contesto ~30k e ~60k, ctx 65536), poi T-09 (riuso del prefisso). Sorvegliante: file di paging +2 GiB o motore non pronto dopo 300 s. Circa 3-4 ore.
- T-11/T-12 non scattano: decode sotto i 12 tok/s.
- **M-11**: T-01…T-03 chiusi; restano contesto lungo, batteria e rapporto.

## Dopo, con te
- Riportare la VGM a **48** (fine delle prove a 64).
- Batteria Nonio (M-10 T-10, M-11 T-05) e rapporti.
- Commit: modifiche a `src-tauri/examples/m08_bench.rs` e script in `.lmbrain-lite/m10` e `m11`. Il push solo con il tuo via.

**Non aprire client né programmi pesanti: la macchina è senza RAM libera.**
