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
