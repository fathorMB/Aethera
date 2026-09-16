---
title: Che cosa cambia per Aethera, dalle misure di M-08
updated: 2026-09-16
---

# Che cosa cambia per Aethera, dalle misure di M-08

Sintesi per chi progetta le prossime versioni. I numeri e il modo in cui sono stati presi stanno in
[`reports/misure-motore-2026-09.md`](../reports/misure-motore-2026-09.md) e
[`reports/npu-blocco-2026-09-16.md`](../reports/npu-blocco-2026-09-16.md); qui c'è solo quello che ne
segue per l'app. Macchina: Minisforum, Radeon 890M con VGM 48 GB, NPU XDNA2, 96 GB LPDDR5X.

## 1. Il client pesa più del motore

Un client che rimanda indietro la risposta appena ricevuta paga ~6 s a turno; uno che la perde ne paga
16,4 (T-14). Nessuna leva del motore sposta tanto: build nuova +6%, MTP +36% sul solo decode, questo
+170% sul turno.

- **Nonio** e **OpenCode** conservano il 100% del prefisso fra un turno e l'altro (T-10). OpenCode lo
  rompe solo quando **compatta** il contesto, vicino al limite servito: una richiesta di riassunto e
  poi un contesto ricostruito da rielaborare (~34 s a 32k, più ~2 min di generazione del riassunto).
  Più contesto servito vuol dire compattazioni più rare ma più care.
- **Claude Code si può collegare**: llama-server b10809 risponde anche su `/v1/messages` (API
  Anthropic), quindi basta `ANTHROPIC_BASE_URL`. Non ancora misurato: la CLI non è installata.
- **Cosa serve in Aethera:** la quota di prefisso conservata da ogni client, sulla pagina Motore. Il
  ponte `m08/prefix_proxy.py` mostra che basta confrontare il prompt di una richiesta con quello della
  precedente (anche il campo `system` dell'API Anthropic). Sarebbe la diagnosi «questo client ti sta
  costando il triplo».
- Il motore riprende da «lunghezza del prompt meno un ubatch» (T-13): con un client che estende,
  un ubatch piccolo costa meno a turno; con uno che modifica il prompt a metà, `-ub 4096` resta il
  migliore. Il valore giusto dipende dal client, e la telemetria deve poterlo mostrare.

## 2. Profili standard

| leva | verdetto | dove |
|---|---|---|
| build b10991 | entra (prefill +6%) | T-07 |
| `-ub 2048` sul Coder-Next | entra (prefill +21%) al posto di `-ub 512` | T-08b |
| `-ngl 999` per modelli 48–70 GB | entra: il Coder-Next da 48,5 GB sta in 46,1 GiB dedicati | T-08 |
| `-ub 4096` sul G1 | confermato | T-13 |
| `--load-mode auto` | confermato; `mmap` fa la doppia copia e lascia 0,4 GiB a Windows | T-09 |
| KV `q8_0`, `--n-cpu-moe`, checkpoint, `--cache-reuse` | scartate | T-04, T-06, T-08 |
| MTP adattivo | solo se si tiene un profilo unico per codice e contesto lungo | T-05 |

## 3. Il driver conta quanto una leva, e il manifest non lo sa

Adrenalin ha aggiornato il driver iGPU da una versione non registrata a **32.0.31041.1004** il 16-09.
Sullo stesso profilo G1 e con gli stessi carichi, a NPU ferma:

| | prima | dopo |
|---|---|---|
| prefill 7k / 21k | 344 / 296 tok/s | 418 / 361 tok/s (+21% / +22%) |
| decode 7k / 21k | ~26 / ~15,5 tok/s | 33,1 / 20,0 tok/s (~+30%) |

**Provvisorio:** verifica in corso con `llama-bench` e con lo scenario di T-07; questa tabella si
aggiorna quando finisce.

**Cosa serve in Aethera:** il manifest di ogni avvio deve registrare la **versione dei driver GPU e
NPU** (e l'overlay di alimentazione, la VGM, il volume dei pesi). Senza, due avvii con numeri diversi
del 30% sembrano uguali, e la soglia «degradato» (decode sotto il 70% della mediana) confronta avvii
che non sono confrontabili. La mediana di riferimento va ricalcolata quando cambia un driver.

## 4. La NPU non è un secondo motore da usare in parallelo

- **Contesa (T-11):** con la NPU che genera in continuo (Qwen3.5-4B su FastFlowLM 1.0.5), il 35B sulla
  iGPU perde ~40% su **tutto** il turno: prefill 418 → 254, decode 33,1 → 19,3 tok/s sul carico da 7k.
  Non è solo banda di memoria, perché cala anche il prefill. Il 4B intanto fa 13,8 tok/s (16,0 da solo).
- **Stabilità:** gli **embedding** con FastFlowLM bloccano la NPU (~1 richiesta su 70–90, reset del
  dispositivo; col driver vecchio fino alla schermata blu `0x139`). La generazione regge (0 blocchi su
  362 richieste, anche con la NPU al 100%). Responsabile nei dump: `ipustack.sys`.
- **Cosa ne segue per Aethera:** se un giorno gestirà un secondo motore sulla NPU, deve
  1. avvisare che rallenta il motore principale di ~40% mentre lavora, e mostrarlo nella telemetria;
  2. non offrire gli embedding su NPU con FastFlowLM 1.0.5 e questi driver;
  3. sorvegliare gli eventi `LiveKernelEvent 141` (registro Application, Windows Error Reporting) e
     fermare il carico al primo nuovo evento.

## 5. Schema del profilo e telemetria

Dal rapporto M-08, ancora da fare:

- **profilo:** `server.n_cpu_moe`; `cache.cache_ram`, `cache.checkpoint_min_step`, `cache.kv_unified`;
  `server.fit` e `server.fit_target` — il vero buco: `--fit` regola solo gli argomenti non impostati,
  e lo schema impone sempre `--n-gpu-layers`; `server.tensor_overrides` e `server.lazy_mode`.
- **telemetria:** leggere i token riusati da `timings.cache_n` della risposta invece di interrogare
  `/slots`; la quota di prefisso per client (sezione 1).

## 6. Disco

I pesi non si leggono dal disco a ogni token su questa macchina: fra 27 MB/s e 4,4 GB/s contro i 49 GB/s
che la memoria dà al motore (T-03). Il disco conta solo per il tempo di caricamento.
