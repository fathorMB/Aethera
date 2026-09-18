---
updated: 2026-09-18
by: lead
---
**Notte del 18-09 — linea tracciata, direzione nuova.** VGM 48.

## Deciso dall'operatore stanotte
- **Obiettivo**: massimo rendimento della macchina (server quasi headless) per l'agente di coding, con Nonio o un harness nostro; servono 128–256k di contesto. Metro: compiti riusciti per ora; a contesto lungo, costo per turno a profondità.
- **Linea**: tag `v0.1.0` messo su main (PR #7), prove della finestra non fatte dichiarate nelle note. M-06, M-07, M-09, M-16 chiusi. Finestra v2 in PR #8, si unisce senza prova.
- FN e FC si chiudono con un no. Nessun modello nuovo finché G1 e G3 non sono spremuti.
- Parere del critico: `reports/critica-piano-2026-09-18.md` (decode = 70 % del tempo; MTP perde in profondità; la batteria non distingue i quant).

## In corso, da solo
1. M-11 T-04 (FC contesto lungo e riuso) → rapporto e chiusura.
2. M-17 T-01…T-03: costo fisso su G3, G1, denso 8B, poi log a `-lv 5`.
3. Download G1 Q6_K e Q8_0.
4. `m18/notte-1.sh`: sentinella, speculativa a n-grammi su G3, MTP/n-grammi su G1, costo per turno a 64k e 128k, KLD dei quant.
5. Agente Opus su Nonio (ramo `aethera/m18-harness`, niente push): `finish` respinto, `run_shell`, regole del riuso → `reports/nonio-harness-2026-09.md`.

## Per l'operatore
- Pubblicare la bozza della release v0.1.0 su GitHub (gesto tuo).
- Poi: verifica mirata py-slug/py-ledger con e senza patch int8 (decisa, da mettere in una notte).
- TDP/modalità di potenza nel BIOS: 5 minuti, quando riavvii.
