---
updated: 2026-09-18
by: lead
---
**Notte del 18-09 finita alle 07:50.** Macchina libera, motore spento, VGM 48.

## Report per te
`.lmbrain-lite/design/notte-2026-09-18/index.html` — leggi quello, le decisioni sono l'ultima sezione.

## I tre risultati
1. **Il costo fisso per richiesta è copia di stato, non calcolo**: il 94 % sono i checkpoint copiati dalla GPU alla RAM a 130 MB/s. `--ctx-checkpoints 0` ne toglie l'82 % (G3 2,20 → 0,40 s; G1 1,85 → 0,33).
2. **Gli n-grammi sono stati bocciati dalla batteria**: raddoppiavano il decode sul banco, ma sul lavoro vero MTP vince di un quarto (20,9 contro 15,8 compiti/ora). Il profilo del G1 non si tocca a 32k.
3. **MTP triplica il costo fisso a 128k** (4,99 s contro 1,76): a contesto lungo il conto va rifatto.

## Deciso da te stanotte
Tag `v0.1.0` messo; finestra v2 in `main`; M-06, M-07, M-09, M-10, M-11, M-12, M-16 chiusi; FN e FC con un no; Nonio corretto su un ramo (test verdi, niente push).

## Aspetta te
1. **Pubblicare la bozza della release v0.1.0** su GitHub (un clic).
2. **Unire le correzioni di Nonio** e rifare la batteria: è la condizione per spegnere i checkpoint.
3. **Batteria a contesto lungo**: tutto quello che sappiamo sui compiti sta a 32k, i tuoi stanno a 128-256k.
4. Se provare gli n-grammi anche sul G3 (lì il confronto è contro «niente»).
5. Batteria sui quant (Q6_K e Q8_0 scaricati, distanza dal Q4 misurata e reale).
6. Issue upstream sui checkpoint lenti; TDP nel BIOS al prossimo riavvio.
