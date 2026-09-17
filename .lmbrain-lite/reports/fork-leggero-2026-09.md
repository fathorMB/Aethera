# Fork leggero di llama.cpp — rapporto (M-14, 17-09-2026)

Sessione notturna del 17-09, sub-agente `m14-fork`, branch `m14-fork-leggero` (non unito, non
pubblicato). Macchina: Minisforum, Ryzen AI 9 HX 470, Radeon 890M, **VGM 48**, driver GPU
32.0.31041.1004, overlay «Massime prestazioni»: le stesse condizioni di M-08 dopo l'aggiornamento
del driver. Percorsi scritti con i segnaposto del README (`<radice>`, `<pesi>`).

## In breve

- **La pipeline funziona.** `build.ps1` fa tag + patch → build Vulkan → `builds\` → provenienza →
  `machine.toml` in **2 minuti e mezzo** (compilazione 110–113 s, molto sotto i 10–15 minuti attesi).
- **Zero patch = ggml-org**, dentro il rumore, una volta tolto un effetto d'ordine (T-04).
- **La PR #27952 è ancora aperta**; ribasata su b10991 senza conflitti.
- **Verdetto della patch: non entra in `moro-ai`, per la regola scritta; la decisione vera spetta
  all'operatore.** La patch cambia le risposte a temperatura 0 contro una base che si ripete identica.
  La perplessità però resta la stessa, e sul Coder-Next G3 il prefill sale del 33 % a 7k e del 27 %
  a 21k (sul 35B G1 solo del 2,6 % e del 3,4 %). Se l'operatore accetta la perplessità come criterio
  di coerenza, per il G3 la patch va adottata (sezione T-06).
- **Windows Update ha riavviato alle 02:33** (KB5129195, Windows 26200.9457) e poi Smart App Control
  ha bloccato i motori, finché l'operatore non l'ha spento (07:52). T-06 è stato rifatto per intero
  dopo il riavvio. I numeri di prima e di dopo coincidono, tranne la base sul 21k del 35B (−2 %).

## T-01 — Toolchain

Confermata dalla build di prova, da riga di comando, di b10991 liscio:

| strumento | versione |
|---|---|
| MSVC (Build Tools 2022) | 19.44.35228 |
| CMake (incluso in Build Tools) | 3.31.6-msvc6 |
| Ninja (incluso in Build Tools) | 1.12.1 |
| Vulkan SDK | 1.4.357.0 (quello della release di ggml-org) |

`llama-server --version` risponde `version: 0.4.1-dev (build 10991, commit 930e2fa59)`, «built with
MSVC 19.44.35228.0». CMake e Ninja non sono nel PATH: lo script li prende da Build Tools (trovato con
`vswhere`) dopo aver caricato `VsDevCmd.bat -arch=x64`.

## T-02 — Il repo del fork

`<radice>\src\llama.cpp`: clone parziale (senza blob, 9 s), remote `upstream` = ggml-org con
l'indirizzo di push reso inservibile, rami `master`, `moro-ai`, `patch/int8-coopmat`. Le regole
stanno in `.lmbrain-lite/fork/README.md`; lo script le copia nel repo come `LEGGIMI-MORO.md`, escluso
da git. Niente su GitHub.

## T-03 — Lo script di build

`.lmbrain-lite/fork/build.ps1 -Tag b10991 [-Patch patch/a,patch/b] [-Serie n]`, percorsi da
`-Radice` o `AETHERA_RADICE`.

1. fetch del tag e di master;
2. per ogni ramo: `rebase --onto <tag> upstream/master <ramo>` (i commit della patch sono quelli
   fuori da master, qualunque fosse la base di prima);
3. `moro-ai` = tag + cherry-pick dei rami, nell'ordine dato;
4. cmake + ninja, Release, `GGML_VULKAN`, `GGML_BACKEND_DL`, `GGML_CPU_ALL_VARIANTS`,
   `LLAMA_BUILD_NUMBER=<tag>` (così `--version` dice il tag e il commit vero di `moro-ai`);
5. copia di `.exe` e `.dll` in `<radice>\builds\llama-b<numero>+moro<n>-win-vulkan-x64`;
6. `provenienza.toml` (id, base, commit della base, serie, commit, rami con i loro commit prima e dopo
   il rebase, data, durate, compilatore, opzioni, riga di versione);
7. voce `[[build]]` in coda a `machine.toml`, dopo una copia di sicurezza accanto
   (`machine.toml.prima-di-b10991_moro0-…`, `…_moro1-…`); le voci esistenti restano come sono.

Un conflitto ferma tutto con codice 2 e l'elenco dei file: **provato** con un ramo finto che tocca una
riga di `llama-bench.cpp` nata dopo b10991 — rebase annullato, ramo invariato, repo pulito.

**Tempi di build** (build pulite, macchina ferma):

| build | configurazione | compilazione (720–722 passi) | totale script |
|---|---:|---:|---:|
| b10991+moro0 | 8 s | 110 s | 140 s |
| b10991+moro1 | 7 s | 113 s | 151 s |

**Differenze note dalla release di ggml-org.** La release compila la parte CPU con clang (varianti
zen4, sapphirerapids… e `libomp`) e la parte Vulkan con MSVC. Qui è tutto MSVC: le varianti CPU
arrivano ad AVX-512 «icelake», senza zen4. Mancano l'interfaccia web (`LLAMA_USE_PREBUILT_UI=OFF`:
scaricarla sarebbe un download a ogni build, e Aethera usa solo le API) e HTTPS (OpenSSL non c'è).
Con tutti i layer sulla GPU la parte CPU non pesa sui numeri: lo dice T-04.

## T-04 — Zero patch contro ggml-org

`llama-bench` come in M-08 T-02 (`-ngl 999 -fa 1 -t 12 -r 5 -p 512 -n 128 -pg 4096,128`), una build
dopo l'altra per ogni modello. Script `.lmbrain-lite/m14/run-T04-banda.sh`, righe in
`<radice>\m14\T-04-*.jsonl` e `T-04b-*.jsonl`. tok/s, media ± sd di 5 giri.

| modello | prova | ggml-org b10991 | moro0 |
|---|---|---:|---:|
| Qwen3-8B denso | pp512 | 357,9 ± 14,6 | 350,5 ± 30,0 |
| | tg128 | 14,72 ± 0,18 | 14,84 ± 0,30 |
| | pp4096+tg128 | 186,7 ± 3,3 | 188,4 ± 2,5 |
| 35B-A3B MoE, ggml-org per prima | pp512 | 394,3 ± 17,4 | 416,5 ± 2,7 |
| | tg128 | 23,68 ± 0,55 | 24,77 ± 0,39 |
| | pp4096+tg128 | 263,6 ± 4,3 | 264,6 ± 2,7 |
| 35B-A3B MoE, moro0 per prima (T-04b) | pp512 | 399,8 ± 13,1 | 393,9 ± 24,8 |
| | tg128 | 23,93 ± 0,08 | 23,71 ± 0,62 |
| | pp4096+tg128 | 259,3 ± 5,2 | 258,8 ± 2,1 |

**Verdetto: coincide.** Sul denso le due build non si separano. Sul MoE il primo giro dava a moro0 un
+4,6 % di decode, ma ggml-org girava per prima e i suoi campioni salivano giro dopo giro
(tg 23,5 → 24,0; pp4096 258,7 → 270,1): a ordine invertito il vantaggio passa dall'altra parte. Con
la media dei due ordini: decode 24,24 contro 23,81, pp512 405 contro 397, pp4096+tg128 261,7 contro
261,4. È rumore più ordine. La toolchain MSVC è equivalente per il lavoro sulla GPU.

**Lezione di metodo.** Il primo modello caricato dopo un periodo fermo parte più lento: un confronto
A-poi-B di `llama-bench` va ripetuto a ordine invertito, oppure fatto A-B-A. Per T-06 il banco usa
A-B-A.

**Da verificare, fuori da M-14.** Il denso 8B oggi fa 14,7–14,8 tok/s di decode con b10991, contro i
16,01 ± 0,03 di M-08 dopo il driver nuovo (con b10809). Stesse condizioni registrate (VGM 48, driver,
overlay). O b10991 è più lenta di b10809 sul denso, o qualcosa della macchina è cambiato col riavvio:
un `llama-bench` di b10809 sul denso lo direbbe in 3 minuti.

## T-05 — La prima patch

PR ggml-org/llama.cpp#27952 «vulkan: int8 coopmat1 matmul implementation for AMD RDNA3 and RDNA4»
(0cc4m), **aperta** al 17-09 (ultimo aggiornamento 16-09), 2 commit, 4 file, +1315/−29: uno shader
`mul_mmq_cm1` che fa il prodotto di matrici quantizzate in int8 con le matrici cooperative, per
Q4_0…Q8_0, IQ4_NL/XS, MXFP4, Q3_K…Q6_K, NVFP4.

- Si accende solo su AMD RDNA3/RDNA4. La 890M (gfx1150, RDNA 3.5) viene classificata RDNA3 perché
  il driver dichiara il prodotto scalare int8 accelerato (`int dot: 1`, `matrix cores: KHR_coopmat`
  nel log del backend): la patch la tocca davvero.
- Presa con `git fetch upstream pull/27952/head:patch/int8-coopmat` (9ff173ddb, base 434ddbbc0,
  precedente a b10991), ribasata su b10991 **senza conflitti** → 8253abef6.
- Build `b10991+moro1-vulkan`: `--version` = `build 10991, commit 8253abef6`.

## T-06 — Misura della patch

La variabile misurata è solo la patch. Base e patch hanno la stessa toolchain e le stesse opzioni
(moro0 contro moro1), quindi il confronto non è con la release di ggml-org. Le misure finali sono
tutte **dopo il riavvio** (Windows 26200.9457, driver 32.0.31041.1004, VGM 48, «Massime
prestazioni», dalle 07:53 alle 09:36). Quelle della notte sono in
`<radice>\m14\T-06a-prima-del-riavvio.jsonl`.

### llama-bench (come T-04, patch per prima, cioè nella posizione sfavorita; notte)

Righe in `<radice>\m14\T-06-bench-*.jsonl`. tok/s, media ± sd di 5 giri.

| modello | prova | moro0 | moro1 | Δ |
|---|---|---:|---:|---:|
| Qwen3-8B denso | pp512 | 380,5 ± 15,7 | 413,8 ± 2,7 | +8,7 % |
| | tg128 | 14,99 ± 0,02 | 15,18 ± 0,13 | +1,3 % |
| | pp4096+tg128 | 188,1 ± 2,0 | 196,2 ± 2,4 | +4,3 % |
| 35B-A3B MoE | pp512 | 406,8 ± 13,1 | **494,9 ± 2,0** | **+21,7 %** |
| | tg128 | 23,94 ± 0,25 | 24,29 ± 0,95 | invariato |
| | pp4096+tg128 | 264,3 ± 2,7 | 295,8 ± 4,9 | +11,9 % |

Il backend conferma `int dot: 1 · matrix cores: KHR_coopmat`: il percorso int8 è acceso.

### Banco di Aethera

`m08_bench` con `.lmbrain-lite/m14/T-06a.toml` e `T-06b.toml`:
- profili standard con la sola build cambiata variante per variante;
- prompt congelati 7k e 21k, 1 riscaldamento e 5 giri;
- temperatura 0, seme 1234, stesso nonce per tutte le varianti allo stesso giro;
- disegno A-B-A: moro0, moro1, moro0-bis.

Righe in `<radice>\m14\T-06a.jsonl` e `T-06b.jsonl`. Mediana (sd), tok/s.

**35B G1** (`qwen3.6-35b-a3b.q4_k_m.vulkan`: draft-mtp 3, ubatch 4096; prompt da 7.097 e 17.679
token). Avvii `r-20260917-075326`, `-080418`, `-081501`.

| variante | 7k prefill | 7k decode | 21k prefill | 21k decode | VRAM | RAM libera |
|---|---:|---:|---:|---:|---:|---:|
| moro0 | 447,3 (2,2) | 33,51 (2,55) | 352,6 (1,0) | 20,78 (0,36) | 23,19 GiB | 30,0 GiB |
| moro1 | **458,7 (1,0)** | 30,80 (2,38) | **364,6 (0,8)** | 20,96 (0,97) | 23,18 GiB | 30,5 GiB |
| moro0-bis | 446,4 (0,9) | 33,20 (2,53) | 352,7 (0,5) | 20,81 (0,42) | 23,19 GiB | 30,7 GiB |
| Δ patch sulla base media | **+2,6 %** | invariato (sd 2,5) | **+3,4 %** | invariato | = | |

**Coder-Next G3** (`qwen3-coder-next.q4_k_m.vulkan`: ubatch 512, senza speculazione; prompt da 6.754
e 13.604 token). Avvii `r-20260917-082545`, `-084330`, `-085829`.

| variante | 7k prefill | 7k decode | 21k prefill | 21k decode | VRAM | RAM libera | pronto in |
|---|---:|---:|---:|---:|---:|---:|---:|
| moro0 | 182,9 (0,5) | 19,95 (0,02) | 163,0 (0,2) | 18,40 (0,03) | 46,37 GiB | 31,5 GiB | 33,2 s |
| moro1 | **242,8 (0,2)** | 19,94 (0,02) | **207,7 (0,6)** | 18,39 (0,03) | 46,37 GiB | 31,5 GiB | 32,7 s |
| moro0-bis | 182,8 (0,5) | 19,96 (0,02) | 163,3 (0,5) | 18,36 (0,04) | 46,37 GiB | 31,6 GiB | 31,2 s |
| Δ patch sulla base media | **+32,8 %** | invariato | **+27,3 %** | invariato | = | | |

Sul G3 il guadagno è grande e pulito: scarti tipo sotto l'1 per mille, e la base si ripete identica
prima e dopo. Sul 35B è piccolo. Una possibile spiegazione, non misurata: con ubatch 4096 e MTP il
tempo del prefill va soprattutto altrove che nei prodotti di matrici quantizzati. Decode, memoria e
tempo di caricamento non cambiano in nessuno dei due.

**Prima e dopo il riavvio (35B).** Il 7k è uguale (moro0 443,1 → 447,3; moro1 458,9 → 458,7). La
base sul 21k è scesa di circa il 2 % (360,3 → 352,6, con moro0-bis a 352,7), la patch no
(363,5 → 364,6). Coerenza identica, giro per giro. I numeri di prima e di dopo si confrontano, ma il
verdetto usa solo quelli di dopo, dove A-B-A è intero.

### Coerenza dell'output

| confronto | 35B 7k | 35B 21k | G3 7k | G3 21k |
|---|---|---|---|---|
| moro0-bis contro moro0 | **5/5 identici** | **5/5 identici** | **5/5 identici** | **5/5 identici** |
| moro1 contro moro0 | 0/5 (diverge dal carattere 8 o 335) | 0/5 (78 o 290) | 0/5 (150–167) | 0/5 (19–531) |

La base è **deterministica**, anche attraverso il riavvio: la notte e la mattina danno gli stessi
testi e gli stessi punti di divergenza. Il testo quindi lo cambia la patch, come ci si aspetta da un
prodotto in int8 (attivazioni in q8_1). I testi della patch restano plausibili (stesso codice di
partenza, stessa prima riga del riassunto), ma sono altri testi.

**Perplessità** (`run-T06-perplexity.sh`: prompt congelato da 21k, contesto 8192, 2 blocchi, cache
f16; righe in `<radice>\m14\T-06-perplexity.txt` e `T-06b-perplexity.txt`):

| modello (ubatch) | moro0 | moro1 | moro0-bis |
|---|---:|---:|---:|
| 35B-A3B (4096) | 2,2601 ± 0,0456 | 2,2557 ± 0,0454 | 2,2601 ± 0,0456 |
| Coder-Next (512) | 2,7182 ± 0,0667 | 2,7133 ± 0,0664 | — |

La patch non peggiora la qualità misurabile: la perplessità è un filo più bassa in entrambi, dentro
l'errore. Il campione è piccolo (2 blocchi da 8.192 token di codice di Aethera).

### Verdetto

**Per la regola scritta: non entra in `moro-ai`.** Il milestone dice che una patch che cambia le
risposte a temperatura 0 non entra anche se è più veloce, e questa le cambia tutte, contro una base
che si ripete identica.

**I numeri però mettono in discussione la regola, e la decisione spetta all'operatore:**
- sul Coder-Next G3 il prefill guadagna il 33 % a 7k e il 27 % a 21k, con rumore sotto l'1 per mille;
- la perplessità non peggiora, né sul G3 né sul G1;
- decode, memoria e caricamento non cambiano;
- sul G1 il guadagno è piccolo (+2,6 %, +3,4 %) e da solo non giustificherebbe un ramo da ribasare.

Se la coerenza si giudica con la perplessità (criterio che il brief stesso indica come più solido)
invece che con l'uguaglianza del testo, la patch entra per il G3: prima in un profilo di prova su
`b10991+moro1`, poi, dopo una prova d'uso, nel profilo standard. Fino a quella decisione:
- il ramo `patch/int8-coopmat` resta nel repo del fork;
- `moro-ai` resta b10991 liscio;
- la build `b10991+moro1-vulkan` resta installata per le prove;
- nessun profilo standard la usa.

Se ggml-org fonde la PR, la patch arriva con un tag e la misura giusta diventa «tag nuovo contro
tag vecchio», come M-08 T-07.

### Il riavvio e Smart App Control

- **02:33, riavvio.** Windows Update ha riavviato (KB5129195, Windows 10.0.26200.9457) mentre
  moro0-bis del 35B faceva il riscaldamento del 21k. L'avvio `r-20260917-022758` è rimasto senza
  sezione di uscita.
- **Dopo il riavvio, i blocchi.** Smart App Control, in modalità attiva, ha bloccato dalle 07:45 i
  `llama-server` senza firma, compresa la release ggml-org b10991 (`llama-server-impl.dll`, uscita
  0xC0E90002), e `cargo.exe`. Registro CodeIntegrity, eventi 3033/3077/3118.
- **07:52, sblocco.** L'operatore ha spento Smart App Control, e da lì tutto è ripartito.
- **Condizioni invariate.** Driver GPU e NPU, VGM e alimentazione sono gli stessi di prima.


## T-07 — Aethera riconosce le build del fork (parte Rust)

- `machine::parse_build_tag` legge `b10991` e `b10991+moro1`; `machine::build_id_matches` è l'unico
  criterio di corrispondenza (usato da risoluzione, diagnosi di `launch` e proposte). Il confronto è
  esatto: **un profilo su `b10991` non prende mai una build del fork**, anche se in `machine.toml`
  viene prima; una build del fork va chiesta per nome (`build = "b10991+moro1"`).
- `provenance.rs`: legge `provenienza.toml`; il manifest lo copia in `[engine.provenance]` (e il
  motivo in `provenance_error` se il file c'è ma non si legge).
- `engine::check_build` all'avvio: il tag si controlla con `--version`, la serie con la provenienza.
  Una build del fork non passa per una di ggml-org, né il contrario. **Trovato dal primo giro di
  T-06**, che rifiutava `b10991+moro0` perché `--version` dice solo `b10991`: i test unitari non lo
  coprivano, ora sì.
- La serie è una **condizione** (`Conditions.build_series`: `ggml-org`, `moro0`,
  `moro1 patch/int8-coopmat@8253abef6`): la mediana di riferimento non mescola build lisce e
  patchate, il separatore della tabella Benchmark scatta («serie di patch ggml-org → moro1 …»), il
  confronto ha la riga «Serie di patch» e la mette fra i motivi di non confrontabilità. La riga corta
  delle condizioni aggiunge `· moro1` (non per ggml-org). Gli avvii di prima di M-14 si leggono come
  `ggml-org`: allora non esistevano build compilate qui.
- La colonna Build di Benchmark mostra `b10991+moro1` (dalla provenienza) invece del solo tag.
- Prove nuove: etichette e corrispondenze in `machine.rs`, lettura della provenienza, serie come
  condizione in `conditions.rs`, `check_build` in `engine.rs`; nei test d'integrazione il manifest
  con provenienza fa il giro TOML, e `runs::list` / `runs::compare` su tre avvii finti (vecchio,
  liscio, patchato) danno build, separatore e riga di confronto giusti. `cargo test`: 114 + 5 verdi;
  `cargo clippy`: nessun avviso nuovo (restano i 10 di prima, in file non toccati).
- Verificato su un avvio vero: il manifest di T-06 porta `[engine.provenance]` e
  `build_series = "moro0"`.
- Il banco `m08_bench` ha due opzioni nuove per gli scenari, spente per difetto: `fixed_nonce`
  (stesso nonce per tutte le varianti allo stesso giro) e `save_text` (testo intero nella riga).

**Resta (dopo il merge di M-12):** nella pagina Benchmark mostrare la serie accanto alla build e
l'avviso nel confronto; nella scheda Build delle Impostazioni mostrare la provenienza. I dati ci sono
già (`RunRow.build`, `conditions.build_series`, `conditions_changed`, `Comparison.conditions`).

## T-08 — Regola di adozione

Scritta in `.lmbrain-lite/fork/README.md` (le tre regole, la regola di adozione, l'elenco delle
candidate con PR e stato) e copiata nel repo del fork. Nel kit va riportata nelle note del milestone
dalla sessione principale (dal worktree i file del kit non si committano).

## Correzioni da riportare nello studio (pagina Motore, sezione build)

- **Tempo di build:** 2–2,5 minuti su questa macchina (110–113 s di compilazione), non 10–15.
- **Toolchain:** bastano Build Tools 2022 (MSVC, CMake e Ninja inclusi) e il Vulkan SDK
  1.4.357.0; il compilatore della parte CPU (MSVC qui, clang nella release) non sposta i numeri con
  tutti i layer sulla GPU.
- **Rumore di llama-bench:** chi gira per primo parte più lento, e un confronto a due va ripetuto a
  ordine invertito (T-04).
- **llama-bench non basta per giudicare una patch del prefill:** +22 % in pp512 sul 35B diventa
  +2,6 % sul profilo G1 a 7k (ubatch 4096), mentre sul Coder-Next con ubatch 512 si arriva a +33 %.
  Il guadagno dipende dal profilo, e si decide sul banco di Aethera.
- **Determinismo:** con b10991 compilata qui, due avvii della stessa build danno lo stesso testo a
  temperatura 0, sul G1 e sul G3, anche dopo un riavvio (in M-08 T-06, con b10809 e cache q8_0, no).
  Anche la perplessità si ripete fino all'ultima cifra.
- **PR #27952:** sulla 890M si accende (RDNA3 per ggml). Cambia le risposte ma non la perplessità,
  e accelera molto il prefill del Coder-Next. Non tocca il decode (`mul_mat_id`), dove resta il
  margine del MoE.
- **Smart App Control:** sulle build aggiornate di Windows 11 può bloccare `llama-server` senza
  firma, **anche quelle di ggml-org**. Va detto nella pagina Motore, perché Aethera allora non
  avvia nulla.
- **Decode del denso 8B, b10809 contro b10991** (llama-bench, nei due ordini, dopo il riavvio):
  15,48 / 15,49 contro 15,52 / 15,52 tok/s, quindi uguale. Il prefill 512 va da 302 a 387 (+28 %) e
  pp4096+tg128 da 171 a 196 (+15 %). I 16,01 di M-08 non li raggiunge nessuna delle due build: lo
  scarto (circa 3 %) viene dalla macchina, non da b10991. Stanotte, prima del riavvio, erano
  14,7–15,2. Righe in `<radice>\m14\T-10-denso-*.jsonl`.

## Cosa resta all'operatore

1. **Decidere sulla patch int8 coopmat:**
   - la regola scritta («stessa risposta a temperatura 0») la esclude;
   - la perplessità invariata e il +27–33 % di prefill sul Coder-Next G3 dicono che conviene.

   Se la regola diventa «perplessità invariata», il passo dopo è un profilo di prova del G3 su
   `b10991+moro1` (la build è già installata), non il profilo standard.
2. **T-07, parte della finestra (dopo il merge di M-12):**
   - Benchmark: la serie accanto alla build e l'avviso nella fascia di confronto;
   - Impostazioni → Build: la provenienza.
3. **Revisione e merge** del branch `m14-fork-leggero`. Niente push.
4. **Note del milestone:** riportarci la regola di adozione (T-08) dalla sessione principale.
5. **Smart App Control resta spento:** ricordarlo, perché Windows non lo riaccende più.
6. **File sulla macchina:**
   - build di prova `b10991+moro0-vulkan` e `b10991+moro1-vulkan` in `<radice>\builds`, con le loro
     voci in `machine.toml` (le copie di sicurezza sono accanto);
   - il repo del fork in `<radice>\src\llama.cpp`, con 1,3 GB di cartelle di build in `build-moro\`,
     cancellabili;
   - l'avvio `r-20260917-022758`, interrotto dal riavvio, non ha la sezione di uscita.
