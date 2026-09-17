---
updated: 2026-09-17
by: lead
---
**Notte 16→17-09 chiusa (13:30).** Report per l'operatore: https://claude.ai/artifact/67FX2ZCpZfx6Vqkvxh6uwu

**GitHub** `main` = `6050cc4`: PR #1–#5 unite; bozza e tag v0.1.0-rc.1 restano fino alla v0.1.0.

## Risultati
- **Batteria (M-15)**, compiti riusciti:
  - G1 (35B): 12/15, 22,4 riusciti per ora;
  - G3 (Coder-Next): 12/15, 12,1 per ora;
  - FC (Flash-Coder): 0/15;
  - FN (Flash-Next): non eseguito, RAM 34,9 GiB invece di 41,3 dopo il riavvio.
- **Patch int8 coopmat (M-14)**: prefill +3% su G1 e +27/33% su G3; testo a temperatura 0 diverso, perplessità invariata.

## Decisioni dell'operatore
1. RAM sparita (~6 GiB) oppure VGM 64 per FN e M-15 T-06.
2. Firma del codice prima della v0.1.0 (Smart App Control).
3. Patch int8 su G3: regola del testo identico o della perplessità.
4. Prova della finestra v2 (M-12, gruppo 6b) e merge di `m12-finestra-v2`.
5. Correggere Nonio (`finish` respinto, virgolette di `cmd /C`), poi batteria con 3 giri.
6. Sospendere Windows Update nelle notti di banco.

## Da correggere in Aethera
- Telemetria: compattazioni false all'inizio di ogni compito nuovo.
