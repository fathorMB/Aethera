# Misure del motore sulla Minisforum — settembre 2026

> **Stato: in scrittura.** Le tabelle si riempiono man mano che le misure finiscono, nella notte fra
> il 16 e il 17 settembre 2026. Ogni riga viene da un avvio registrato da Aethera: il `run id` nella
> colonna di destra è il manifest, il log e la telemetria di quell'avvio, in `C:\AetheraData\runs\`.

Questo rapporto sostituisce con misure fatte su **questa** macchina i numeri di altre macchine che
lo studio `design/studio-motore-2026-09/` citava da fonti esterne. Dove una misura contraddice lo
studio, la correzione è segnata con **[correzione]** e va riportata nella pagina indicata.

## Come sono state prese

**La macchina, mentre misurava.** Minisforum AI X1 Pro, AMD Ryzen AI 9 HX 470 con Radeon 890M,
96 GB di LPDDR5X di cui **48 GB assegnati alla GPU** (Windows ne vede 47,65 GiB). Driver GPU
32.0.22042.1, driver NPU 32.0.203.314. Modalità di alimentazione **«Massime prestazioni»**:
`powercfg /getactivescheme` dice «Bilanciato» e mente, perché l'overlay sta in
`HKLM\SYSTEM\CurrentControlSet\Control\Power\User\PowerSchemes\ActiveOverlayAcPowerScheme` e vale
`ded574b5-45a0-4f42-8737-46345c09c238`. Adrenalin **non** aggiornato (la 26.9.1 ha la regressione
DPM). Pesi su `C:`, 1.769 GB liberi.

**Un motore alla volta, nessun client.** Prima di ogni avvio il banco controlla che non ci sia un
`llama-server` acceso e salta la misura se ne trova uno: una misura presa mentre lavora qualcun
altro misura la contesa, non la leva.

**Ogni variante è un avvio di Aethera da profilo**, non una riga di comando scritta a mano:
`launch::prepare` + `Engine::start`, gli stessi del pulsante «Avvia». Da lì vengono la riga esatta,
il manifest, il log e la memoria misurata dopo il caricamento.

**I prompt sono congelati**, versionati in `.lmbrain-lite/m08/`: `prompt-7k.txt` (**7.015 token**) e
`prompt-21k.txt` (**20.618 token**) sono sorgenti veri di questo repository, concatenati in un ordine
dichiarato e tagliati su un confine di riga; il conto dei token viene da `/tokenize` del modello
vero, non da una stima a caratteri. Il contenuto passa per il template del modello con il
ragionamento esplicito **spento** (`enable_thinking: false`), che è la modalità del profilo G1 —
senza template un modello istruito risponde con un EOS e si misurerebbe il solo prefill.

**Cinque giri più un riscaldamento non contato**, mediana e scarto tipo. I numeri sono quelli che
manda il motore (`timings` della risposta), scritti grezzi in `C:\AetheraData\m08\<misura>.jsonl`:
il rapporto aggrega, non riscrive. La quota di prompt riusata dalla cache è `timings.cache_n` della
risposta, confermata leggendo `/slots` mentre la richiesta gira; **non** viene da
`/metrics`, il cui contatore `prompt_tokens_cached_total` sale all'avvio della richiesta successiva.

---

<!-- SEZIONI: le tabelle vengono aggiunte qui sotto man mano -->
## T-02 — Quanta banda raggiunge davvero la 890M

**Il metodo, perché il numero dipende da questo.** I tok/s si trasformano in GB/s moltiplicandoli
per i byte di pesi che il motore legge a **ogni token generato**. Quei byte non sono la dimensione
del file: su un MoE il file contiene 256 esperti per blocco e per token se ne leggono 8. Sono stati
sommati tensore per tensore dall'intestazione GGUF (`m08_bytes`), con tre regole dichiarate: la
tabella di embedding **non** entra (per token se ne legge una riga), la testa di uscita entra tutta,
i tensori degli esperti entrano in proporzione `expert_used / expert_count`. Prova che la somma è
giusta: il totale dei tensori chiude con la dimensione del file a meno di 11 MB su 22,3 GB.

| modello | byte letti per token | decode tok/s (5 giri) | **banda utile** | prefill tok/s (512) |
|---|---:|---:|---:|---:|
| Qwen3-8B Q4_K_M (denso) | 4,672 GB | 13,418 ± 0,025 | **62,7 GB/s** | 244,4 ± 3,2 |
| Qwen3.6-35B-A3B Q4_K_M (MoE, 8/256) | 2,265 GB | 21,830 ± 0,038 | **49,4 GB/s** | 345,6 ± 10,5 |

**Verdetto: il limite non è la piattaforma.** Il denso arriva a 62,7 GB/s, dentro la finestra
60-67 GB/s che lo studio dava come tetto raggiungibile su questa memoria: se il collo di bottiglia
fosse il BIOS, il driver o i clock, non ci arriverebbe nemmeno lui. Al MoE manca il 21%, ed è il
costo dei kernel e della raccolta degli esperti — un lavoro di indirizzamento che il denso non fa.

**[correzione]** Lo studio (pagina «Motore») parla di «~42 GB/s» per il MoE. Sono 49,4 GB/s: la
cifra precedente nasceva da un conto dei byte per token diverso da questo. Il 79% del tetto è un
divario normale per un MoE, non un sintomo di qualcosa di rotto.

Condizioni: b10809 Vulkan, `-ngl 999 -fa 1 -t 12`, 5 ripetizioni, `llama-bench`.
Righe grezze: `m08/T-02-llama-bench.jsonl`.

---

## T-03 — L'SSD sul file dei pesi

DiskSpd in sola lettura, unbuffered (`-Sh`), venti secondi per combinazione, sul GGUF vero del 35B
(20,75 GiB) e non su un file di prova.

| prova | MB/s | IOPS | latenza media | 99° percentile |
|---|---:|---:|---:|---:|
| 4 KiB casuale QD1 | 27 | 6.527 | 0,15 ms | 0,24 ms |
| 4 KiB casuale QD8 | 171 | 41.781 | 0,19 ms | 0,76 ms |
| 4 KiB casuale QD32 | 296 | 72.253 | 0,36 ms | 6,05 ms |
| 2 MiB casuale QD1 | 1.952 | 931 | 1,07 ms | 1,52 ms |
| 2 MiB casuale QD8 | 771 | 367 | 21,78 ms | 39,00 ms |
| 2 MiB casuale QD32 | 1.057 | 504 | 63,36 ms | 136,16 ms |
| 2 MiB sequenziale QD8 | **4.378** | 2.088 | 3,83 ms | 13,73 ms |
| 2 MiB sequenziale QD32 | 2.321 | 1.107 | 28,95 ms | 74,04 ms |
| 1 MiB sequenziale QD32, 4 thread | 4.321 | 4.121 | 31,06 ms | 52,48 ms |

---

## T-04 — I checkpoint dello stato ricorrente: la leva non serve, ma la misura ha trovato altro

**Che cosa si è provato.** Due avvii che differiscono per la sola riga dei checkpoint, quattro turni
della stessa conversazione per giro: turno 0 a freddo, turno 1 con il **prefisso identico** (cambia
solo la coda), turni 2 e 3 con **una riga cambiata a metà del prompt** — il caso dell'agente che
riscrive un file già in conversazione. Cinque giri più un riscaldamento.

| variante | turno | prefill tok/s | decode tok/s | token elaborati | token riusati |
|---|---:|---:|---:|---:|---:|
| oggi (default 32 / 8192 / cache-ram 8192) | 0 (freddo) | 350,2 ± 1,8 | 18,89 ± 0,76 | 7.096 | — |
| oggi | 1 (prefisso identico) | 317,3 ± 1,5 | 18,92 ± 0,43 | 4.106 | 3.000 |
| oggi | 2 (riga cambiata) | 317,2 ± 2,4 | 19,00 ± 0,91 | 4.120 | 3.010 |
| oggi | 3 (riga cambiata) | 321,8 ± 1,7 | 20,11 ± 1,07 | 4.096 | 3.034 |
| checkpoint 128 / 128 / 15000 / kv-unified | 0 (freddo) | 349,7 ± 1,1 | 17,62 ± 1,39 | 7.096 | — |
| checkpoint | 1 (prefisso identico) | 318,0 ± 1,6 | 18,31 ± 0,92 | 4.106 | 3.000 |
| checkpoint | 2 (riga cambiata) | 315,7 ± 1,6 | 19,57 ± 0,61 | 4.120 | 3.010 |
| checkpoint | 3 (riga cambiata) | 321,9 ± 1,3 | 18,74 ± 0,78 | 4.096 | 3.034 |

**Verdetto: scartata.** Le due colonne dei token riusati sono identiche **fino all'unità**, turno per
turno. Prefill e decode coincidono dentro il rumore. `--ctx-checkpoints 128 --checkpoint-min-step
128 --cache-ram 15000 --kv-unified` non cambia niente su questo modello con questa build: non entra
nei profili standard.

**[correzione]** Lo studio (pagina «Piano di prova», riga 1.1) si aspettava «TTFT dei turni con
prefisso divergente da decine di secondi a meno di uno». Non succede.

**La scoperta vera, che vale più della domanda di partenza.** Guardare la colonna «token riusati»
insieme a «token elaborati» dice una regola precisa:

> il motore riusa il prefisso fino a **lunghezza del prompt meno un ubatch**, e rielabora sempre
> l'ultimo ubatch — **anche quando il prompt non è cambiato di un carattere**.

Si legge nei numeri: 7.106 − 4.106 = 3.000 al turno 1, dove l'unica differenza dal turno 0 sono
dieci token in coda. Con `-ub 4096` sono **4.096 token di prefill per ogni turno**, circa **13
secondi**, pagati sempre. Per un agente che fa decine di turni sullo stesso contesto questo è il
costo dominante, molto più del decode: da qui nasce la misura T-13.

Righe grezze: `m08/T-04.jsonl`. Condizioni e run id: `m08/T-04.meta.json`.

---

## T-05 — MTP adattivo contro MTP 3 fisso, e contro nessuna speculazione

Tre avvii, cinque giri più un riscaldamento, campionamento della model card
(0,7 / 0,8 / 20 / 0 / presence 1,5), 384 token generati.

| variante | codice 7k: prefill | codice 7k: **decode** | riassunto 21k: prefill | riassunto 21k: **decode** | VRAM dedicata |
|---|---:|---:|---:|---:|---:|
| senza speculazione | 338,7 ± 1,2 | **20,55 ± 0,04** | 316,3 ± 0,3 | **19,22 ± 0,02** | 21,24 GiB |
| MTP 3 fisso (oggi) | 353,8 ± 3,6 | **28,00 ± 1,52** (+36%) | 302,9 ± 0,7 | **16,29 ± 0,30** (−15%) | 22,68 GiB |
| MTP adattivo 2/4/0,75 | 365,6 ± 0,9 | **27,39 ± 0,63** (+33%) | 307,6 ± 0,5 | **17,57 ± 0,51** (−8,6%) | 22,68 GiB |

**Verdetto: da tenere, ma non per il motivo che diceva il piano.** L'adattivo non porta il «+5-15%
sul codice» atteso: sul codice **toglie** due punti percentuali rispetto a MTP 3 fisso. Quello che fa
è **dimezzare il danno sul contesto lungo**, da −15% a −8,6%: il prezzo noto del profilo G1. Quindi:
con un profilo solo per i due usi, l'adattivo è la scelta migliore; con due profili separati, sul
codice resta meglio MTP 3 fisso.

Da notare, perché non era scritto da nessuna parte: **MTP costa 1,44 GiB di VRAM dedicata**
(22,68 contro 21,24 senza speculazione). Il prefill non c'entra con la speculazione, e le differenze
fra le tre righe (339 / 354 / 366) sono rumore fra avvii, non un effetto della leva.

**[correzione]** `--spec-draft-adaptive`, che il piano cita come riga di comando, **non esiste** in
b10809: la speculazione adattiva si ottiene con `--spec-draft-n-min` / `--spec-draft-n-max` /
`--spec-draft-p-min`, ed è così che è stata misurata. (`--adaptive-target` esiste ma riguarda il
campionamento, non la speculazione.) Va corretto nello studio, pagina «Piano di prova», riga 1.2.

---

## T-13 — Perché ogni turno costa, e quanto: il punto di ripresa sta a «prompt meno un ubatch»

Misura nata da T-04. Quattro turni per giro sullo stesso prompt da 7k: turno 0 a freddo, turno 1 che
cambia **solo in coda**, turni 2 e 3 con **una riga cambiata al 42% del prompt**. Quattro giri più un
riscaldamento. `-b` resta 4096 ovunque: l'unica variabile è `-ub`.

| variante | turno | token rielaborati | riusati | prefill s | decode s | **turno s** |
|---|---:|---:|---:|---:|---:|---:|
| ub-512 | 1 (coda) | 526 | 6.584 | 3,71 | 6,98 | **10,70** |
| ub-512 | 2-3 (riga cambiata) | 7.134 | **0** | 27,1 | 6,4 | **33,5** |
| ub-1024 | 1 (coda) | 1.038 | 6.073 | 6,67 | 6,89 | **13,57** |
| ub-1024 | 2-3 | 7.135 | **0** | 33,8 | 6,4 | **40,2** |
| ub-2048 | 1 (coda) | 2.062 | 5.049 | 7,76 | 6,95 | **14,72** |
| ub-2048 | 2-3 | 7.135 | **0** | 21,3 | 6,4 | **27,7** |
| ub-4096 (oggi) | 1 (coda) | 4.106 | 3.005 | 12,83 | 6,91 | **19,74** |
| ub-4096 | 2-3 | 4.108 | 3.027 | 12,8 | 6,5 | **19,3** |
| ub-4096 + cache-reuse 256 | 1 (coda) | 4.106 | 3.012 | 12,80 | 6,78 | **19,58** |
| ub-4096 + cache-reuse 256 | 2-3 | 4.108 | 3.034 | 12,7 | 6,2 | **18,9** |

**La regola.** Il motore tiene **un solo punto di ripresa**, e sta a `lunghezza del prompt − ubatch`.
Da lì discende tutto:

1. se il turno cambia **solo in coda**, rielabora esattamente un ubatch — 526 token (3,7 s) con
   `-ub 512`, 4.106 (12,8 s) con `-ub 4096`: **tre volte e mezzo di differenza**;
2. se il turno cambia **prima** di quel punto, non riusa **niente** e rielabora tutto il prompt. È
   quello che succede a `-ub` 512, 1024 e 2048 nei turni 2 e 3;
3. `-ub 4096` se la cava in entrambi i casi solo perché il suo punto di ripresa cade al 42% del
   prompt, **appena prima** della modifica. È una fortuna che dipende da dove cade la modifica, non
   una proprietà del valore.

| variante | media dei turni 1-3 |
|---|---:|
| ub-4096 + cache-reuse 256 | **19,15 s** |
| ub-4096 (la riga di oggi) | **19,44 s** |
| ub-2048 | 23,39 s |
| ub-512 | 25,91 s |
| ub-1024 | 31,31 s |

**Verdetto: `-ub 4096` resta, e adesso si sa perché.** Non per il prefill a freddo (dove il vantaggio
su 2048 è del 5%: 26,6 s contro 27,5), ma perché tiene il punto di ripresa abbastanza indietro da
sopravvivere a una modifica a metà prompt. Su una conversazione che cresce **solo in coda** — il caso
più comune di un agente — `-ub 512` sarebbe invece tre volte e mezzo più rapido: se un profilo
«conversazione lunga» avrà senso, questa è la leva.

**`--cache-reuse 256` è da scartare**: non cambia un numero (19,15 contro 19,44 s, dentro il rumore) e
i token riusati sono identici. Lo spostamento della cache KV non si applica a questo modello ibrido.

Conferma di quanto già visto in `minis-config` §7: il prefill a freddo **non è monotono** in `-ub` —
353 tok/s a 4096, 335 a 2048, 262 a 512, **211 a 1024**. Ogni valore si misura, non si interpola.

---

## T-06 — Cache KV q8_0 contro f16, a 32k, 64k e 128k

Sei avvii, tre giri più un riscaldamento, un carico solo: il riassunto da 20.618 token. Il carico
corto è stato tolto di proposito — la cache KV si vede a contesto lungo, non su un prompt breve.

| contesto | KV | prefill tok/s | **decode tok/s** | VRAM dedicata | VRAM condivisa |
|---|---|---:|---:|---:|---:|
| 32k | f16 | 303,1 ± 0,8 | **18,80 ± 0,79** | 22,68 GiB | 1,24 GiB |
| 32k | q8_0 | 309,9 ± 0,4 | **17,38 ± 0,54** (−7,6%) | 22,40 GiB | 1,24 GiB |
| 64k | f16 | 244,1 ± 0,3 | **17,62 ± 0,44** | 23,39 GiB | 2,36 GiB |
| 64k | q8_0 | 247,4 ± 0,1 | **16,65 ± 0,36** (−5,5%) | 23,06 GiB | 2,12 GiB |
| 128k | f16 | 308,0 ± 0,2 | **17,64 ± 1,23** | 26,00 GiB | 2,99 GiB |
| 128k | q8_0 | 316,2 ± 0,5 | **17,04 ± 0,47** (−3,4%) | 24,83 GiB | 2,99 GiB |

**Verdetto: scartata.** `q8_0` peggiora il decode a **tutti** i contesti provati e risparmia poco:
0,28 GiB a 32k, 0,33 a 64k, 1,17 a 128k. I due punti percentuali guadagnati sul prefill non
compensano. Non entra nei profili standard.

**[correzione]** Lo studio (riga 1.4 del piano) si aspettava «nessun guadagno sotto 64k, da +10%
oltre». Oltre i 64k il guadagno resta **zero** e il costo resta. La ragione si vede nella colonna
della VRAM: su questo modello solo **10 blocchi su 41** hanno attenzione piena, quindi la cache KV è
piccola in partenza — da 32k a 128k la VRAM dedicata sale solo da 22,68 a 26,00 GiB, cioè ~35 MiB
ogni 1.000 token. Non c'è abbastanza KV perché quantizzarla convenga.

**Sulla «coerenza dell'output a temperatura 0» chiesta dal milestone:** non è misurabile
confrontando il testo generato. Su Vulkan **due giri identici della stessa variante danno già testi
diversi** (visto in T-04 e confermato qui): il backend non è deterministico, quindi un'impronta
diversa fra f16 e q8_0 non direbbe niente. Il confronto di qualità che regge è la perplessità sullo
stesso testo — `run-T06-perplexity.sh`, risultati più sotto.

**Un'anomalia che non so spiegare e che lascio dichiarata:** il prefill a **64k** è più lento del 20%
sia rispetto a 32k sia rispetto a 128k (244 contro 303 e 308), in entrambe le varianti di KV, con uno
scarto tipo di 0,3 tok/s. Non è rumore. È lo stesso tipo di non monotonia già vista su `-ub` in
T-13 e in `minis-config` §7: su questo backend i parametri non si interpolano, si misurano.

---

## T-07 — La build b10991 contro la b10809

Scaricata dal Catalogo con il digest di GitHub verificato **prima** dell'estrazione. Stesso profilo,
nessun altro cambiamento. Cinque giri più un riscaldamento.

| build | codice 7k: **prefill** | codice 7k: decode | riassunto 21k: **prefill** | riassunto 21k: decode | VRAM |
|---|---:|---:|---:|---:|---:|
| b10809 (5266f24da) | 353,2 ± 0,8 | 25,25 ± 2,71 | 303,2 ± 0,3 | 16,81 ± 0,35 | 22,68 GiB |
| b10991 (930e2fa59) | **377,0 ± 1,7** (+6,7%) | 26,52 ± 2,21 | **320,6 ± 0,8** (+5,7%) | 16,61 ± 0,63 | 22,73 GiB |

**Verdetto: entra.** Il prefill guadagna il 6,7% sul codice e il 5,7% sul contesto lungo, con scarti
tipo sotto i 2 tok/s: è molto più del rumore, e su questa macchina **il prefill è il collo di
bottiglia**. Il decode non si muove: la differenza sul codice (26,52 contro 25,25) ha uno scarto tipo
di oltre 2 e non si separa, e sul contesto lungo va semmai un filo peggio. La memoria è identica.
Il profilo G1 gira sulla build nuova senza toccare una leva.
