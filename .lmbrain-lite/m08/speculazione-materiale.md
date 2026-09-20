# Materiale per il milestone sulla speculazione

**Non è un milestone.** È la roba raccolta il 20-09-2026 mentre si preparava la notte di M-19
(Ling-3.0-flash contro il G1 Q8). Il milestone si scrive **dopo il verdetto** di M-19, perché la
sua forma dipende da quale modello resta il profilo principale.

## Perché questa direzione

Splash (`incoai/splash`, Apple/Metal, M3+) dichiara 2,0× contro oMLX/Ollama e spedisce i modelli
**con i draft DFlash 2**. Il suo guadagno non viene dai kernel, viene dalla speculazione e da una
memoria molto più larga. Sui kernel la nostra strada è già stata provata e misurata:

| lavoro a basso livello | al banco | sulla batteria |
|---|---|---|
| patch `int8-coopmat`, b10991+moro1 (PR ggml-org#27952, mirata a AMD RDNA3/RDNA4) | prefill **+33%** a 7k, +27% a 21k | 11,5 e 10,1 compiti/ora contro 10,8 e 12,1 della build liscia — **zero** |

Il motivo: nel giro agentico il riuso del prefisso è 89-99% e il prefill quasi non gira. Nei giri
col ragionamento, 480 s di decode contro 101 di prefill. **Il decode è il bersaglio, e il decode su
questa macchina si accorcia solo in tre modi: meno byte per token, più banda, o più token per
lettura dei pesi.** Il terzo è la speculazione, ed è il meno esplorato dei tre.

## Il fatto nuovo: l'accettazione nel lavoro vero

Mai calcolata prima. Da `telemetry.jsonl` di tutti gli avvii in `C:\AetheraData\runs`, sommando
`draft_n` e `draft_accepted` richiesta per richiesta:

| profilo | richieste | token di bozza | accettati | **accettazione** |
|---|---:|---:|---:|---:|
| `qwen3.6-35b-a3b.q8_0.vulkan` (G1, MTP 3) | 4.092 | 1.246.492 | 1.027.670 | **82,4%** |
| `qwen3.6-35b-a3b.q4_k_m.vulkan` (MTP 3) | 2.136 | 413.125 | 290.741 | 70,4% |
| `qwen3-coder-next.q4_k_m.vulkan` (MTP 3) | 32 | 15.534 | 12.277 | 79,0% |
| `qwen3.6-35b-a3b.q6_k.vulkan` (MTP 3) | 131 | 26.637 | 20.887 | 78,4% |
| `qwen3.6-35b-a3b.q4_k_m.vulkan.ngram` | 196 | 27.552 | 7.050 | **25,6%** |
| `qwen3-coder-next.q4_k_m.vulkan.ngram` | 106 | 18.544 | 4.414 | 23,8% |

Due cose da notare:

1. **Gli n-grammi accettano un quarto di quello che accetta MTP** (25,6% contro 82,4%), su decine di
   migliaia di token. Conferma il verdetto di M-18 T-03, ma ora con un numero da lavoro vero.
2. **Il Q8 accetta più del Q4** (82,4% contro 70,4%): la testa MTP è più fedele a precisione alta.
   Non era scritto da nessuna parte, e dà un'altra ragione al Q8 oltre a quelle già note.

## La domanda che il milestone deve aggredire

Con `draft_n_max = 3` e accettazione all'82,4%, ogni verifica dovrebbe emettere circa **3,5 token
per una sola lettura dei pesi**. Su una macchina limitata dalla banda, dove la lettura dei pesi è
quasi tutto il costo, quello dovrebbe valere un fattore vicino a 3.

Misurato (M-08 T-05, sul Q4): **+36%** sul codice, e **−15%** sul contesto lungo.

**Perché l'82% di accettazione compra solo il 36%?** Questa è la domanda. Il divario fra 3,5× teorico
e 1,36× reale è esattamente il margine che Splash sembra prendersi. Ipotesi da separare:

- la testa MTP costa un passaggio suo, e non è gratis come la verifica;
- verificare 4 token insieme non è più «una lettura dei pesi» su questo backend (batch piccolo che
  diventa compute-bound sulla 890M invece che bandwidth-bound);
- l'accettazione aggregata all'82% nasconde una distribuzione a gruppi: molte verifiche con 3 su 3 e
  molte con 0 su 3, che rendono poco;
- il costo fisso per richiesta mangia il guadagno sui turni corti.

## Leve da provare, con la regola di M-18

`draft_n_max` oltre 3 (se l'accettazione regge, allungare la bozza dovrebbe pagare) · un modello di
draft separato invece della testa MTP (è quello che spedisce Splash) · la lunghezza adattiva già
misurata in T-05, rivista alla luce dell'82% · il comportamento a contesto lungo, dove oggi la
speculazione **toglie** il 15%.

**Si misura sulla batteria di coding, non al banco.** Il 18-09 il banco ha sbagliato due volte su
due, e la riga dell'int8 in cima a questo file è la terza.

## Da decidere quando si scrive il milestone

- il modello di riferimento: G1 Q8 o Ling-3.0-flash, secondo il verdetto di M-19;
- se Ling vince, va ricordato che **la conversione di bartowski non ha i tensori MTP**
  (`nextn_predict_layers = 1` nei metadati, ma `mtp_types` vuoto): su Ling la speculazione oggi non
  esiste, e sarebbe una leva da costruire, non da tarare.
