# Modelli piccoli per i ruoli di GalaxyCenter: NPU, secondo motore, o profilo più leggero

**Studio, 20 settembre 2026.** Nessuna misura nuova: tutto quello che c'è qui viene da misure già
prese su questa macchina, dalla documentazione di FastFlowLM e dal suo tracker. Dove una cosa è
ipotizzata e non misurata lo dice la riga stessa, e la tabella del §7 le raccoglie tutte.

## La domanda

Aethera accende un motore solo, e in questo momento è sempre il profilo unico: Qwen3.6-35B-A3B Q8
su Vulkan, contesto 262144. La NPU XDNA2 della Minisforum non la usa nessuno. GalaxyCenter, nella
directory accanto, ha cinque ruoli (Ingestore, Collegatore, Spazzino, Autore digest, Ricercatore)
che non scrivono codice e che, sulla carta, un modello molto più piccolo potrebbe reggere.

Da qui le due domande dell'operatore:

1. Aethera può avviare anche modelli piccoli **sulla NPU**, per fare quei ruoli?
2. Se no, conviene provare **profili principali più leggeri** capaci di fare quel lavoro?

La risposta breve: **a tutte e due no, ma per motivi diversi, e la domanda giusta è una terza** —
e cioè che Aethera avvii, accanto al motore grosso, **uno o più motori di servizio piccoli sulla
stessa iGPU**. Il resto del documento è il perché.

---

## 1. Che lavoro è, davvero

Dai documenti di GalaxyCenter (`knowledge/agents.md`, `knowledge/agent-costs.md`,
`knowledge/llm-server-contract.md`, aggiornati il 18-19 settembre):

- **Non è un lavoro solo di chat.** Il motore di GalaxyCenter pretende **tre endpoint**:
  `chat` (con `--jinja`, tool calling, `response_format` a schema JSON), `embed`
  (Qwen3-Embedding-0.6B, 1024 dimensioni, fisse in configurazione) e `rerank`
  (Qwen3-Reranker-0.6B, `/v1/rerank`).
- **GalaxyCenter non li avvia.** È scritto due volte: «Avvio e gestione dei processi llama.cpp» è
  fuori scope della v1, e il contratto dice che il motore «si collega solo agli endpoint di un
  server custom gestito dall'operatore». Quel server custom **è Aethera**. E Aethera oggi ne
  accende uno.
- **La sessione di un ruolo è fatta di poche richieste corte.** Il kit di valutazione del
  Collegatore misura 4,9 chiamate per caso e 45,3 s con il 35B locale. Il motore consegna il
  lavoro a pezzi (`next_*`) e valida i salvataggi nel codice (`rules.py`): il modello vede una
  scheda e 3-4 candidate, non un repository. Contesto richiesto: qualche migliaio di token, non
  262144.
- **Quasi tutto è notturno.** Collegatore 02:00, Spazzino 03:00, Autore digest 07:00, tetti di
  15-30 minuti. Fanno eccezione l'**Ingestore**, che parte quando un progetto ha fonti nuove —
  cioè quando l'operatore committa, cioè di giorno — e il **Ricercatore**, che parte quando lo
  chiede l'orchestratore.
- **Un modello piccolo forse basta davvero.** Sul kit del Collegatore, dopo la correzione dei casi
  del 20-09, Qwen3.6-35B-A3B Q8 locale, Haiku e Sonnet fanno **7/7 tutti e tre**. GalaxyCenter
  stesso scrive le due avvertenze giuste: la batteria non discrimina più, e c'è varianza fra un
  giro e l'altro. Quindi *non* sappiamo che un 4B basti; sappiamo che il compito non ha ancora
  separato tre modelli molto diversi fra loro.

Riassunto del carico: **due servizi sempre accesi e minuscoli** (embed, rerank) più **una chat
agentica a raffiche corte, quasi tutta di notte**.

---

## 2. Strada A — la NPU con FastFlowLM

### Quello che abbiamo già misurato su questa macchina

M-08 T-11, 16 settembre 2026, driver NPU 32.00.20102.3930, iGPU 32.0.31041.1004, FastFlowLM 1.0.5:

| | prefill 7k / 21k | decode 7k / 21k |
|---|---|---|
| 35B su Vulkan, **NPU ferma** | 418,5 / 361,1 tok/s | 33,09 / 20,01 tok/s |
| 35B su Vulkan, **NPU che genera in continuo** | 253,6 / 226,6 (**−39% / −37%**) | 19,26 / 11,50 (**−42% / −43%**) |

Nello stesso momento il Qwen3.5-4B sulla NPU faceva 13,8 tok/s di decode e 306 di prefill (da solo:
16,0 e ~400, TTFT ~1 s). Scarti tipo sotto l'1,5%.

Da leggere bene: **la contesa non è solo di banda**. Se fosse banda cadrebbe il decode e non il
prefill; è caduto anche il prefill, del 39%. NPU e iGPU stanno sullo stesso SoC e si dividono la
stessa memoria **e** lo stesso tetto di potenza. Una NPU al lavoro toglie al motore grosso circa il
40% di tutto il turno.

E il 4B sulla NPU, **anche da solo**, è più lento del 35B-A3B sull'iGPU: 16 contro 24,6 tok/s di
decode. Non è un paradosso: il MoE legge 2,265 GB di pesi per token in Q4 (8 esperti su 256), un
denso da 4B li legge quasi tutti. Su una macchina limitata dalla banda vince chi legge meno.

Conclusione già scritta in LOG.md il 16-09 e che questo studio conferma: *«la NPU come secondo
motore costa il 40% al motore principale; ha senso solo per lavori occasionali, non in parallelo
continuo»*.

### E gli embedding, che sarebbero il pezzo più utile

Non si possono fare: bloccano la NPU. `reports/npu-blocco-2026-09-16.md` — 3 blocchi su 199
richieste, LiveKernelEvent 141, `FAILURE_BUCKET_ID: LKD_0x141_IMAGE_ipustack.sys`, e col driver
vecchio una schermata blu 0x139. Con la sola generazione: 0 blocchi su 362 richieste. Il difetto è
nel percorso degli embedding, non nella NPU in sé — ma è esattamente il pezzo che a GalaxyCenter
serve sempre acceso.

### Lo stato del software, letto oggi sul tracker di FastFlowLM

Versione corrente **v1.0.6**. Il tool calling c'è dalla 0.9.26 e nella 1.0.6 è arrivato anche
`tool_choice`; il modello consigliato per il tool calling è `qwen3-it:4b`. Fin qui bene. Il resto no:

- **[#733](https://github.com/ROCm/FastFlowLM/issues/733) — aperta.** La cache del prompt non
  riaggancia nemmeno un reinvio identico (`can_use_cache` pretende due messaggi nuovi), e una
  corrispondenza parziale butta tutto: 145 messaggi su 146 uguali, e ne ri-prefilla 146. Per di più
  durante il prefill non scrive un byte sul socket, quindi il client va in timeout e riprova, e il
  ciclo si autoalimenta.
- **[#737](https://github.com/ROCm/FastFlowLM/issues/737) — aperta.** Nessun riuso del prefisso
  condiviso fra richieste one-shot: ogni richiesta ri-prefilla tutto il prompt di sistema, «~7
  secondi di prefill NPU per ~35 token nuovi».
- **[#744](https://github.com/ROCm/FastFlowLM/issues/744) — aperta.** Checkpoint che non si
  ripristinano e «Max length reached» sulle chiamate a strumenti multi-turno, proprio con
  Qwen3.6-MoE.
- **[#608](https://github.com/ROCm/FastFlowLM/issues/608) — aperta.** `flm serve` va in crash
  sotto richieste concorrenti; confermata su **Windows 11 build 26200** (la nostra) e su v1.0.5 e
  v1.0.6.

Il riuso del prefisso è il numero che su questa macchina abbiamo deciso di sorvegliare più di ogni
altro: la baseline del prodotto lo mette a 90,3% e conta «le richieste che riusano zero». Sulla NPU
oggi quel numero sarebbe **zero riuso, sempre**. Una sessione di ruolo da 4,9 chiamate ri-prefilla
il prompt di sistema del ruolo cinque volte.

### Verdetto su A

**No, oggi.** Non per la licenza (accettata dall'operatore il 16-09) e nemmeno per l'idea, che
resta sensata: per quattro fatti indipendenti, tre misurati qui e uno letto sul tracker.

1. costa il 40% al motore principale quando lavora in parallelo (misurato, 16-09);
2. da sola è più lenta del motore principale (misurato, 16-09);
3. gli embedding bloccano il dispositivo (misurato, 16-09) — e sono il pezzo più utile;
4. non ha riuso del prefisso e va in crash sulla concorrenza (difetti aperti sul tracker, letti
   oggi 20-09).

Non è un no definitivo: è un no con una data e quattro cause, tre delle quali possono cambiare con
un rilascio di FastFlowLM. In coda c'è una riprova che costa poco, al §6.4.

---

## 3. Strada C — un profilo principale più leggero

La metto prima della B perché si chiude subito. L'idea è: se il lavoro dei ruoli è facile, teniamo
un modello più piccolo come motore unico.

Questa macchina ha già risposto, e la risposta è **no**, due volte.

**Sulla velocità.** M-08 T-02, misurato qui, dopo l'aggiornamento dei driver:

| modello | decode |
|---|---|
| Qwen3-8B Q4_K_M (**denso**) | 16,01 tok/s |
| Qwen3.6-35B-A3B Q4_K_M (**MoE**) | 24,30 tok/s |

Un denso da 8B è **più lento** del MoE da 35B, su questa macchina, perché il decode è banda e il
MoE legge meno byte per token. Un 4B denso starebbe attorno ai 30 tok/s: un guadagno modesto contro
un modello molto più stupido. Non c'è una velocità da comprare.

**Sull'intelligenza.** M-18 T-04 ha già misurato che il Q8 batte il Q4 dello stesso modello *pur
avendo il decode più lento* (33-36 compiti/ora contro 27-28), perché sbaglia meno e rifà meno
turni. È la stessa aritmetica, e va nella direzione opposta al rimpicciolire.

**E soprattutto il profilo è uno per decisione del 20-09.** Non ci sono «profili principali» al
plurale: cambiare il principale vuol dire cambiare il prodotto e la baseline. Quella decisione è
di ieri.

**Verdetto su C: no**, e non serve misurarlo — il numero che lo chiude è già in casa.

---

## 4. Strada B — un secondo motore, piccolo, sulla stessa iGPU

È la strada che le misure di sopra lasciano in piedi, ed è anche quella che chiude un buco vero
del prodotto: **GalaxyCenter oggi non può funzionare del tutto, perché ha bisogno di tre endpoint
e Aethera gliene serve uno.**

### C'è spazio in VGM?

Ultimo avvio vero, `runs/r-20260920-160335`, profilo unico a 262144, VGM 64:

```
vram_dedicated_gib = 45.89     vram_shared_gib = 0.64
ram_available_gib  = 17.43     margin_ok = true
```

**Misurato il 20-09 sera, a motore acceso e mentre serviva davvero una richiesta** (M-20 T-01,
prima metà):

| | |
|---|---|
| VRAM dedicata dichiarata dal driver (VGM) | **64,00 GiB** |
| in uso con il profilo unico a 262144, sotto richiesta | **46,76 GiB** |
| **dedicata libera** | **17,24 GiB** |
| RAM visibile a Windows / libera | 31,65 / 16,94 GiB |
| con accanto un compagno Qwen3-8B Q4_K_M a ctx 8192 | **52,52 GiB**, cioè 11,48 liberi |

La stima che questo studio portava prima — «~57,8 GiB assegnati, ~12 GiB liberi» — era **sbagliata
in meno**: lo spazio vero è 17,2 GiB, cioè metà in più. Era un conto ricavato dalla RAM che Windows
non vede, ed è il motivo per cui era marcato come calcolato e non misurato.

I 46,76 GiB stanno sopra i 45,89 del manifest subito dopo il caricamento: la differenza sono i
buffer di calcolo e la KV che cresce mentre la conversazione si riempie.

**Attenzione al metodo, perché quello scritto prima non funziona.** `llama-server --list-devices`
riporta `81740 MiB, 77653 MiB free` **identico al byte** con il motore spento e con il motore
carico che sta servendo una richiesta: su questo driver la cifra «free» è il budget dell'heap, non
la memoria davvero libera, e non vede quello che il motore ha allocato. Il numero vero viene dal
contatore di Windows `\GPU Adapter Memory(*)\Dedicated Usage` — lo stesso che Aethera già usa per
`vram_dedicated_gib` — e la capacità dalla chiave di registro `HardwareInformation.qwMemorySize`
della scheda.

Cosa ci starebbe dentro:

| servizio | pesi | contesto | VRAM stimata |
|---|---|---|---|
| Qwen3-Embedding-0.6B Q8 (`--embedding --pooling last`) | ~0,7 GB | 8k | ~1 GB |
| Qwen3-Reranker-0.6B Q8 (`--reranking`) | ~0,7 GB | 8k | ~1 GB |
| chat dei ruoli, 4B Q8 o 8B Q4, `--jinja` | 4-5 GB | 16-32k | ~6 GB |

Totale ~8 GiB sui **17,2 misurati**. Il 20-09 sera M-20 T-01 ha chiuso anche la seconda metà: un
compagno vero — Qwen3-8B Q4_K_M, molto più grande di quello che servirebbe — prende **5,76 GiB** e
ne lascia liberi 11,48. Un embedding e un reranker da 0,6B insieme staranno attorno ai 2 GiB.

I pesi dei due servizi sono stati scaricati e verificati lo stesso giorno: `Qwen3-Embedding-0.6B-Q8_0`
dal repository ufficiale Qwen, e il reranker solo dalla comunità, perché Qwen non ne pubblica uno —
il che è esattamente la situazione da cui nasce il test di sanità.

### Perché un secondo processo e non lo stesso motore

Si potrebbe mandare il lavoro dei ruoli al motore grosso, che è già acceso e già capace. Sarebbe
gratis in codice e **caro in produttività**, per un motivo che abbiamo misurato proprio qui: il
riuso del prefisso è **tutto-o-niente** (misura del 20-09, scritta nella baseline). Il profilo ha
`n_parallel = 1`: infilare la conversazione di un ruolo nello stesso slot fa divergere il prompt
del client che sta lavorando, e alla divergenza il motore riparte da zero. A 262144 un prefill a
freddo costa fino a ~28 minuti (M-18 T-06/T-09).

**Un motore separato protegge la cache del motore grosso.** Questa, e non la velocità, è la
ragione tecnica della strada B. Non è misurata come tale — è una conseguenza diretta di due misure
che abbiamo, e va verificata.

### E la contesa? Adesso è misurata, e la risposta è doppia

*Scritto prima come domanda aperta; **misurato la sera del 20-09** (M-20 T-02, 71,5 minuti, quattro
condizioni di fila nella stessa sessione col metodo di M-08). Compagno: Qwen3-8B Q4_K_M a ctx 8192,
scelto perché già caratterizzato su questa macchina e più grande di quello che servirebbe.*

| condizione | prefill 7k | decode 7k | prefill 21k | decode 21k |
|---|---|---|---|---|
| **A1** compagno spento | 385,0 ±4,6 | 18,55 ±1,35 | 322,4 ±1,7 | 13,40 ±0,52 |
| **B** compagno **carico e fermo** | 391,8 **+1,8%** | 20,20 +8,9% | 326,4 **+1,2%** | 12,75 −4,9% |
| **C** compagno che **genera** | 159,7 **−58,5%** | 9,97 −46,2% | 134,8 **−58,2%** | 6,56 −51,1% |
| **A2** sentinella, compagno rispento | 392,3 +1,9% | 20,78 +12,0% | 323,4 +0,3% | 13,59 +1,4% |

**Come va letta, prima delle conclusioni.** Lo scarto tipo del decode in A1 è 1,35 su 18,55, cioè
il 7%, e la sentinella A2 — due condizioni *identiche* — mostra +12,0% di decode. Quindi **la
colonna del decode non discrimina sotto il 15%**, e tutto quello che si conclude su B poggia sul
prefill, il cui scarto tipo è fra 1,7 e 4,6. Che A2 torni dov'era A1 dice che la sessione non è
derivata e che i confronti valgono.

**Due risposte, una attesa e una no.**

1. **La condizione che decide, la B, è gratis.** Tenere un secondo motore *carico e fermo* non
   costa niente al principale: +1,8% e +1,2% di prefill, cioè zero. Con la precedenza decisa
   dall'operatore è lì che la macchina passa quasi tutto il tempo, quindi i servizi possono restare
   residenti e il ripiego «accendi a richiesta, spegni per quiete» non serve.
2. **Due motori che lavorano insieme costano più della NPU**, e questo smentisce quello che questo
   stesso documento dava come possibile in entrambi i versi: −58% di prefill contro il −39% di
   M-08 T-11. Si serializzano sui comandi della GPU *oltre* a dividersi la banda. Il compagno
   intanto faceva 6,3 tok/s complessivi e 39,5 s mediani per richiesta: in C perdono tutti e due.

La conseguenza pratica è che **la precedenza al coding non è una cortesia, è una necessità**, e il
buco di `in_use` a 30 secondi descritto sopra va chiuso davvero: un ruolo che parte nella finestra
sbagliata non rallenta un po' il lavoro dell'operatore, lo dimezza.

### llama.cpp lo sa già fare da solo

La build fissata nel profilo, **b10809, ha già la modalità router** (verificato oggi con il binario
vero, `C:\Nonio\llama-b10809-vulkan\llama-server.exe --help`):

```
--models-dir PATH      directory containing models for the router server
--models-preset PATH   path to INI file containing model presets for the router server
--models-max N         maximum number of models to load simultaneously (default: 4, 0 = unlimited)
--models-autoload / --no-models-autoload
```

Un `llama-server` senza `-m` diventa un router: tiene la porta, e per ogni `model` che arriva nelle
richieste avvia un processo figlio, fino a `--models-max` caricati insieme.

**Non la consiglierei come base.** Il router nasconde ad Aethera proprio quello che Aethera esiste
per mostrare: la riga di comando per intero, il manifest dell'avvio, il log, la memoria misurata
dopo il caricamento, la telemetria per richiesta. Aethera sa già avviare processi in job object,
scegliere porte, misurare memoria e leggere `/health`: ripetere quella meccanica per un motore di
servizio è meno codice che reinventare l'osservabilità sopra un router. Il router resta però la via
buona per una cosa: **provare la contesa fra due modelli in un pomeriggio**, senza toccare Aethera.

### Verdetto su B

**Sì come direzione**, con una misura da fare prima di scrivere la finestra. È la sola strada che
(a) chiude un buco dichiarato di GalaxyCenter, (b) protegge la baseline del prodotto invece di
metterla a rischio, (c) usa una capacità che il binario fissato ha già.

---

## 5. La proposta

**Deciso dall'operatore il 20-09, a studio letto:** i ruoli devono poter girare **anche di giorno,
ma il coding ha la precedenza**. Non è una via di mezzo fra le altre due: cambia la fase 2 da
«sopportare la contesa» a «non farla accadere», e sposta la misura che conta (§6.1) dalla
condizione C alla condizione B.

Tre pezzi, in quest'ordine, ognuno utile da solo.

### Fase 1 — profili di servizio (embed e rerank)

Un tipo di profilo nuovo, `role = "service"` accanto al profilo principale, per un motore che
**non è il soggetto di una misura ma un servizio**: pesi piccoli, contesto piccolo, porta sua,
nessuna speculazione.

Perché è il primo pezzo:

- è quello che **serve davvero e che oggi manca**: senza embed e rerank GalaxyCenter non indicizza
  e non cerca, e non è una questione di modelli piccoli o grossi;
- non ha tool calling, non ha conversazione, non ha riuso del prefisso da proteggere: i difetti che
  affossano la NPU qui non esistono;
- il carico è a impulsi (15 s per ricalcolare i vettori di due repo, dice GalaxyCenter), quindi la
  contesa con il motore grosso è per costruzione piccola;
- in codice è poco: un `Service` accanto a `Engine` con processo, porta e `/health`, senza
  manifest, telemetria e «in uso». `AppState` oggi tiene un `Engine` solo
  ([lib.rs:43](src-tauri/src/lib.rs:43)), e [launch.rs:38](src-tauri/src/launch.rs:38) rifiuta già
  tutto ciò che non è `llama.cpp`: i punti di innesto ci sono.

Da decidere in questa fase e non dopo: i motori di servizio **non entrano** nello stato «in uso»
del motore principale e non bloccano arresto e riavvio, altrimenti un embedding notturno impedisce
all'operatore di riavviare il motore.

### Fase 2 — il motore di chat dei ruoli, con la precedenza al coding

Stesso meccanismo, modello più grande (4-8B, `--jinja`), porta sua, **acceso e residente** accanto
al principale.

La precedenza **non va costruita**: il meccanismo c'è già su tutti e due i lati, e questa è la cosa
più importante che questo studio ha trovato oltre alle misure.

- **Aethera sa già dire quando il motore grosso è in uso**, e lo espone: `GET /status` su
  `127.0.0.1:8090` porta `in_use` e **perché** — slot attivo, richiesta negli ultimi 30 s, lock
  dichiarato da un client, protezione manuale ([engine.rs](src-tauri/src/engine.rs), `Usage`).
  È l'informazione esatta che serve, e non serve inventarla.
- **GalaxyCenter sa già mettere in attesa una sessione e riprenderla.** M-14 ha costruito lo stato
  `waiting` con il motivo, la ri-accodatura a 2, 10, 30 minuti e un'ora, il pulsante «Riprova
  adesso», e il cancello `available(role)` che vale per pianificatore, ingest automatico e avvii a
  mano. Oggi guarda quota e tetti di spesa; guardare anche `in_use` di Aethera è **una condizione
  in più nello stesso cancello**, non un pezzo nuovo.

**Ma `in_use` da solo non basta, e questo l'ho misurato oggi.** Mentre l'operatore mi diceva che il
motore era occupato da un altro sviluppo, `GET /status` rispondeva:

```
"in_use": false,  "last_request_s": 1276,  "locks": [],  "protected": false
```

Cioè: **libero**, con una sessione di coding aperta e ferma da 21 minuti. Non è un difetto, è la
definizione: `IN_USE_WINDOW_S = 30`. Fra un turno e l'altro di una sessione agentica — mentre
l'operatore legge, o mentre il client gira i test — `in_use` dice sempre «libero», e una sessione
di ruolo che partisse in quella finestra entrerebbe in collisione.

Quello che manca non è un meccanismo nuovo: è il **lock**, che Aethera ha già (`POST /lock` con
TTL, `DELETE /lock`) e che in quel momento non teneva nessuno. Il lock è l'unico modo in cui
Aethera sa che un client *sta lavorando* anche quando non sta mandando richieste in questo secondo,
ed è anche l'unico modo in cui sa **quale** client è. Quindi la precedenza si scrive così:

- **il client di coding prende un lock** per la durata della sessione e lo rinnova;
- **GalaxyCenter guarda `in_use`**, che comprende già i lock fra le sue `reasons`.

La prima metà è una modifica a Nonio, non ad Aethera, e la decide l'operatore. Finché non c'è, la
precedenza funziona a maglie larghe: protegge dalle collisioni durante una richiesta, non fra due
richieste.

Quindi la regola: prima di aprire una sessione di ruolo, `available(role)` chiede ad Aethera se il
motore grosso è in uso; se lo è, la sessione resta `waiting` e ritenta. Una sessione già aperta che
vede il motore tornare in uso finisce il pezzo che ha in mano e poi si ferma come `interrupted` —
il lavoro è già spezzato in pezzi da `next_*`/`save_*`, quindi fermarsi fra due pezzi non perde
niente.

**Chi decide resta GalaxyCenter, non Aethera.** È la stessa linea del lock: *«Il lock non cambia il
motore: rende solo rifiutati arresto e riavvio»*. Aethera dice com'è messa; la politica è di chi
usa. Sospendere d'autorità il processo compagno sarebbe più semplice da scrivere e sbagliato: farebbe
scadere le richieste in volo e metterebbe in Aethera una decisione di un'altra app.

**Cosa resta da misurare, e cambia rispetto a prima.** Con la precedenza, il caso «due motori che
lavorano insieme» non dovrebbe quasi mai capitare: resta la finestra di 30 s con cui Aethera
dichiara «in uso», e i pezzi già iniziati. Quindi la condizione che decide non è più la C
(entrambi al lavoro) ma la **B**: quanto costa al motore grosso avere accanto un secondo
`llama-server` **caricato e fermo**, cioè che occupa VRAM e basta. Se la B è gratis, la fase 2 si
fa; se la B costa, si torna a spegnere e riaccendere il motore compagno, e allora sì che conviene
la notte.

### Fase 3 — niente

Nessun supporto NPU in Aethera finché i quattro fatti del §2 non cambiano. Se cambiano,
`runtime.kind` è già la leva: oggi vale `"llama.cpp"` e `launch.rs` rifiuta il resto con un
messaggio che dice «questa versione avvia solo llama.cpp». Un domani `"fastflowlm"` entra da lì,
con una `cmdline` sua, senza toccare il resto.

---

## 6. Le misure che mancano, in ordine di quanto pesano

**6.1 — Quanto costa un secondo motore sulla stessa iGPU.** *(la misura che decide)*
Motore grosso acceso col profilo unico, carichi congelati di T-05 (`prompt-7k`, `prompt-21k`),
cinque giri, esattamente il metodo di M-08. Tre condizioni: (A) secondo motore spento;
(**B**) secondo motore acceso e fermo — la sola VRAM occupata; (C) secondo motore che genera in
continuo. Con la precedenza decisa il 20-09 **la condizione che decide è la B**: è quella in cui la
macchina passerà quasi tutto il tempo. La C serve solo a mettere un prezzo alla finestra di 30 s con
cui Aethera dichiara «in uso», e ai pezzi già iniziati. Si fa senza toccare Aethera, avviando il
secondo `llama-server` a mano o con `--models-dir`. **Stima: 1,5-2 ore.** Va di giorno.

**6.2 — Lo spazio vero in VGM.** *(prima metà fatta il 20-09 sera, vedi §4: 17,24 GiB liberi.)*
Resta la seconda metà — la stessa lettura con un motore di servizio carico — che richiede il
motore libero, perché caricare pesi sottrae memoria e tempo di GPU a chi sta lavorando.
**Dieci minuti**, appena la macchina è libera.

**6.3 — Un 4B basta per i ruoli?** Il kit del Collegatore di GalaxyCenter (`engine/eval_cases/`,
7 casi) girato su un 4B e su un 8B, più giri per via della varianza che GalaxyCenter ha già
dichiarato. Se un 4B fa 7/7 come il 35B, la fase 2 ha un modello; se no, i ruoli restano sul 35B e
la fase 2 muore lì. **Stima: 2-3 ore**, e va fatta in GalaxyCenter, non qui.

**6.4 — La riprova della NPU, se e quando.** Costa poco e non la farei adesso: `--pmode balanced`
o `powersaver` invece di `performance` (mai provati, e il 40% è stato misurato in `performance`),
con `--q-len 1` per evitare [#608](https://github.com/ROCm/FastFlowLM/issues/608). Ha senso
riaprirla quando [#733](https://github.com/ROCm/FastFlowLM/issues/733) e
[#737](https://github.com/ROCm/FastFlowLM/issues/737) si chiudono: senza riuso del prefisso il
resto non conta. **Stima: 1 ora.**

---

## 7. Misurato contro ipotizzato

| affermazione | stato | dove |
|---|---|---|
| NPU al lavoro costa −39%/−42% al 35B (prefill/decode) | **misurato** 16-09, driver 32.00.20102.3930 | LOG.md M-08/T-11 |
| Qwen3.5-4B su NPU: 16,0 tok/s da solo, 13,8 in contesa | **misurato** 16-09 | LOG.md M-08/T-11 |
| Gli embedding su NPU bloccano il dispositivo (3 su 199) | **misurato** 16-09 | `npu-blocco-2026-09-16.md` |
| Qwen3-8B denso (16,0 tok/s) è più lento del 35B-A3B (24,3) | **misurato** 16-09, b10809 | M-08 T-02 |
| Il riuso del prefisso è tutto-o-niente | **misurato** 20-09 | baseline del prodotto |
| Motore grosso a 262144: 45,89 GiB dedicati, 17,4 GiB RAM liberi | **misurato** 20-09, run `r-20260920-160335` | manifest |
| FastFlowLM: niente riuso del prefisso, crash sulla concorrenza | **letto** 20-09 sul tracker, difetti aperti | #733, #737, #744, #608 |
| b10809 ha già la modalità router con `--models-max` | **verificato** 20-09 col binario vero | `--help` |
| VGM 64,00 GiB, 46,76 in uso a 262144, **17,24 liberi** | **misurato** 20-09 sera, PDH + registro | §4 |
| Un compagno 8B Q4 carico prende 5,76 GiB: 52,52 in uso, 11,48 liberi | **misurato** 20-09 sera | §4 |
| `--list-devices` dice «77653 MiB free» sia a motore spento sia carico | **misurato** 20-09 sera: la cifra e' il budget dell'heap, non la memoria libera | §4 |
| Embed + rerank + 4B stanno in ~8 GiB dei 17,2 liberi | **stimato** sui pesi dichiarati, coerente con i 5,76 misurati per un 8B | §4 |
| Un secondo motore **carico e fermo** non costa niente (+1,8% di prefill) | **misurato** 20-09 sera, M-20 T-02 | §4 |
| Due motori che **lavorano** insieme costano −58% di prefill, **più della NPU** | **misurato** 20-09 sera: l'ipotesi «costa meno» era sbagliata | §4 |
| La colonna del decode non discrimina sotto il 15% (sentinella +12% fra condizioni uguali) | **misurato** 20-09 sera | §4 |
| Un motore separato protegge la cache del principale | **dedotto** da due misure, mai verificato | §4 |
| Un 4B basta per i ruoli di GalaxyCenter | **non si sa** | chiude con 6.3 |
| `in_use` dice «libero» fra due turni: 21 minuti di quiete con la sessione aperta | **misurato** 20-09, `GET /status` | §5, fase 2 |
| Il lock chiude quel buco, ma oggi non lo prende nessun client | **verificato** 20-09 (`locks: []`); la modifica a Nonio è dell'operatore | §5, fase 2 |

## 8. Fonti esterne

- [FastFlowLM — OpenAI API](https://fastflowlm.com/docs/instructions/server/openapi/) ·
  [server mode](https://fastflowlm.com/docs/instructions/server/) ·
  [tool calling](https://fastflowlm.com/docs/instructions/server/tool_calling/)
- [v1.0.6](https://github.com/ROCm/FastFlowLM/releases/tag/v1.0.6) ·
  [#733](https://github.com/ROCm/FastFlowLM/issues/733) ·
  [#737](https://github.com/ROCm/FastFlowLM/issues/737) ·
  [#744](https://github.com/ROCm/FastFlowLM/issues/744) ·
  [#608](https://github.com/ROCm/FastFlowLM/issues/608) ·
  [#730](https://github.com/ROCm/FastFlowLM/issues/730)
- [Model management in llama.cpp](https://huggingface.co/blog/ggml-org/model-management-in-llamacpp)
