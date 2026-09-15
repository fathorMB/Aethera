---
updated: 2026-09-16
by: lead
---
**M-04: manca solo la tua prova (T-10).** Chiusi e provati dalla macchina: **T-01** lettore GGUF, **T-03** catalogo, **T-04** SHA-256 e oid LFS, **T-05** download riprendibile, **T-06** installazione vera di una build, **T-08** protezioni, **T-09** E2E (36 controlli verdi, 58 prove unitarie, frontend che compila).

**T-02 e T-07 li lascio aperti apposta**: il codice c'è e compila, ma la stima sulla pagina Avvio e la pagina Catalogo **non le ho mai viste in finestra**. Le dichiari fatte tu quando le guardi.

Da guardare in `npm.cmd run tauri dev`:
1. **Catalogo → Modelli**: due righe (Qwen3.6 Q4_K_M e il file MTP), stato «presente», arch `qwen35moe · MTP 1 · 262.144`, e sul Q4_K_M il marchio **hard link ×2**.
2. **Verifica** su un modello: barra di avanzamento, «Ferma» che funziona, e a fine calcolo lo stato che passa a «verificato».
3. **Rimuovi…** sul Q4_K_M: deve avvisare dei 2 collegamenti **prima** di cancellare.
4. **Con il motore acceso**: verifica e rimozione dei pesi in uso devono essere **rifiutate** (è il pezzo di T-08 non ancora esercitato).
5. **Catalogo → Build**: «Cerca le release», digest visibile, `--list-devices` su b10809.
6. **Avvio**: card «Stima memoria» che si aggiorna cambiando `ctx` e `cache_type_k/v`; con ubatch 4096 · vulkan mostra il buffer misurato su `r-20260916-001040`, altrimenti dice «totale minimo».

**Nota onesta sulla stima**: sullo stesso avvio da cui viene il buffer, stima e misura coincidono **per costruzione** — è un'identità, non una previsione.
