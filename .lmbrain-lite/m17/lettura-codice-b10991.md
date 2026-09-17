# M-17 T-03 — che cosa fa il server fra la richiesta e il primo token (lettura del codice)

> Sola lettura di `<radice>\src\llama.cpp`, HEAD `930e2fa59` = tag `b10991`, fatta da un sub-agent
> il 18-09-2026 notte. Nessuna misura qui dentro: ogni «deduzione» va confermata dal log a `-lv 5`.
> Il ramo `patch/int8-coopmat` cambia solo gli shader matmul di Vulkan: server e memoria sono quelli
> del tag. `S` = `tools/server/server-context.cpp`.

**In breve.** Con ~22 token nuovi e il prefisso tutto in cache, dentro `prompt_ms` il server fa
**due checkpoint dello stato ricorrente copiati su host e due decode separati (18 + 4 token) senza
riuso del grafo**. Il log a `-lv 3` non lo mostra perché i messaggi sui checkpoint sono a livello 4.

## 1. Confini di `prompt_ms`

- **Fuori**: template jinja e tokenizzazione (thread HTTP); scelta dello slot `get_available_slot`
  S:1547-1660 (similarità LCP S:1584); prompt cache su RAM `prompt_save`/`prompt_load` S:1642-1655,
  solo se lo slot è scelto per LRU o `f_keep < 0.5` (S:1606, 1632); `common_sampler_init` S:1783.
- **Inizio**: `slot.stats.update_prompt_start()` S:3143 (`server-common.h:372-396`).
- **Dentro, nell'ordine**: confronto del prefisso `get_common_prefix` S:3219; `--cache-reuse`
  S:3238-3285; ricerca e ripristino del checkpoint S:3349-3377; cancellazione dei checkpoint con
  `pos_max > pos_next` S:3390; `seq_rm(p0,-1)` S:3444; `create_checkpoint` S:3634 (prima del decode
  del batch corrente, commento S:3631); `llama_decode` S:3682; `common_speculative_process`
  S:3744-3748; `init_sampler` S:3603 (`common_sampler_accept` su tutti i token del prompt,
  S:409-431); campionamento del primo token.
- **Fine**: `set_prompt_last` in `metrics_post_decode` S:4109-4114, `update_prompt_last` S:3869.
- **Quando nasce un checkpoint** (S:3549-3578): il batch si spezza quando mancano `4 + n_ubatch`
  token alla fine e quando ne mancano 4, e all'inizio dei messaggi utente. Vicino alla fine del
  prompt (`near_prompt_end`, S:3586) il checkpoint si crea **sempre**, ignorando
  `--checkpoint-min-step` (S:3625-3628).

Deduzione, 22 token nuovi: checkpoint n. 1 (stato di fine turno precedente) → decode di 18 →
checkpoint n. 2 → decode di 4.

## 2. Ri-elaborazione sui modelli ibridi

- `seq_pos_min` degli ibridi è il massimo fra attenzione e stato ricorrente
  (`llama-memory-hybrid.cpp:172-175`), cioè l'ultima posizione; per Qwen3-Next `n_swa = 0` (S:1204).
- Se il prompt nuovo contiene tutto `slot.prompt.tokens` (generato compreso) non c'è ripristino.
- Se diverge dentro il generato, anche di un token: si cerca all'indietro l'ultimo checkpoint con
  `pos_max <= pos_next` (S:3351-3363); trovato → `n_past` = i suoi token (tipicamente N_prec−4);
  non trovato → «forcing full prompt re-processing», `n_past = 0` (S:3380-3383).
- Il rollback `n_rs_seq` dello stato ricorrente (`llama-memory-recurrent.cpp:193-203`) serve solo
  alla speculative: il server non lo usa per questo.
- I token ri-elaborati **entrano in `prompt_n`** (S:4088-4097); `cache_n` è `n_past` dopo il
  ripristino (S:3409).

## 3. Peso di un checkpoint e sincronizzazione

- Flag `LLAMA_STATE_SEQ_FLAGS_PARTIAL_ONLY` (S:2363-2364): sugli ibridi copia solo lo stato
  ricorrente (tensori r e s di ogni layer ricorrente, una cella), non la KV di attenzione.
- Mai `LLAMA_STATE_SEQ_FLAGS_ON_DEVICE`: la copia va **su host**, sincrona, **un tensore per
  volta** (`llama_io_write_host`, `ggml_backend_tensor_get` in ciclo, TODO sul batching,
  `llama-context.cpp:2585-2590`). Su Vulkan UMA ogni chiamata fa submit, `waitForFences`, pulizia
  dei command pool e `memcpy` (`ggml-vulkan.cpp:8949-8975`).
- `llama_state_seq_get_data_ext`/`set_data_ext` iniziano con `ctx->synchronize()`
  (`llama-context.cpp:4203-4211`); `data_tgt.resize()` alloca un buffer nuovo ogni volta
  (`common.cpp:2292`).
- **Con MTP**: il contesto draft di `QWEN35MOE` è una `llama_kv_cache` semplice il cui
  `state_write` ignora il flag (`llama-kv-cache.cpp:2059`): `update_dft` (S:2364) copia **l'intera
  KV del layer MTP**, che cresce col contesto.
- Stima: Coder-Next ~75-80 MiB e ~72 `tensor_get` per checkpoint; 35B-A3B ~63 MiB più ~2 KiB per
  token di KV MTP. La dimensione vera è nel campo `size = … MiB` della riga di log.

## 4. Costi fissi per batch

- Riuso del grafo solo se `can_reuse(gparams)` (`llama-context.cpp:1350`), altrimenti
  `sched_reset` + `build_graph` + `sched_alloc_graph` (:1362-1379). I due decode da 18 e 4 token
  non riusano il grafo della generazione.
- Pipeline Vulkan compilate una volta per processo (`ggml-vulkan.cpp:3349-3351`).
- Kernel MoE: `mul_mat_vec_id` fino a 8 token, poi `mul_mat_id` (:11092-11097,
  `mul_mat_vec_max_cols = 8`). Nessuna soglia a 32.
- Gated Delta Net fusa supportata da Vulkan (:19797-19808); riga «fused Gated Delta Net … enabled»
  all'avvio.
- `decode()` sincronizza solo se il batch ha output (S:3683-3685): il batch da 18 non ne ha, il suo
  tempo GPU viene atteso dal `synchronize` del checkpoint n. 2 (o da MTP).

## 5. Speculative / MTP

Dopo ogni decode del target `common_speculative_process` (S:3744) fa un `llama_decode(ctx_dft)` per
head sugli stessi token (`speculative.cpp:1519-1571`); a ogni checkpoint si aggiungono la copia
della KV MTP e `common_speculative_get_state` (S:2366). Tutto dentro `prompt_ms`.

## 6. Flag (`common/arg.cpp`)

| flag | default | nota |
|---|---|---|
| `--ctx-checkpoints` | 32 | 0 toglie i checkpoint **e** la spezzatura 18+4; una divergenza costa il ricalcolo completo |
| `--checkpoint-min-step` | 8192 | non vale per i due checkpoint di fine prompt |
| `--cache-ram` | 8192 MiB | fuori da `prompt_ms` |
| `--cache-idle-slots` | attivo | inerte con `--parallel 1` |
| `--cache-reuse` | 0 | |
| `--swa-full` | false | irrilevante, `n_swa = 0` |
| `--slot-prompt-similarity` | 0.10 | |
| `--kv-unified` | false | |
| `-ub` | — | fissa il punto di spezzatura «4 + n_ubatch» |

Variabili d'ambiente: `LLAMA_GRAPH_REUSE_DISABLE` (`llama-context.cpp:279`),
`GGML_VK_PERF_LOGGER` (tempi per operazione, `ggml-vulkan.cpp:7817`).

## 7. Righe di log per cronometrare (3 = info, 4 = trace, 5 = debug)

- `-lv 4`: **«new prompt, n_ctx_slot…» (S:3147) = inizio di `prompt_ms`**; «restored context
  checkpoint», «forcing full prompt re-processing», «erased invalidated context checkpoint»
  (S:3356-3393); «cached n_tokens = …, memory_seq_rm» (S:3442), una per batch del prompt;
  **«created context checkpoint N of M (… size = … MiB)» (S:2368)**; «init sampler, took … ms»
  (S:429); «prompt cache update took» (S:1655).
- `-lv 5`: «do_checkpoint = yes/no» (S:3629); «n_batch (effective)» (S:3648), inizio di ogni
  decode; «created speculative checkpoint» (S:3086).

## Candidati per gli 1,5 s (deduzione, da misurare)

1. Due decode piccoli (18 + 4) in regime MoE sparso, senza riuso del grafo.
2. Due checkpoint per richiesta: ~60-72 submit e fence sincroni e 63-80 MiB di `memcpy` ciascuno.
3. Solo G1: KV del draft MTP dentro ogni checkpoint più prefill del draft a ogni batch.
4. Ripristino nascosto con ri-elaborazione, `init_sampler` sull'intero prompt, `sched_reserve` a
   ogni richiesta: si escludono guardando il log.

Esperimento più economico: lo scenario di T-01 con `-lv 5`; i tempi fra «new prompt», «cached
n_tokens», «created context checkpoint» e «n_batch (effective)» separano i candidati in un colpo.
