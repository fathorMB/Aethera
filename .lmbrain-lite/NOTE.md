---
updated: 2026-09-16
by: lead
---
**Sessione chiusa il 16-09.** Hai aperto l'app e detto che va: **M-05 è chiuso**, otto task su otto. Tutto pubblicato su `main` fino a `42e9b1f`. L'app è rimasta aperta.

## M-08, dodici task su quattordici

Rapporto in [`reports/misure-motore-2026-09.md`](reports/misure-motore-2026-09.md), 524 righe: una tabella per misura con run id, il verdetto leva per leva, **undici correzioni da riportare nello studio** (con pagina e riga) e la lista di cosa serve ad Aethera a valle.

**Entrano:** build b10991 (prefill +6%), `-ub 2048` sul Coder-Next (+21%), `-ngl 999` per i 48-70 GB. **Confermate:** `-ub 4096` sul G1, `--load-mode auto`. **Scartate:** KV `q8_0`, `--n-cpu-moe`, i checkpoint, `--cache-reuse`, `mmap`.

**La cosa che pesa più di ogni leva non è nel motore:** un client che rimanda indietro la risposta appena ricevuta paga 6 s a turno, uno che la perde ne paga 16,4. Nonio fa la cosa giusta (100% di prefisso conservato). MTP vale il 36% sul solo decode; questo vale il 170% sul turno.

## Cosa resta, tutto in [`reports/da-provare-operatore.md`](reports/da-provare-operatore.md)

- **M-06 T-09** — l'ho lasciato aperto di proposito: quei cinque pezzi si vedono solo **provocando** il guasto, non usando l'app. Quindici minuti, e dire che erano provati sarebbe stato falso.
- **M-07 T-02** (una VM) e **T-07** (il tag).
- **M-08 T-08** — VGM a 64 GB: serve Adrenalin e un riavvio. A 48 GB il Coder-Next **ci sta già**.
- **M-08 T-10** — OpenCode non installato; Claude Code non parla con un endpoint OpenAI, quindi va tolto anche dallo studio.
- **M-08 T-11** — NPU non misurata. FastFlowLM è pronto in versione portatile, licenza letta: libera sotto i 10 M$ di fatturato, e quello lo sai solo tu.

**Una decisione visiva:** in tema scuro `--fg3` sta a 3,6:1, sotto soglia. `#838a94` lo mette a norma senza cambiare l'aria. Non l'ho toccato: è la palette che hai approvato in M-01.
