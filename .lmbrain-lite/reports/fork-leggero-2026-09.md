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
- **Verdetto della patch: non entra.** Sul profilo G1 vero il guadagno è piccolo, e la patch cambia
  le risposte a temperatura 0 contro una base che invece si ripete identica (sezione T-06).
- **Misura incompleta:** alle 02:33 Windows Update ha riavviato la macchina (KB5129195, Windows
  26200.9457). Dopo il riavvio Smart App Control blocca **tutti** i `llama-server`, anche quello di
  ggml-org. Mancano il Coder-Next e la perplessità (vedi «Cosa resta all'operatore»).
- Aethera riconosce le build del fork (T-07, parte Rust); la parte della finestra aspetta M-12.

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

Due strumenti, una variabile: la patch. Base e patch sono compilate con la stessa toolchain e le
stesse opzioni (moro0 contro moro1), quindi il confronto non è con la release di ggml-org.

### llama-bench (come T-04, patch per prima: la posizione sfavorita)

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

### Banco di Aethera sul 35B G1 (T-06a)

`m08_bench` con `.lmbrain-lite/m14/T-06a.toml`: profilo standard `qwen3.6-35b-a3b.q4_k_m.vulkan`
(draft-mtp 3, ubatch e batch 4096, ctx 32768) con la sola build cambiata variante per variante;
prompt congelati 7k (7.097 token) e 21k (17.679 token, di cui 3.001 riusati dal prefisso comune);
1 riscaldamento e 5 giri; temperatura 0, seme 1234, stesso nonce per tutte le varianti allo stesso
giro. Disegno A-B-A. Avvii `r-20260917-020612` (moro0), `r-20260917-021708` (moro1),
`r-20260917-022758` (moro0-bis); righe in `<radice>\m14\T-06a.jsonl`. Mediana (sd).

| variante | 7k prefill | 7k decode | 21k prefill | 21k decode | VRAM dopo il caricamento | RAM libera |
|---|---:|---:|---:|---:|---:|---:|
| moro0 | 443,1 (3,1) | 32,54 (2,79) | 360,3 (4,6) | 20,80 (0,33) | 23,19 GiB | 31,6 GiB |
| moro1 | **458,9 (3,7)** +3,6 % | 31,00 (2,12) | 363,5 (0,6) +0,9 % | 21,14 (0,95) | 23,18 GiB | 32,7 GiB |
| moro0-bis | 448,0 (1,1) | 33,33 (2,53) | interrotto dal riavvio | | | |

Sul server il guadagno è molto più piccolo che in llama-bench: +3,6 % a 7k, con moro0-bis a metà
strada (448), e +0,9 % a 21k, dentro l'oscillazione di moro0. Il decode non si muove, come atteso da
una patch del prefill, e nemmeno la memoria. Perché così poco: il profilo usa ubatch 4096 e sul
prompt lungo il tempo va soprattutto nell'attenzione, non nei prodotti di matrici che la patch
cambia. Questo resta un'ipotesi: non l'ho misurato.

### Coerenza dell'output

| confronto | 7k: giri identici | 21k: giri identici |
|---|---:|---:|
| moro0-bis contro moro0 (base contro base) | **5 su 5**, testo uguale byte per byte | riscaldamento identico (l'unico giro fatto) |
| moro1 contro moro0 (patch contro base) | **0 su 5**: diverge dal carattere 8 (3 giri) o 335 (2 giri) | **0 su 5**: diverge dal carattere 78 (4 giri) o 290 (1 giro) |

A differenza di M-08 T-06, qui la base è **deterministica**: stessa build, stesso prompt, stesso
testo. La divergenza quindi la introduce la patch, ed è quello che ci si aspetta da un prodotto in
int8 (attivazioni quantizzate in q8_1 invece che in virgola mobile). I testi della patch restano
plausibili (stesso codice di partenza, stessa prima riga del riassunto), ma sono **altri** testi. La
perplessità direbbe se la qualità cambia (`run-T06-perplexity.sh`, pronto), ma il blocco di Smart
App Control l'ha impedita.

### Verdetto: non entra

- La regola 3 e il milestone lo dicono chiaramente: una patch che cambia le risposte a temperatura 0 non entra
  anche se è più veloce, e qui cambia tutte le risposte contro una base stabile.
- Il guadagno sul carico vero (G1) è del 3,6 % a 7k e quasi nullo a 21k: non varrebbe comunque il
  costo di tenere un ramo da ribasare.
- Il ramo `patch/int8-coopmat` resta nel repo del fork, non in `moro-ai`. Da rimisurare quando la
  PR sarà fusa o aggiornata: se ggml-org la accetta, arriva comunque con un tag, e la misura giusta
  diventa «tag nuovo contro tag vecchio», come M-08 T-07.
- La build `b10991+moro1-vulkan` resta installata come build di prova; nessun profilo standard la usa.

**Non misurato:** il Coder-Next G3 (T-06b, scenario pronto in `.lmbrain-lite/m14/T-06b.toml`) e la
perplessità. Non cambierebbero il verdetto (la divergenza basta), ma il task li chiede.

### Il riavvio a metà misura

Windows Update ha riavviato alle 02:33 (KB5129195, Windows 10.0.26200.9457). **Tutte le righe di
T-06a e di llama-bench sono di prima del riavvio**: l'ultima è delle 02:32:10, e la build di Windows
non è cambiata durante la misura. moro0-bis si è fermato dopo il riscaldamento del 21k; l'avvio
`r-20260917-022758` non ha la sezione di uscita, quindi Benchmark lo mostra come non chiuso. Driver
GPU e NPU, VGM e alimentazione sono invariati (verifica dell'agente principale alle 07:45). Dopo il
riavvio non c'è nessun numero, quindi il problema del confronto prima/dopo non si pone. Una misura
futura però andrà fatta tutta sulla build nuova, rifacendo anche la base.

**Dopo il riavvio Smart App Control blocca i motori.** Dalle 07:45:27 il registro CodeIntegrity
(eventi 3033/3077, criterio `{0283ac0f-…}`, «Smart App Control Block») rifiuta:

- `llama-server.exe` di b10991+moro0;
- `llama-server-impl.dll` di b10991+moro1 **e della release ggml-org b10991**, che ieri girava
  (uscita 0xC0E90002);
- `cargo.exe` della toolchain rustup.

`VerifiedAndReputablePolicyState = 1`: la modalità è «attivo». Nessun evento prima delle 07:45.
Non so dire se l'abbia attivato l'aggiornamento o se sia cambiata la reputazione cloud di questi file.
Smart App Control non l'ho toccato: è un'impostazione di sicurezza.

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
- **llama-bench non basta per giudicare una patch del prefill:** +22 % in pp512 sul 35B diventa +3,6 %
  sul profilo G1 a 7k e +0,9 % a 21k. Si decide sul banco di Aethera.
- **Determinismo:** con b10991 compilata qui e il profilo G1, due avvii della stessa build danno lo
  stesso testo a temperatura 0 (in M-08 T-06, con b10809 e cache q8_0, no). La coerenza si può
  quindi confrontare sul testo, oltre che con la perplessità.
- **PR #27952:** sulla 890M si accende (RDNA3 per ggml), ma cambia le risposte; non è la leva per
  il margine del MoE. Quel margine resta nel decode (`mul_mat_id`), e questa patch non lo tocca.
- **Smart App Control:** sulle build aggiornate di Windows 11 può bloccare `llama-server` senza
  firma, **anche quelle di ggml-org**. Va detto nella pagina Motore, perché Aethera allora non
  avvia nulla.
- **Da verificare:** il denso 8B fa oggi 14,7–15,2 tok/s di decode con b10991, contro i 16,01 di
  M-08 con b10809 e lo stesso driver.

## Cosa resta all'operatore

1. **Smart App Control** (Sicurezza di Windows → Controllo app e browser). Oggi blocca tutti i
   `llama-server`, compreso quello di ggml-org, e `cargo`: finché resta così, Aethera non avvia
   il motore. Spegnerlo si fa da lì, ma **non si può riaccendere senza reinstallare Windows**:
   la decisione è tua. Prima puoi guardare il registro «CodeIntegrity/Operational» (eventi 3077,
   3118) per vedere cosa blocca.
2. **Quando i motori ripartono:**
   - completare T-06, con la macchina ferma e nessun client:
     - `m08_bench <radice> .lmbrain-lite/m14/T-06a.toml` (tutto da capo, sulla build nuova di Windows);
     - `m08_bench <radice> .lmbrain-lite/m14/T-06b.toml` (Coder-Next);
     - `bash .lmbrain-lite/m14/run-T06-perplexity.sh moro0 <radice>/builds/llama-b10991+moro0-win-vulkan-x64 moro1 <radice>/builds/llama-b10991+moro1-win-vulkan-x64`;
     - poi `python .lmbrain-lite/m14/analisi.py banco <radice>/m14/T-06a.jsonl <radice>/m14/T-06b.jsonl`.
     Circa un'ora e mezza in tutto.
   - `cargo test` e `cargo clippy` in `src-tauri`, verdi alle 02:05 dopo l'ultimo cambio al codice
     Rust; dopo non ho più toccato il Rust.
3. **T-07, parte della finestra (dopo il merge di M-12):**
   - Benchmark: la serie accanto alla build e l'avviso nella fascia di confronto;
   - Impostazioni → Build: la provenienza.
4. **Revisione e merge** del branch `m14-fork-leggero`. Niente push.
5. **Note del milestone:** riportarci la regola di adozione (T-08) dalla sessione principale.
6. **Facoltativo:** `llama-bench` di b10809 sul denso 8B, per capire il decode sceso a 14,7–15,2
   (M-08: 16,01).
7. **File sulla macchina:**
   - build di prova `b10991+moro0-vulkan` e `b10991+moro1-vulkan` in `<radice>\builds`, con le loro
     voci in `machine.toml` (le copie di sicurezza sono accanto);
   - il repo del fork in `<radice>\src\llama.cpp`, con 1,3 GB di cartelle di build in
     `build-moro\`, cancellabili.
