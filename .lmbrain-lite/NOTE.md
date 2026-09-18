---
updated: 2026-09-18
by: lead
---
**Sessione del 18-09, ore 10:15.** Macchina libera, motore spento, VGM 48.

## Fatto stamattina
- **Release v0.1.0 pubblicata** su GitHub (non più bozza, con installatore e SHA-256). M-13 chiuso.
- **Nonio corretto, unito e pubblicato** (`main` = `608a67a`): **+28/+32 % di compiti per ora**, confermato su due giri (27,5 e 26,8 contro 20,9). Turni da 249 a 134-155, comandi shell falliti da 102 a 13-24. È la leva più grande trovata finora, e non era nel motore.

## Report per te
`.lmbrain-lite/design/notte-2026-09-18/index.html` — aggiornato con i numeri di Nonio. Le decisioni aperte sono l'ultima sezione.

## I risultati della notte, in breve
1. Il costo fisso per richiesta è **copia dei checkpoint** (94 %), a 130 MB/s. `--ctx-checkpoints 0` ne toglie l'82 %.
2. Gli **n-grammi sono bocciati** dalla batteria: sul lavoro vero MTP vince di un quarto. Profilo del G1 invariato a 32k.
3. **MTP triplica il costo fisso a 128k**: a contesto lungo il conto va rifatto.

## Aspetta te
1. **Batteria a contesto lungo** — la cosa che manca per rispondere alla domanda vera: tutto quello che sappiamo sui compiti sta a 32k, i tuoi stanno a 128-256k. È quello che farei adesso.
2. **Spegnere i checkpoint nei profili standard**: ora la condizione c'è (Nonio corretto), ma restano le tre rotture del riuso, prima fra tutte il ragionamento non rimandato.
3. Se provare gli n-grammi anche sul G3, dove il confronto è contro «niente».
4. Batteria sui quant (Q6_K e Q8_0 pronti, distanza dal Q4 misurata).
5. Issue upstream sui checkpoint lenti; TDP nel BIOS al prossimo riavvio.
6. Da tenere d'occhio: `rs-lru` fallisce nei due giri col Nonio nuovo e prima riusciva (era già marginale, arriva sempre al tetto dei turni).
