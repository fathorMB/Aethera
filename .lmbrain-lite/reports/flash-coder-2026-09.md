# Qwen3.8-Flash-Coder (Jab1718): verdetto

> **M-11, 16/18-09-2026.** Misure a VGM 48, motore `b10991` Vulkan, `--load-mode none`, ubatch 2048,
> KV f16. Sigle: FC = Flash-Coder (taglio a 160 esperti, senza tabella n-gram), FN = Flash-Next
> intero, G1 = Qwen3.6-35B-A3B, G3 = Qwen3-Coder-Next. Chiuso con un **no** per decisione
> dell'operatore del 18-09, sui numeri qui sotto.

**In breve.**
- **Come agente non funziona: 0 compiti su 15** nella batteria di M-15 (Q4_K_M, 1,76 h, 121 chiamate
  agli strumenti respinte su 186: diff illeggibili ripetuti fino a 19 volte, chiamate scritte nel
  testo, comandi appesi). G1 e G3 ne risolvono 12. Il Q8_0 non è stato provato sulla batteria:
  facoltativo secondo il rapporto di M-15, e il verdetto sul Q4 basta.
- **È lento come FN, anche se sta tutto in VGM**: decode 10,5 tok/s (Q4_K_M) e 7,8 (Q8_0) contro
  21,8 di G1 e 17,8 di G3. Il taglio degli esperti non rende più veloce: per ogni token lavorano
  sempre 10 esperti. Il freno sembra il tipo di modello (`qwen4exp`) sul backend Vulkan, non la
  memoria.
- **Il riuso del prefisso è quello dei modelli ibridi**: 5.051 token su 7.099 (71 %) con il prefisso
  identico, zero dopo una riga cambiata a metà, identico a FN e spiegato dal codice (M-15 T-06).
- La scheda del modello (91 % pass@1) non descrive quello che fa qui.

## 1. Velocità e memoria (T-03, T-04a)

| quant | caricamento | GPU dedicata + condivisa | 7k prefill / decode | 21k prefill / decode | 30k prefill / decode | 58k prefill / decode |
|---|---:|---|---|---|---|---|
| Q4_K_M (28,4 GB) | 20 s | 28,4 + 0,59 GiB | 181,0 / 10,45 | 161,7 / 9,92 | 151,0 / 9,7 | 120,0 / 8,72 |
| Q8_0 (45,4 GB) | 28,7 s | 44,4 + 0,88 GiB | 169,0 / 7,60 | 153,2 / 7,27 | — | — |

- 5 giri a 7k e 21k; 2 giri (più riscaldamento) a 30k e 58k con ctx 65536, sorvegliante mai
  intervenuto, file di paging fermo.
- Il prefill cala del 34 % fra 7k e 58k (FN: −54 %); a 58k un prompt a freddo costa 8 minuti.
- Il Q8_0 a 64k di contesto non è stato provato: a 32k occupa già 44,4 GiB dedicati.

## 2. Riuso del prefisso (T-04b)

Quattro turni su `/completion`, prompt da 7,1k, temperatura 0, 5 giri per quant:

| turno | che cosa cambia | Q4_K_M token riusati | Q8_0 token riusati |
|---|---|---:|---:|
| 1 | coda diversa, prefisso identico | 5.051 su 7.099 | 5.051 su 7.099 |
| 2 | una riga cambiata a metà | 0 | 0 |
| 3 | uguale al turno 2 | 0 | 0 |

Stesso comportamento di FN (M-10 T-09): riparte dal checkpoint a `n − 4 − ubatch`; dopo una
divergenza non c'è niente da riusare. Per una chat che cresce in coda il riuso resta buono.

## 3. Coerenza (T-02)

Metadati giusti (160 esperti, 10 attivi, nessuna chiave PLE, lingue dichiarate en/vi/zh). Risposte
deboli già sul prompt corto: codice sbagliato o che non compila, ragionamento che finisce con
contenuto vuoto, Q4_K_M che in italiano degenera in «assistant» ripetuto. I GGUF di mradermacher
(08-09, forse di una versione precedente dei pesi) non sono stati usati.

## 4. Verdetto

**No.** Né come modello quotidiano né come «modello per i compiti difficili»: non risolve compiti
che G1 e G3 risolvono, e va alla metà della loro velocità. La ricetta di profilo non serve; i
profili di prova `qwen3.8-flash-coder.*.vulkan` restano nella radice dati come documentazione delle
misure e i pesi restano sul disco (l'operatore ha spazio).

**Confronto con FN** (`reports/flash-next-2026-09.md`): FC è più veloce di FN del 12-30 % e sta in
VGM 48 senza spill, ma il vantaggio non basta a renderlo utile; il taglio degli esperti fatto da
altri, su compiti di altri, ha tolto qualcosa che ai nostri compiti serve (formato delle chiamate
agli strumenti, italiano). Se mai si tagliasse un modello, il conteggio andrebbe fatto sulle nostre
tracce (idea parcheggiata, vedi il LOG del 18-09).

Dati grezzi: `<radice>\m11\T-02-*.jsonl`, `T-03.jsonl`, `T-04a.jsonl`, `T-04b.jsonl` con i
`*-memoria.csv` e i `*.meta.json`; batteria in `<radice>\m15\risultati`.
