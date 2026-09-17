# Batteria di coding agentico: quale modello lavora meglio su questa macchina

> **M-15, 17-09-2026.** Misure fra le 09:45 e le 13:13, dopo il riavvio di Windows Update delle 02:33
> (Windows 26200.9457, KB5129195), VGM 48, driver GPU invariati. Batteria eseguita con tre modelli su
> quattro: Flash-Next non è partito, perché la RAM non basta più (sezione 5). Sigle in fondo.

**In breve.**
- **G1 e G3 risolvono 12 compiti su 15 ciascuno**, ma G1 lo fa in metà del tempo: 22,4 compiti
  riusciti per ora di macchina contro 12,1. Per il coding quotidiano con Nonio il profilo da usare
  resta G1 (sezione 4).
- **FC non ne risolve nessuno** in 106 minuti. Scrive patch che non sono diff validi (fino a 19
  tentativi uguali di fila), lancia comandi che restano appesi, risponde senza chiamare strumenti.
  Come modello di coding con un agente, qui non vale.
- **Più della metà dei compiti arriva al tetto dei turni anche quando è già risolta.** Le cause
  stanno nell'harness, non nei modelli, e valgono per tutti allo stesso modo (sezione 3):
  - Nonio respinge sempre il primo `finish` («una revisione prima»);
  - il suo `run_shell` su Windows rompe ogni comando con le virgolette doppie.

  Il tempo per compito è quindi gonfiato, per tutti i modelli nella stessa misura.
- **Il riuso del prefisso di M-10 T-09 è spiegato dal codice**, ma la prova non si è potuta fare:
  Flash-Next a VGM 48 non ci sta più in RAM (sezione 6).

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

**Validazione del runner (T-04).** Su G1, motore acceso (avvii `r-20260917-094453` e
`r-20260917-131307`), tre compiti (rs-durata-en, py-slug, ts-giorni):
- il fixture non toccato fallisce tutti e tre, con causa «nessuna_modifica»;
- la soluzione di riferimento li passa tutti e tre;
- gli stessi tre compiti con Nonio danno righe complete: ogni campo è misurato, oppure è `null`
  dove non ha senso (per esempio i campi di Nonio con l'agente spento).

In tutte le 45 righe con Nonio `solo_motore_locale` è vero:
- il modello che ha risposto è quello acceso da Aethera;
- le richieste contate da Nonio sono arrivate tutte al motore, con il lock `nonio`.

Nessuna ricaduta sul cloud. Senza motore, poi, Nonio si ferma con un errore d'infrastruttura
(`nonio doctor`: «cannot reach the engine»).

## 3. Risultati (T-05)

Sessione `notte-1`, un giro per modello. Righe in `<radice>/m15/risultati/notte-1/risultati.jsonl`;
le tabelle si rifanno con `python analizza.py <jsonl>`.

| modello | avvio | riusciti | ore di macchina | **riusciti per ora** | secondi per compito riuscito | turni (mediana) | decode (mediana) | riuso del prefisso |
|---|---|---|---|---|---|---|---|---|
| **G1** | r-20260917-094858 (b10809, MTP) | **12/15** | 0,53 | **22,4** | 160 | 20 | 33,5 tok/s | 96,2% |
| **G3** | r-20260917-121215 (b10809) | **12/15** | 0,99 | **12,1** | 296 | 20 | 19,9 tok/s | 94,2% |
| **FC** | r-20260917-102533 (b10991) | **0/15** | 1,76 | **0** | — | 20 | 11,9 tok/s | 94,8% |
| FN | non avviato | — | — | — | — | — | — | — |

Come leggere la tabella:
- **Ore di macchina**: la somma dei tempi dei compiti. Il caricamento del modello sta a parte:
  7 s per G1 (pesi già in cache), 19 s per FC, 34 s per G3.
- **Secondi per compito riuscito**: le ore di macchina divise per i compiti riusciti.
- **Riuso del prefisso**: token riusati diviso token di prompt, letti dalla telemetria di Aethera.
  Con Nonio la cronologia cresce solo in coda, quindi anche G3, che è ibrido, riusa il 94%.

**Per livello.** G1 e G3 hanno lo stesso profilo:
- facili 3/4;
- medi 7/8;
- difficili 2/3.

FC è a 0 in tutti e tre.

| compito | livello | G1 | G3 | FC |
|---|---|---|---|---|
| rs-durata-en | facile | ✓ 41 s | ✓ 113 s | ✗ 337 s |
| rs-durata-it | facile | ✓ 80 s | ✓ 113 s | ✗ 663 s |
| py-slug | facile | ✗ 44 s | ✓ 115 s | ✗ 322 s |
| ts-giorni | facile | ✓ 76 s | ✗ 104 s | ✗ 8 s |
| rs-lru | media | ✓ 186 s | ✓ 231 s | ✗ 236 s |
| rs-report | media | ✓ 89 s | ✓ 125 s | ✗ 462 s |
| py-intervals | media | ✓ 148 s | ✓ 131 s | ✗ 383 s |
| py-config-en | media | ✓ 139 s | ✓ 126 s | ✗ 302 s |
| py-config-it | media | ✓ 83 s | ✓ 161 s | ✗ 314 s |
| py-csvreport | media | ✓ 123 s | ✓ 218 s | ✗ 21 s |
| ts-eventi-en | media | ✓ 202 s | ✓ 89 s | ✗ 340 s |
| ts-eventi-it | media | ✗ 176 s | ✗ 184 s | ✗ 274 s |
| rs-calc | difficile | ✗ 242 s | ✗ 1293 s | ✗ 1443 s |
| py-ledger | difficile | ✓ 201 s | ✓ 296 s | ✗ 1217 s |
| ts-carrello | difficile | ✓ 96 s | ✓ 257 s | ✗ 22 s |

Nessun modello ha toccato un file protetto: né i test, né `ledger/rates.py`, né `legacy/`.
Il compito «vincolo» (py-config) è quindi passato da G1 e G3 in entrambe le lingue.

### Dove falliscono, con esempi dalle tracce

**G1 (3 fallimenti, tutti di logica o di dettaglio):**
- *py-slug*: i test visibili passano, uno nascosto no. `slugify("Straße")` dà `stra-e` invece di
  `strae`: la docstring dice che i caratteri non ASCII che non sono lettere accentate vanno
  eliminati, e il modello li trasforma in separatori. Ha chiuso in 4 turni senza rileggere la
  specifica.
- *rs-calc*: tre correzioni su quattro sono giuste, e lo dice lui stesso nel `finish`. Ma `1.2.3`
  dà `UnexpectedToken("Num(0.3)")` invece di `BadNumber("1.2.3")`: il lexer si ferma al secondo
  punto invece di consumare tutto il letterale.
- *ts-eventi-it*: `once` non toglie la registrazione **prima** di chiamarla, quindi un `emit`
  rientrante va in ricorsione infinita («Maximum call stack size exceeded»). Nella gemella
  inglese lo stesso modello l'ha fatto giusto.

**G3 (3 fallimenti):**
- *rs-calc*: 23 turni e 17.700 token di output ragionando sulle binding power. È l'unica
  compattazione della notte: la conversazione arriva a 21.134 token contro un tetto di 19.972
  e Nonio la biforca. Poi scade il tempo, con il lexer ancora sbagliato
  (`BadNumber("1.2")` invece di `"1.2.3"`).
- *ts-giorni*: 11 `run_shell` e nessuna modifica. Il modello cerca di far girare TypeScript a mano
  e finisce i 12 turni.
- *ts-eventi-it*: lo stesso errore di G1 su `once`. Ha chiuso rispondendo in italiano che «i 4 test
  passano», senza provare il caso rientrante.

**FC (15 fallimenti, quasi tutti di formato degli strumenti):**
- **Patch non valide.** In 10 compiti su 15 usa `apply_patch` con diff che non si leggono
  (`@@` senza numeri, intestazioni come `--- Fixed file appconf/loader.py`, file inventati come
  `test/sug.py` o `a/python.py`). Ripete lo **stesso** patch rifiutato fino a 19 volte di fila:
  su py-config-it sono 19 chiamate respinte su 20.
- **Chiamate scritte nel testo.** In 3 compiti (ts-giorni, py-csvreport, ts-carrello) risponde
  al primo turno con una chiamata scritta nel testo invece che come chiamata vera
  (`<function=run_shell> {"command": "cd /workspace && …"}`), con percorsi da Linux, e Nonio
  chiude.
- **Comandi appesi.** `python -import subprocess …` resta appeso finché `run_shell` non lo uccide
  dopo 300 s. Su rs-calc un argomento JSON di 17.955 caratteri tronca il turno. Tre compiti
  finiscono per tempo.
- **Ragionamento.** Lo streaming non lo separa: Nonio vede `reasoning_tokens` 0 e un `</think>`
  nel contenuto. Il modello ragiona pochissimo, come aveva già visto la sonda di M-11 T-02.

**Per tutti: il costo dell'harness.** Su 45 compiti, 26 arrivano al tetto dei turni o del tempo,
e 15 di questi sono comunque riusciti. Le due cause:
1. **Nonio respinge il primo `finish`** («Not finished yet: one review first»): 13 volte su G1,
   9 su G3. Dopo il rifiuto il modello ricomincia a verificare, e di solito lo fa con la shell.
2. **`run_shell` su Windows rompe le virgolette.** Nonio avvia `cmd /C` passando il comando come
   argomento quotato alla maniera MSVC (`\"`), che `cmd` non capisce:
   - `python -c "print('hello')"` esce con 0 e senza output;
   - un `python -c` su più righe dà «unterminated string literal».

   Riprodotto fuori da Nonio (con la quotatura: nessun output; con la riga grezza: l'output
   giusto). In più i modelli scrivono comandi da bash (heredoc, `/tmp`, `;`), che `cmd` rifiuta.

   Comandi di shell usciti con codice ≠ 0 (compresi i test che falliscono di proposito):
   G1 81 su 137, G3 109 su 151. Comandi usciti con 0 e muti: 11 e 4.

   Un caso tipico, G1 su py-config-en:
   - il compito è risolto al turno 3;
   - il `finish` del turno 5 viene respinto;
   - i turni 6–20 se ne vanno a cercare di far stampare qualcosa a `python -c`, mentre il modello
     scrive «The shell seems to swallow stdout».

   Senza queste due cause il tempo per compito di G1 e G3 sarebbe molto più basso. La
   classifica non cambia, perché le cause valgono per tutti.

**Costo del ragionamento.**
- G1 e G3 sono andati a `thinking = false`: 0 token di ragionamento misurati da Nonio. G3 non
  ha comunque un canale di ragionamento.
- Il motore ha generato 43.394 token per G1 e 51.518 per G3. Circa 6.600 e 5.500 non si vedono
  né nel testo né nelle chiamate: è formattazione delle chiamate, non ragionamento, perché
  G1 non ragionava.
- Per FC il costo del ragionamento non si può misurare: il canale non arriva separato. Il motore
  ha generato 46.967 token in 192 turni, 40.329 dei quali sono argomenti di chiamate (le patch
  rifiutate).
- Con il decode di FC a 11,9 tok/s, questi token valgono 66 minuti di decode su 106 totali.
- La prova con `reasoning_effort` low contro medium, chiesta da M-10 T-10, non è stata fatta:
  medium su FC basta già a dire che il modello non funziona con Nonio, e su FN non si è arrivati.

**Lingua (gemelli en/it).** Su 6 coppie eseguite da G1 e G3:
- 5 danno lo stesso esito nelle due lingue;
- 1 no: ts-eventi fallisce in italiano con **entrambi** i modelli e riesce in inglese con
  entrambi. L'errore è identico (`once` rientrante).

Con un giro solo non si può separare l'effetto della lingua dal caso. L'enunciato italiano rimanda
ai commenti TODO, che sono in inglese e dicono «removed right before its first call», proprio come
quello inglese. Da ripetere con più giri prima di concludere. Sui tempi:
- rs-durata in italiano costa di più (G1 80 s contro 41 s; G3 113 s contro 113 s);
- py-config in italiano costa meno per G1 (83 s contro 139 s) e di più per G3 (161 s contro 126 s).

Nessuna tendenza chiara.

**Telemetria di Aethera.** La pagina Motore conterebbe 11, 10 e 15 «compattazioni» nei tre avvii,
ma Nonio ne ha fatta una sola. La classificazione per richiesta (`telemetry::classify`) scambia
l'inizio di un compito nuovo, che ha un prompt più corto del precedente con lo stesso prefisso
fisso, per una conversazione ricostruita. Con un client che apre una sessione nuova per ogni
compito serve distinguere le due cose, per esempio dal cambio di lock.

## 4. Raccomandazione

- **Coding quotidiano con Nonio: G1 (`qwen3.6-35b-a3b.q4_k_m.vulkan`)**, così com'è nel profilo:
  b10809, MTP a 3 token, `-ub 4096`, contesto 32k, `thinking = false`, campionamento instruct.
  Risolve quanto G3, in metà tempo (22 contro 12 compiti per ora), con decode a 33 tok/s grazie
  a MTP e caricamento in pochi secondi. In più i pesi sono meno della metà della VGM.
- **G3 (Coder-Next)** vale come secondo parere sui compiti lunghi: ha risolto py-slug, dove G1
  ha sbagliato. Costa però il doppio del tempo e riempie la VGM (46 GiB). Non è da tenere acceso
  come modello di tutti i giorni.
- **FC (Flash-Coder Q4_K_M)** da scartare come modello per agenti con Nonio. M-11 T-05 lo chiude
  con verdetto negativo; il Q8_0 non è stato provato, e con un formato delle chiamate così
  sbagliato non c'è motivo di aspettarsi di meglio da una quantizzazione più fine.
- **Prima di rifare la batteria, conviene sistemare Nonio** (fuori da questo milestone): raw arg
  per `cmd /C` su Windows e un `finish` che non costi un giro di verifica alla cieca. Poi due o
  tre giri per modello, per avere pass^k e separare l'effetto della lingua.

**Dati per gli altri milestone:**
- **M-10 T-10** (G1 con Nonio): G1 fa 12/15 in 0,53 h, cioè 22,4 compiti per ora, con 0 token di
  ragionamento per turno (`thinking` spento). **Manca FN** e la prova low/medium, quindi il task
  resta aperto.
- **M-11 T-05**: FC Q4_K_M fa 0/15 in 1,76 h. Errori di chiamata: 121 respinte su 186 (65%),
  quasi tutte `apply_patch` non leggibili. In italiano fallisce come in inglese. Il confronto è
  contro G1 12/15 e G3 12/15. **Mancano FC Q8_0 e FN**; il verdetto su FC Q4_K_M però è netto.

## 5. Flash-Next non eseguito, e perché

**Che cosa è successo:**
- Il runner ha rifiutato l'avvio di FN alle 13:12: RAM libera 33,9 GiB, sotto i 39 fissati.
- La soglia è stata alzata dopo che la prova di T-06, alle 10:19, aveva mostrato il problema:
  - avvio `r-20260917-101913`: IQ3_XXS, `load_mode none`, `--cache-ram 0`, riga del profilo;
  - pronto in 76 s;
  - al primo prefill la RAM disponibile scende a **0,05 GiB** e il sorvegliante ferma llama-server.

**Perché la stessa riga del 16-09 non ci sta più:**
- In M-10 T-04 (16-09, VGM 48, `none`) si partiva da **41,3 GiB** disponibili e si scendeva al
  minimo a 2,9. Il modello occupa quindi circa 38,4 GiB di RAM: la tabella n-gram in memoria
  privata, la parte condivisa della GPU e i buffer.
- Il 17-09, dopo il riavvio di Windows Update, a macchina ferma i GiB disponibili sono **34,9**
  su 47,6 visibili, e 18,4 GiB sono già impegnati senza nessun motore.
- Mancano circa 6 GiB, e da qui non si vede dove siano finiti. Nessun processo ne usa più di 1:
  i candidati sono componenti di sistema dopo KB5129195 o Smart App Control, acceso e poi
  spento stamattina.

**Che cosa servirebbe per eseguire FN e T-06** (decide l'operatore):
1. **Ritrovare i ~6 GiB** (RAMMap, commit a vuoto, servizi nuovi dopo l'aggiornamento) e tornare
   sopra i 39 GiB disponibili: la stessa riga di M-10 T-04 allora ci sta con 2–3 GiB di margine.
2. **Oppure VGM a 64 con `mmap`** e `--lazy-mode auto`, come in M-10 T-07/T-08/T-09: lì FN girava
   a 9 tok/s con la tabella n-gram lasciata sul file. Serve il cambio di VGM da Adrenalin e un
   riavvio: è una scelta dell'operatore, e la VGM ora deve restare a 48.
3. In entrambi i casi basta rilanciare `python .lmbrain-lite/m15/notte.py --sessione notte-1`:
   le fasi fatte si saltano, restano t06 e fn.

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

**La prova (non conclusa).**
- Scenario `.lmbrain-lite/m15/T-06-riuso.toml`: lo scenario di M-10 T-09 con `load_mode none`,
  `--cache-ram 0` e due varianti, la riga di default (32 checkpoint, passo minimo 8192) e
  `--ctx-checkpoints 16 --checkpoint-min-step 0`.
- Fermata dal sorvegliante al primo turno della prima variante, per la RAM (sezione 5). Le righe
  parziali sono in `<radice>/m15/T-06-fermato-dal-sorvegliante.jsonl` (vuoto) e `T-06.out`.
- **M-10 T-09 non si chiude.** Dal codice, però, la risposta attesa è già chiara:
  - sul banco `/completion` nessuna combinazione di flag recupera il riuso dopo una modifica a
    metà prompt;
  - con un client a chat che estende la cronologia il riuso c'è già: G3, ibrido anche lui, ha
    riusato il 94% dei token di prompt con Nonio in questa batteria.

  Per un modello ibrido la domanda utile è «quanto costa una modifica a metà cronologia»,
  e va misurata con messaggi veri (`/v1/chat/completions` e un messaggio utente dopo la modifica),
  non con `/completion`.

Fonti: github.com/ggml-org/llama.cpp PR #16391, #20087, #20288, #22929, #26004, #28302; issue #18497,
#19794, #24055; `tools/server/README.md`. Letti il 17-09-2026.

## 7. Che cosa resta

| cosa | di chi | come |
|---|---|---|
| FN nella batteria e prova T-06 | operatore, poi agente | ritrovare ~6 GiB di RAM (sezione 5) o VGM 64 con mmap; poi `notte.py` riprende da solo |
| `run_shell` su Windows (quotatura di `cmd /C`) e `finish` respinto alla cieca | Nonio (fuori da M-15) | raw arg su Windows; revisione del `finish` che non costi turni |
| classificazione delle compattazioni con un client a compiti brevi | Aethera (telemetria) | distinguere un compito nuovo da una conversazione ricostruita |
| più giri per modello (pass^k, effetto della lingua) | agente | `esegui.py --ripetizioni 3` dopo le correzioni di Nonio |
| FC Q8_0, `reasoning_effort` low contro medium | facoltativo | solo se FN o un Nonio corretto cambiano il quadro |

## Sigle

| sigla | modello | profilo di Aethera |
|---|---|---|
| **G1** | Qwen3.6-35B-A3B, Q4_K_M (bartowski) | `qwen3.6-35b-a3b.q4_k_m.vulkan` |
| **G3** | Qwen3-Coder-Next, Q4_K_M (unsloth) | `qwen3-coder-next.q4_k_m.vulkan` |
| **FC** | Qwen3.8-Flash-Coder, Q4_K_M: il taglio «coding» di Jab1718 (160 esperti su 512) | `qwen3.8-flash-coder.q4_k_m.vulkan` |
| **FN** | Qwen3.8-Flash-Next intero, UD-IQ3_XXS (unsloth) | `qwen3.8-flash-next.iq3_xxs.vulkan` |
