---
updated: 2026-09-18
by: lead
---
**18-09, ore 15:35.** Batterie in corso fino a circa le 18.

## Il risultato del giorno: il Q8 del G1
Primo giro: **13/15 e 35,9 compiti/ora**, contro 12/15 e 27,5 del Q4. Ha il decode più lento (24,6 contro 32,4) ma spreca meno turni (101 contro 134). Secondo giro in corso per confermare, più una prova a ctx 131k per vedere se 37,8 GB di pesi ci stanno in VGM 48. **Se regge, è il candidato a profilo standard.**

## Contesto lungo: risposta alla tua domanda
- G1 lucido 18/18 fino a 128k, G3 9/9: a quella profondità **il modello non si perde**, nemmeno a metà conversazione.
- Il limite è il **prefill a freddo**: 148 tok/s a 128k sul G1 (11 min), 119 sul G3 (14 min), 104 a 200k (28 min).
- **Servire 131k invece di 32k non costa niente**: proponibile subito come cambio di profilo.

## Nonio
Tre punti del riuso corretti sul ramo `aethera/m18-riuso` (4 commit, niente push). **Scoperta**: il template Qwen rende il ragionamento dei turni precedenti nella forma che hanno le sessioni di Nonio, quindi rimandarlo chiude la divergenza — e il thinking, spento proprio per questo, potrebbe tornare praticabile. Test e clippy in coda dietro le batterie.

## Da fare stanotte
1. Misura del ragionamento: un compito lungo con thinking acceso, `send_reasoning` on/off, guardando i token riusati (piano preciso in `reports/nonio-harness-2026-09.md`, §7.5).
2. Congelare `lc-py-pipeline` (oggi la batteria avvisa che non è congelato).
3. G1 a 200k completo e G3 oltre 128k.
4. Ridisegnare il compito a contesto lungo: quello di oggi si risolve in 6 turni leggendo 7 file, serve un lavoro che tocchi molti file.

## Aspetta te
Niente di bloccante. Da decidere quando i numeri sono confermati: profilo standard del G1 al Q8, contesto a 131k, e se spegnere i checkpoint.
