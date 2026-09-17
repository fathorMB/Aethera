# Batteria di coding agentico: quale modello lavora meglio su questa macchina

> **M-15, notte del 17-09-2026.** Stato: RISULTATI_STATO

RIASSUNTO

## 1. Perché una batteria scritta qui (T-01)

Il vincolo è il tempo. Su questa macchina un modello genera 8–22 token al secondo ed elabora il
prompt a 100–350. Un compito da 15 turni con 10k token di contesto costa già 5–10 minuti. Per
confrontare quattro modelli in una notte, ogni compito deve chiudersi in pochi minuti.

**Le batterie pubbliche, e perché non vanno bene così come sono:**

| batteria | dimensione | cosa misura | per noi |
|---|---|---|---|
| SWE-bench Lite / Verified | 300 / 500 issue di repository Python veri (Verified: il 39% sotto i 15 minuti umani) | compito risolto se passano i test nascosti | no: Docker, repository grandi, contesti da 20–60k; decine di minuti a compito a 100–350 tok/s |
| SWE-bench Verified mini, bash-only (mini-swe-agent) | 50 compiti; agente con la sola shell | come sopra, con tetti di passi e di costo | forse, come ancora esterna, qualche compito facile e di rado |
| Aider polyglot | 225 esercizi Exercism in 6 linguaggi | pass@1 e pass@2 (il secondo tentativo vede i test falliti), formato delle modifiche, secondi per caso | forma utile (test, due tentativi, formato), ma è l'harness di aider e gli esercizi sono pubblici, quindi probabilmente contaminati |
| Terminal-Bench 2.0 (Harbor) | 89 compiti in container, con verificatore e soluzione scritta a mano | verificatore binario | compiti troppo lunghi; il formato del compito (tetti, verificatore separato) è quello da copiare |
| Multi-SWE-bench, SWE-rebench, SWE-smith | da centinaia a migliaia di compiti, anche Rust e TypeScript | come SWE-bench, con attenzione alla contaminazione | troppo pesanti; SWE-smith insegna a iniettare bug in repository con test |
| EvalPlus, LiveCodeBench | funzioni singole, datate | pass@k senza agente | fuori tema: niente strumenti né più turni |

**Quello che la letteratura insegna e che la batteria applica:**
- **Contaminazione.** I modelli indovinano il file da correggere di SWE-bench leggendo solo la issue
  (76% contro 53% su repository esterni). Per questo i compiti sono scritti da zero e datati.
- **Test nascosti.** Test visibili deboli gonfiano i punteggi (UTBoost, SWE-ABS). Per questo pochi test
  visibili fanno da specifica e il giudizio lo danno test nascosti in più.
- **Varianza.** Da un giro all'altro la stessa batteria oscilla di 0,5–3 punti. τ-bench propone
  pass^k, cioè il compito riuscito in tutte le k prove. Con una notte sola si fa un giro per modello:
  i confronti vanno letti con questa cautela (sezione 5).
- **Compiti per ora.** Non esiste una metrica standard con questo nome. Aider registra i secondi per
  caso, Terminal-Bench un tetto per compito. Qui la metrica è: compiti riusciti diviso ore di
  macchina, con il caricamento del modello contato a parte.

**La forma scelta:**
- 15 compiti su 12 repository piccoli (5–20 file), scritti per la batteria: 5 Rust, 6 Python e
  4 TypeScript (Node 24 lo esegue senza build).
- Difficoltà: 5 facili, 7 medi, 3 difficili.
- Tipi mescolati: bug su un file, bug distribuiti su più file, piccola funzionalità, funzionalità
  su più file, refactoring con controllo strutturale, e un compito in cui la cosa giusta è non
  toccare una cartella.
- Tre compiti sono ripetuti in italiano sullo stesso fixture, per misurare l'effetto della lingua.
- Tetti per livello: facile 12 turni e 7 minuti, medio 20 e 15, difficile 30 e 20.
- I casi di conformance di Nonio (`nonio conformance`) misurano l'harness, non il modello, e non entrano.

Fonti: swebench.com e il paper di SWE-bench Verified (openai.com/index/introducing-swe-bench-verified);
github.com/SWE-agent/mini-swe-agent; aider.chat/2024/12/21/polyglot.html e il README del benchmark di
aider; tbench.ai e harborframework.com; arxiv 2506.12286 (contaminazione); arxiv 2506.09289 (UTBoost);
arxiv 2406.12045 (τ-bench, pass^k); ai21.com/blog/scaling-agentic-evaluation-swe-bench (varianza);
github.com/multi-swe-bench; arxiv 2505.20411 (SWE-rebench); arxiv 2504.21798 (SWE-smith);
evalplus.github.io; arxiv 2403.07974 (LiveCodeBench). Letti il 17-09-2026.

## 2. Com'è fatta la batteria (T-02, T-03)

Sta in `.lmbrain-lite/batteria/`. Il README spiega come si usa.

| id | linguaggio | livello | tipo | che cosa chiede |
|---|---|---|---|---|
| rs-durata-en / -it | Rust | facile | bug | due errori nel parser di durate (unità dei minuti, cifre senza unità) |
| rs-lru | Rust | media | funzionalità | `get` e `put` di una cache LRU, con i casi limite (capacità 0 e 1) |
| rs-calc | Rust | difficile | bug su più file | precedenza e associatività di `^` e del meno unario, decimali nel lexer |
| rs-report | Rust | media | refactoring | estrarre `format_row`; il formato deve comparire una volta sola (controllo con regex) |
| py-slug | Python | facile | bug | slug con accenti, separatori ripetuti, taglio a `max_len` |
| py-intervals | Python | media | funzionalità | unione, lunghezza e buchi di intervalli semiaperti |
| py-ledger | Python | difficile | bug su più file | conversione di valute in Decimal con arrotondamento half-up, valuta sconosciuta, trasferimento invertito; `rates.py` non si tocca |
| py-config-en / -it | Python | media | vincolo | booleani dalle variabili d'ambiente; `legacy/` ha lo stesso bug ma è congelata |
| py-csvreport | Python | media | funzionalità su più file | `--format json` nella CLI, con la resa accanto a quella testuale |
| ts-giorni | TypeScript | facile | bug | giorni lavorativi: sabato contato, `n` negativo ignorato |
| ts-eventi-en / -it | TypeScript | media | funzionalità | `once`, `off`, `listenerCount` con la semantica di «fotografia» durante `emit` |
| ts-carrello | TypeScript | difficile | bug su più file | sconti a gruppi, IVA sull'importo scontato, categoria sconosciuta |

**Verificatore.** Ogni compito ha test visibili che falliscono sul fixture. Il verificatore:
1. controlla che i file protetti (test, `Cargo.toml` o `package.json`, e quelli del compito) siano
   identici al fixture;
2. rimette i test visibili com'erano e copia sopra i test nascosti;
3. esegue `cargo test`, `python -m unittest` o `node --test`;
4. conta le regex dei controlli strutturali.

Il compito riesce solo se tutto passa.

**Congelamento.** `congela.py` ha provato ogni compito prima di congelarlo (hash in `congelato.json`):
il fixture fallisce, la soluzione di riferimento passa, e toccare un file protetto o svuotare un test
fa fallire. Tutti e 15 superano le quattro prove. Le soluzioni stanno in `soluzioni/`, fuori dalla
copia che vede l'agente.

**Runner.** `esegui.py` fa girare un modello alla volta:
1. aspetta che il motore sia libero (M-14 finito, nessun `llama-server` acceso, nessuna build in
   corso, RAM libera sopra 16 GiB);
2. accende il modello con `m15_hold`, un esempio di Aethera come `m08_hold` che in più apre
   l'endpoint 127.0.0.1:8090;
3. per ogni compito copia il fixture in `<radice>/m15/lavoro/…` e fa un commit git;
4. prende il lock `nonio` sull'endpoint ed esegue `nonio run --json` con i tetti;
5. rilascia il lock, legge la telemetria dell'avvio ed esegue il verificatore.

Ogni compito produce una riga JSONL con:
- esito e causa;
- secondi e turni;
- token di prompt, riusati, elaborati, di output e di ragionamento;
- chiamate agli strumenti, fallite per tipo;
- compattazioni;
- file toccati;
- run id del motore;
- lato motore: richieste, prefill, decode;
- `solo_motore_locale`: il modello che ha risposto è quello acceso da Aethera, e le risposte di
  Nonio sono tutte arrivate al motore locale.

Un valore che non si può misurare è `null`, mai zero. Un riassunto della traccia (chiamate fallite,
verifiche di Nonio, motivo della fine) accompagna ogni riga.

**Configurazione per modello:**
- contesto 32.768 per tutti;
- G1 e G3 con `thinking = false`, `max_tokens` 4096;
- Flash-Coder e Flash-Next con `thinking = true`, `max_tokens` 8192 e `reasoning_effort` medium
  passato al motore (`--chat-template-kwargs`). Il loro template accetta xhigh, medium e low, con
  xhigh di default;
- campionamento dalle model card;
- Flash-Next con prompt cache spenta e 8 checkpoint (vedi sezione 6);
- il template tollerante di Claude Code non serve a Nonio e non è stato usato.

RISULTATI

## 6. Riuso del prefisso sui modelli ibridi (T-06, per M-10 T-09)

**Che cosa dice il codice di b10991** (`tools/server/server-context.cpp`, letto nel sorgente locale
del fork di M-14):
- un checkpoint dello stato ricorrente si crea:
  - due volte vicino alla fine di ogni prompt, a `n − 4 − ubatch` e a `n − 4` (PR #20288);
  - a metà prompt, solo all'inizio di un messaggio utente: l'ultimo, e quelli distanti almeno
    `--checkpoint-min-step` dal checkpoint precedente (PR #22929, che ha tolto
    `--checkpoint-every-n-tokens`, introdotto dalla PR #20087);
- b10991 contiene la PR #28302 (merged l'8-09), che smette di cancellare i checkpoint recenti quando
  il prompt è più corto di `checkpoint_min_step`. b10809 non la contiene: verificato con l'API di
  GitHub sul commit di merge;
- i checkpoint e la prompt cache (`--cache-ram`) vivono nella RAM del sistema.

**Che cosa spiega del dato di M-10 T-09:**
- **Il 71%.** Il banco usa `/completion`, che non ha messaggi, e il turno 1 non è un prefisso
  identico al turno 0: aggiunge «(turno 1 della stessa conversazione)» dopo l'istruzione. Il
  checkpoint a `n − 4` cade quindi dopo il punto in cui i due prompt divergono, e il server riparte
  da quello a `n − 4 − ubatch`. Con ubatch 2048 e circa 7.100 token, sono i 5.053 token misurati.
- **Il riuso nullo dopo una modifica a metà.** Senza messaggi utente non esiste un checkpoint prima
  della metà del prompt, e lo stato ricorrente non si può troncare. Si riparte da zero, e nessun
  flag lo cambia su `/completion`.
- **Un agente vero** (chat, cronologia in sola aggiunta) dovrebbe invece riusare quasi tutto:
  - il prompt nuovo estende quello vecchio;
  - il checkpoint a `n − 4` della richiesta precedente è valido;
  - nella batteria lo si misura dalla telemetria di ogni richiesta.
- **La RAM.** Un'entrata della prompt cache di Flash-Next a 7k pesa 530–640 MiB (log dell'avvio di
  M-10 T-09), e il default di `--cache-ram` è 8 GiB. A VGM 48, con 2–3 GiB liberi, va spenta.

T06_PROVA

Fonti: github.com/ggml-org/llama.cpp PR #16391, #20087, #20288, #22929, #26004, #28302; issue #18497,
#19794, #24055; `tools/server/README.md`. Letti il 17-09-2026.

ALTRO
