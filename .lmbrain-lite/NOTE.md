---
updated: 2026-09-20
by: lead
---
**20-09 notte: M-20 fatto per dieci task su undici, non committato.**

**Le due misure che aprivano il milestone sono chiuse, e una smentisce lo studio.**
- **T-01**: VGM 64,00 GiB. Principale a 262144 da solo 46,76 → **17,24 liberi**; con un compagno carico 52,52 → 11,48. `--list-devices` **non serve** a questo (dà lo stesso «free» a motore spento e carico): il numero vero è il contatore PDH, e per singolo servizio `\GPU Process Memory(pid_N_*)`.
- **T-02** (71,5 min, quattro condizioni, sentinella ok): **B, compagno carico e fermo, è gratis** (+1,8% di prefill). Ma **C, due motori che lavorano insieme, costa −58% di prefill: più della NPU** (−39%). L'ipotesi «sulla stessa GPU costerà meno» era sbagliata. Attenzione: il decode ha ±7-15% di rumore e **non discrimina** — le conclusioni poggiano sul prefill.

Conseguenza: i servizi possono restare residenti, e **la precedenza al coding non è una cortesia ma una necessità**.

**Implementato e provato sulla macchina**, non solo a unit test: embedding e reranker accesi dal codice di Aethera in ~1 s, 1,80 e 1,81 GiB, e **14 controlli su 14** del contratto di GalaxyCenter.

**Il test di sanità del reranker ha ripagato subito.** Il GGUF della comunità (SHA-256 giusto) dà **8,9e-16 al documento pertinente**: rotto, come GalaxyCenter avverte. Escluso un difetto nostro (`--pooling rank`: identico), ho convertito i pesi ufficiali Qwen — **0,9967**. Per questo il profilo del reranker **non è seminato**: quei pesi non si scaricano, si fanno.

**Da te, due cose:**
- **T-10** (basta un 4B per i ruoli?) è bloccato: serve far girare il kit di valutazione dentro **GalaxyCenter**, di cui non sono il lead. Dimmi se lo faccio io o il suo.
- Il **lock**: oggi nessun client lo prende, quindi la precedenza protegge dentro una richiesta ma non fra due. Farlo prendere a Nonio è una modifica a Nonio.

Niente committato e niente pubblicato. Motori spenti.
