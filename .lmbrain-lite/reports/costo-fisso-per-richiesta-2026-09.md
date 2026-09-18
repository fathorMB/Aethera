# Il costo fisso di ogni richiesta: che cos'è e come si toglie

> **M-17, 18-09-2026 notte.** Misure a VGM 48, macchina riavviata la sera prima, sentinella della
> deriva prima e dopo ogni blocco (tutte entro il 3 %). Sigle: G1 = Qwen3.6-35B-A3B Q4_K_M con MTP,
> G3 = Qwen3-Coder-Next Q4_K_M, D8 = Qwen3-8B Q4_K_M (denso, come metro).
> Dati grezzi in `<radice>\m17\T-0*.jsonl`; script `m17/scenario.py` e `m17/fasi.py`.

**In breve.**
- Ogni richiesta di un agente paga **1,8-2,2 secondi** prima del primo token, anche quando i token
  nuovi sono venti. Sulla batteria di M-15 vale il 10-12 % del tempo sul G3 e il **19 %** sul G1.
- **Non è calcolo: è una copia.** Il 94 % di quel tempo è la creazione dei *context checkpoint*
  dello stato ricorrente, copiati dalla GPU alla RAM **un tensore per volta**: 75 MiB a
  **130 MB/s** su una macchina che ne fa 75.000.
- **Esiste solo sui modelli ibridi.** Sul denso 8B il costo fisso è zero.
- **Una leva lo toglie quasi tutto**: `--ctx-checkpoints 0` porta il G3 da 2,20 a 0,40 s e il G1 da
  1,85 a 0,33 s, senza toccare niente altro.
- **Con MTP il costo triplica in profondità**: a 128k il G1 paga 4,99 s per richiesta contro 1,76 s
  senza. Il colpevole è la cache del draft, copiata dentro ogni checkpoint.

## 1. La misura (T-01, T-02)

Scenario: una conversazione che cresce in coda come quella di un agente — un primo messaggio con
~4k token di codice, poi venti turni che rimandano tutto e aggiungono da 20 a 1.200 token nuovi.
Tre giri per variante, 60 punti ciascuna; il costo fisso è l'intercetta della regressione di
`prompt_ms` su `prompt_n`.

| variante | costo fisso | costo per token | mediana con ~57 token nuovi |
|---|---:|---:|---:|
| G3, chat | 2.202 ms | 3,64 ms | 2.354 ms |
| G3, `/completion` | 1.575 ms | 3,56 ms | 1.745 ms |
| G1, chat | 1.855 ms | 2,59 ms | 2.099 ms |
| G1, `/completion` | 1.431 ms | 2,95 ms | 1.608 ms |
| **D8 (denso), chat** | **−16 ms** | 4,98 ms | 429 ms |

Tre letture:
- **il costo è dei modelli ibridi**: sul denso non esiste, quindi non è template, tokenizzazione né
  confronto del prefisso;
- **la chat costa 0,4-0,6 s più di `/completion`** a parità di token nuovi: è un checkpoint in più,
  quello che nasce all'inizio di ogni messaggio utente;
- **il riuso è pieno**: `cache_n` copre sempre prompt precedente più token generati, nessuna
  ri-elaborazione nascosta. Fuori dai tempi dichiarati dal motore restano 25-40 ms per richiesta.

## 2. Dove vanno i millisecondi (T-03)

Log del motore a `-lv 5`, letto da `m17/fasi.py`. G3, richiesta con 57 token nuovi:

| fase | tempo | quota |
|---|---:|---:|
| confronto del prefisso e `seq_rm` | 0 ms | — |
| **creazione di 3 checkpoint (226 MiB)** | **2.254 ms** | **94 %** |
| decode dei token nuovi (3 batch) | 16 ms | 0,7 % |
| `init sampler` | 0,5 ms | — |
| ultimo decode e campionamento | 117 ms | 5 % |
| **totale** | **2.395 ms** | |

Un singolo checkpoint impiega 524-1.098 ms per 75,4 MiB. Sono **~130 MB/s**: non è la banda di
memoria (75 GB/s misurati in M-08), è il costo di `ggml_backend_tensor_get` chiamato per ogni
tensore, che sul backend Vulkan fa submit, attesa della fence e pulizia dei command pool ogni volta.
Nel sorgente di llama.cpp c'è il TODO che chiede di raggrupparli
(`llama-context.cpp:2586`).

**Perché tre checkpoint.** Il motore spezza il prompt e ne crea uno a inizio del messaggio utente,
uno a `n − 4 − ubatch` e uno a `n − 4`; vicino alla fine del prompt li crea **sempre**, ignorando
anche `--checkpoint-min-step`.

**Sul G1 con MTP** i checkpoint sono 2,6 per richiesta, 596 ms l'uno (127 MB/s), e la dimensione
**cresce col contesto**: da 62,8 a 78,4 MiB in un solo giro, perché il checkpoint include tutta la
cache del modello draft.

## 3. Con la profondità (T-04)

Stesso scenario su un sorgente congelato di 1 MB, a tre profondità:

| profilo | 32k | 64k | 128k |
|---|---:|---:|---:|
| G1 con MTP (profilo standard di oggi) | 1.855 ms | 3.344 ms | **4.987 ms** |
| G1 senza speculazione | — | 1.746 ms | 1.755 ms |
| G3 (non ha speculazione) | 2.202 ms | 2.119 ms | 2.293 ms |

**Il costo fisso non cresce con il contesto**: è piatto sia sul G3 sia sul G1 senza speculazione.
Cresce solo con MTP, e triplica. A 128k una richiesta di agente sul G1 paga **6,3 secondi** di sola
preparazione (mediana misurata, 56 token nuovi) contro 2,3 senza MTP.

Per i compiti dell'operatore, che chiedono 128-256k, **MTP è un costo e non un guadagno**: vedi
anche il rapporto di M-18 sul decode, dove la speculativa a n-grammi lo batte senza portarsi dietro
questo prezzo.

## 4. Le leve (T-05)

| leva | G3 | G1 | verdetto |
|---|---:|---:|---|
| profilo di oggi (`ctx-checkpoints` 32) | 2.202 ms | 1.855 ms | riferimento |
| **`--ctx-checkpoints 0`** | **400 ms** | **329 ms** | **−82 % e −82 %** |
| `--ctx-checkpoints 1` | 2.123 ms | 1.861 ms | inutile |
| `--cache-ram 0` | 2.118 ms | 1.793 ms | inutile (sta fuori da `prompt_ms`) |

`--ctx-checkpoints 1` non serve perché i checkpoint di fine prompt si creano comunque: l'unico
valore che cambia qualcosa è **0**. Il costo per token non cambia con nessuna leva.

**Il prezzo.** Con i checkpoint spenti, un prompt che diverge in mezzo al testo già generato si
ricalcola da capo. Misurato sul G3 con la stessa riga cambiata a ogni turno, contesto ~5k:

| | token riusati | ri-elaborati | tempo per turno |
|---|---:|---:|---:|
| checkpoint 32 | 1.763 | 3.366 | 13,9 s |
| checkpoint 0 | 0 | 5.127 | 17,3 s |

Una divergenza costa **3,4 s in più**; una richiesta normale ne costa **1,8-2,2 in meno**. Il
pareggio sta intorno al 55 % di richieste divergenti. Con un harness che scrive solo in coda le
divergenze sono **zero**, quindi la leva conviene senza discussione.

**Avvertenza, e non è piccola.** La prova della divergenza è a 5k di contesto. A 128k un ricalcolo
completo costa minuti invece di secondi, quindi la leva va accesa **solo dopo** aver corretto le tre
cose che in Nonio rompono la sola aggiunta (`reports/nonio-harness-2026-09.md`, sezione sul riuso):
il ragionamento non rimandato, il turno troncato sostituito da un avviso, le chiamate malformate
tolte dalla cronologia. La prima delle tre è anche il motivo per cui oggi i profili tengono il
`thinking` spento.

## 5. Una patch? No (T-06)

Valutazione completa in `m17/patch-checkpoint-valutazione.md`. In breve:
- il flag `LLAMA_STATE_SEQ_FLAGS_ON_DEVICE` esiste (`llama.h:916`) e il server non lo usa mai; ma
  sposterebbe 32 × 75 MiB = 2,4 GiB dalla RAM, che qui abbonda, alla **VGM**, che è la risorsa
  scarsa, e romperebbe prompt cache, salvataggio su disco e `/slots`;
- **upstream ci sta già lavorando**: la PR ggml-org/llama.cpp **#28118** (bozza) lo applica ai soli
  checkpoint speculativi, con misure su Strix Halo che dicono 600 ms su 825 per giro — lo stesso
  fenomeno. Da seguire e ribasare nel fork quando esce dalla bozza;
- raggruppare le copie darebbe di più ma richiede un'API nuova nel backend Vulkan: fuori dalla
  regola del fork. Semmai un'issue upstream con i nostri numeri, che nessuno sembra avere.

## 6. Che cosa proporre

Per l'operatore, in ordine di valore:

1. **Correggere l'harness** perché la conversazione sia davvero in sola aggiunta (tre punti noti,
   con file e riga). È la condizione di tutto il resto, e serve comunque al contesto lungo.
2. **Poi** mettere `ctx_checkpoints = 0` nei profili standard: −1,8/−2,2 s su ogni richiesta, che
   su una sessione da 240 richieste sono **7-9 minuti**.
3. **Togliere MTP dal G1** e sostituirlo con la speculativa a n-grammi (rapporto di M-18): si
   guadagna sul decode e si toglie il triplicarsi del costo fisso in profondità.
4. Seguire la PR #28118; valutare un'issue upstream sul batching delle copie.

Niente di tutto questo è stato applicato ai profili standard: i profili di prova sono
`*.vulkan.ngram` e le leve si passano da riga di comando.

## 7. Che cosa Aethera dovrebbe mostrare

- Il costo fisso per richiesta è già calcolabile dai dati che la pagina Motore ha: basta la
  regressione di `prompt_ms` su `prompt_n` sulle richieste della sessione. Sarebbe un KPI onesto
  accanto a prefill e decode, e direbbe subito se un profilo sta pagando i checkpoint.
- Un avviso quando `speculative.type = "draft-mtp"` e il contesto servito supera i 64k: a quelle
  profondità MTP costa più di quanto rende.
- Il numero di checkpoint e i MiB copiati si leggono dal log a `-lv 4`: sarebbero una riga in più
  nel riquadro delle condizioni dell'avvio.
