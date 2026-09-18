---
updated: 2026-09-19
by: lead
---
**Fine della giornata del 18-09, ore 00:30 del 19.** Macchina libera, motore spento, VGM 64. L'operatore riavvia.

## Report per te
`.lmbrain-lite/design/giornata-2026-09-18/index.html` — le decisioni sono l'ultima sezione.

## I risultati
1. **Il candidato è il G1 Q8_0** (`Qwen_Qwen3.6-35B-A3B-Q8_0.gguf`): 33-36 compiti/ora su due giri contro 27-28 del Q4. A VGM 64 regge 256k di contesto allo stesso costo per turno.
2. **Contesto lungo: nessuna risposta sbagliata fino a 200k** su G1, Q8 e G3. Il limite è il prefill a freddo (28 min per 200k): il prefisso va pagato una volta sola.
3. **Nonio corretto: +30 %**, già pubblicato.
4. **n-grammi fuori dal piano**: sul lavoro vero non aiutano né G1 né G3.
5. **Checkpoint spenti: −4 % sul lavoro vero, non −82 %**: senza checkpoint il riuso cala da 92,6 a 81,1 %, perché le sessioni divergono ancora. Restano accesi. M-17 chiuso.

## Da decidere
1. Profilo standard del G1 al **Q8, contesto 262.144, VGM 64**.
2. VGM 64 come impostazione fissa.
3. Unire e pubblicare il ramo `aethera/m18-riuso` di Nonio (619 test verdi).

## Prossimo lavoro
1. **M-18 T-14**: trovare la divergenza residua di Nonio (sospetto: argomenti delle chiamate ri-serializzati). Se si chiude, i checkpoint spenti valgono il −82 %.
2. Batteria intera col thinking acceso (una notte), per un eventuale profilo «intelligente».
3. Capire perché col thinking e senza checkpoint il prefisso resta valido anche senza rimando (log a `-lv 4`).
4. Compito a contesto lungo che tocchi molti file.
