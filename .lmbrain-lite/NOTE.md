---
updated: 2026-09-18
by: lead
---
**Sessione chiusa il 18-09 alle 00:10.** Macchina libera: motore spento, VGM 48, nessun banco in corso.

## Su GitHub (`main` = `9994f6d` più commit locali)
PR #1–#6 unite. Repo pubblico, percorsi fuori. Bozza e tag `v0.1.0-rc.1` fino alla v0.1.0.

## Chiuso in questa sessione
- **M-16**: la patch int8 coopmat entra; regola di fedeltà nuova (KL contro i metri ubatch e CPU, non il testo identico); report in `design/int8-coopmat-2026-09`.
- **Profilo standard G3** su `b10991+moro1` e ubatch 2048, verificato: prefill 281,8 · decode 18,7 (copia `.prima-di-m16`).
- **M-10 T-09 e M-15 T-06**: i checkpoint non recuperano il riuso dopo una modifica a metà prompt (71% con prompt identico, 0 dopo una divergenza).

## Da sapere sulla macchina
- **Riavviare prima di una sessione di misure**: dopo 12 ore di banchi il decode del denso cala del 13% e il G3 dimezza il prefill.
- I ~6 GiB delle funzioni AI di Windows sono morbidi: il sistema li restituisce.
- FN a VGM 48 con `none` fa crescere il file di paging: la batteria su FN si fa a **VGM 64 con mmap**, in una sessione dedicata (circa 2 ore e mezza).

## Aperto, per l'operatore
1. Prova della finestra v2 (M-12 T-10, gruppo 6b), poi merge di `m12-finestra-v2` e parte finestra di M-14 T-07.
2. Tag `v0.1.0` (M-13 T-07).
3. Milestone da aprire sul costo fisso di 1,5 s per richiesta (vale per G1 e G3).
4. Rapporti di chiusura di M-10 (T-13) e M-11 (T-06); profilo G1 con la patch solo dopo il merge upstream.
5. Commit locali non pubblicati: stato del kit e soglia della batteria.
