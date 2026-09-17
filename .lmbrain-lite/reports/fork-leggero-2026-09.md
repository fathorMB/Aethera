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
- **Verdetto della patch: VERDETTO_PATCH** (sezione T-06).
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

SEZIONE_T06

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

CORREZIONI_STUDIO

## Cosa resta all'operatore

RESTA_OPERATORE
