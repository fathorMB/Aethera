---
updated: 2026-09-20
by: lead
---
**Sessione del 20-09 pomeriggio: T-14 chiuso e profilo unico.** Motore spento, VGM 64, tutto pubblicato su `main` (`152d0f1..b0fb8a7`).

**M-18 T-14 chiuso.** Il sospetto scritto nel task era giusto: `serde_json` riordinava le chiavi degli argomenti e il template di Qwen3.6 li rende nell'ordine ricevuto. Nonio l'ha chiuso con `78b4099` il 19-09 all'1:29, **cinque ore dopo** la nostra misura dell'81,1%. A-B di oggi (16 compiti per braccio, stesso binario): su 245 richieste, quelle a riuso zero sono **una per compito** — la cartella nuova, non una divergenza. Costo fisso pari (0,43 spenti contro 0,42 accesi), quindi **il −82% non c'è più da incassare**.

**Il profilo è uno solo**, sull'installazione e nel prodotto: `qwen3.6-35b-a3b.q8_0.vulkan`. Gli altri 21 in `profiles/archivio/`. Aethera ora lo semina in `profiles/` alla prima esecuzione (regola dei template: solo se manca). Verificato accendendo davvero il motore a 262144: pronto in 32,7 s, una richiesta vera servita senza ragionamento.

**Baseline del prodotto** in `reports/baseline-prodotto-2026-09.md` e dentro le note del profilo: 16/16, 42,8 compiti/ora, riuso 90,3%, prefill 1,61 s/richiesta, costo fisso 0,42 s, una sola richiesta senza riuso per compito. Sotto 2 compiti o il 10% è rumore. Il riferimento continuo sono le sessioni vere: `python .lmbrain-lite/batteria/baseline.py --ultime 5` legge i `runs/<id>/` e stampa gli stessi numeri (verificato contro i due bracci dell'A-B).

**Due cose che solo l'operatore può fare:**
- il kit ha ancora **due milestone attivi**, M-18 e M-19: rimetterne uno su `approved` (un agente non può, `approved` è riservato all'operatore);
- il repo di **Nonio è rimasto su `main`** (era su `famiglia-argomenti-strumenti`, pubblicato e non unito): dirmi se lo riporto lì.
