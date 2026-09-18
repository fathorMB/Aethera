# Nonio: i due difetti misurati dalla batteria, e il riuso del prefisso sugli ibridi

> **M-18, 18-09-2026.** Lavoro fatto su un ramo `aethera/m18-harness` nella copia di sviluppo
> dell'operatore, a partire da `main`. Niente push, niente PR, `main` invariato. La radice del
> repository di Nonio è scritta `<nonio>`. Il riferimento tecnico sul server è
> `.lmbrain-lite/m17/lettura-codice-b10991.md`; i numeri sui difetti vengono da
> `.lmbrain-lite/reports/batteria-coding-2026-09.md` §3.

**In breve.**
- **`run_shell` su Windows**: corretto. `cmd` riceveva la riga quotata alla maniera MSVC, che non
  sa leggere; ora la riga è scritta grezza (`raw_arg`), `cmd /S /C` con il comando avvolto una
  volta sola. Nello stesso punto è emerso un secondo difetto non sospettato: **il timeout non
  uccideva niente**, perché tokio abbandona il figlio invece di ammazzarlo. Ora muore l'albero.
- **Revisione al primo `finish`**: la salvaguardia resta, ma prende un criterio. Il `finish` è
  respinto solo se qualcosa ha scritto nel workspace dopo l'ultimo comando uscito con 0, o se in
  tutto il lavoro nessun comando è mai uscito con 0. Chi ha modificato, lanciato i test e visti
  passare chiude al primo colpo.
- **Riuso del prefisso**: Nonio è già costruito per questo e la parte grossa è a posto (coda in
  sola aggiunta, prefisso congelato, prompt identico byte per byte con tanto di test). Quello che
  rompe il riuso sugli ibridi è **il ragionamento che il template non riprende in ingresso**, ed è
  un turno su un turno; poi, più raramente, i turni tagliati al tetto dell'uscita e le chiamate
  malformate. **Nonio non usa `cache_prompt`, `id_slot` né `/slots` save/restore**, e non c'è modo
  di riprendere una sessione contro uno slot caldo.
- **Niente è stato compilato.** Il motore (`llama-server.exe`, `m08_bench.exe`) è rimasto acceso
  per tutta la sessione; secondo il vincolo di macchina non è stato lanciato nessun `cargo`. Gli
  esiti dei test sono quindi **da verificare**: sezione 5.

## 1. Il primo `finish` respinto sempre

**Dove nasce.** `<nonio>/crates/nonio-core/src/run.rs`:

- `FINISH_REVIEW` (la frase «Not finished yet: one review first…») era a riga 46;
- l'armamento era un `AtomicBool` inizializzato da `RunOptions::review_finish` nel ciclo
  (`execute`), e il rifiuto avveniva in `run_one`: primo `finish` riuscito → `swap(false)` →
  l'esito diventa un errore con quel testo.
- Chi lo accende: `<nonio>/crates/nonio-session/src/compose.rs:349` (`review_finish: true`),
  `<nonio>/crates/nonio-cli/src/main.rs:921` e `<nonio>/desktop/src-tauri/src/session.rs:177`, che
  lo spengono attraverso i fork quando `RunReport::finish_reviewed` dice che la revisione c'è
  stata. Il banco di conformità lo tiene spento (`nonio-conformance/src/runner.rs:358`).

**L'intenzione originale, dal commento nel codice.** Sul banco delle tre sfide, in ogni corsa
chiusa con `finish` i test visibili passavano e quelli nascosti no, sempre su un requisito
enunciato a parole e non coperto da nessun test del workspace. La revisione non nomina né requisito
né test: chiede di rileggere il compito. È quindi **una difesa contro il codice su cui non è
passato niente**, non contro la fretta in generale. La commit `a9cd02a` («La revisione del primo
finish vale una volta per il lavoro, non per sessione») aveva già ristretto una volta l'ambito.

**Il costo misurato** (batteria §3): 13 rifiuti su G1, 9 su G3; dopo il rifiuto il modello
ricomincia a verificare, di solito con la shell; su 45 compiti 26 arrivano a un tetto e 15 di
questi erano già risolti.

**Correzione fatta** (commit `f38fcff`). La salvaguardia non è tolta: prende un criterio.
Il primo `finish` è respinto quando

- qualcosa ha scritto nel workspace **dopo** l'ultimo comando uscito con 0, **oppure**
- in tutto il lavoro nessun comando è mai uscito con 0.

Cioè: chi ha modificato e chiuso alla cieca viene rimandato indietro; chi ha modificato, lanciato i
test e visti passare chiude subito, perché la rilettura alla cieca non aggiunge niente che il
rifiuto possa imporre.

Due dettagli di disegno, per non tornarci sopra:

- **che cosa sia una modifica lo dichiara lo strumento**, non lo deduce il ciclo. Nuova costante
  `nonio_core::tool::WORKSPACE_CHANGED` (`<nonio>/crates/nonio-core/src/tool.rs:124`), messa fra i
  `facts` da `apply_patch` (`builtin/write.rs`) e `replace_in_file` (`builtin/replace.rs`).
  Dedurlo dagli altri numeri sarebbe stato indovinare: `git_status_diff` riporta `files_changed` e
  non cambia niente.
- **che cosa sia una verifica** è un `exit_code` a 0 (`EXIT_CODE`, stessa riga 128), che copre
  `run_shell` e `terminal_run`. Sono fatti già scritti nella traccia, quindi la decisione si
  rilegge da una sessione invece di essere ricostruita.

Il codice nuovo è la struttura `ReviewGate` (`run.rs:74-141`), con `observe` (riga 101) che legge
un risultato e `owes_a_review` (riga 125) che decide e consuma l'armamento.

**Che cosa resta da decidere all'operatore.**

1. **Un comando qualsiasi uscito con 0 vale come verifica.** Un modello che tocca un file e poi
   lancia `dir` viene considerato verificato. Il criterio si può stringere (solo comandi che
   somigliano a un test o a una build, oppure solo il verificatore del progetto, `nonio-tools/verify.rs`),
   ma ogni stretta è una regola che indovina che cosa sia un test.
2. **Non è ancora un'opzione di profilo.** Il `review_finish` esiste come booleano in
   `RunOptions`, ma non c'è una chiave nel TOML. Il progetto sarebbe una sezione `[run]` con
   `finish_review = "when_unverified" | "always" | "off"`, valore di default
   `when_unverified`, portata da `Profile` → `EffectiveConfig` → `compose`. Non l'ho fatta perché
   tocca quattro crate e la macchina non permetteva di compilare; è mezz'ora di lavoro meccanico.
3. Uno strumento MCP che scrive file senza dichiarare `workspace_changed` lascia il ciclo a credere
   che il lavoro sia dove l'ha lasciato l'ultimo comando. È scritto nel commento della costante.

## 2. `run_shell` su Windows

**Dove nasce.** `<nonio>/crates/nonio-tools/src/process.rs`, funzione `shell_command`: era

```rust
let mut shell = tokio::process::Command::new("cmd");
shell.arg("/C").arg(command);
```

`Command::arg` quota come vuole il runtime C di MSVC: raddoppia i backslash e scrive `\"` per una
virgoletta. `cmd.exe` non legge così: vede un backslash letterale e poi una virgoletta che tratta
come propria. Da qui i due sintomi della batteria: `python -c "print('hello')"` esce con 0 e senza
niente, e uno script su due righe arriva con la stringa non chiusa.

**Correzione fatta** (commit `923309f`, `process.rs:157-190`). Su Windows la riga dopo `cmd` la
scrive `windows_command_line`: `/S /C "` + il comando + `"`, passata con `raw_arg` (che tokio
espone proprio per questo). `/S` è la parte che conta: senza, `cmd` toglie le virgolette esterne
**solo** se dentro non ce ne sono altre, quindi un comando con virgolette e uno senza sono trattati
in modo diverso; con `/S` la regola non ha casi — via il primo e l'ultimo carattere, il resto è il
comando com'è scritto. Così si avvolge una volta e non si escapa mai niente.

Due cose che restano rotte e non sono quotatura: `%` viene espanso da `cmd` prima che il comando
parta, e un a capo letterale chiude la riga di comando, quindi uno script su più righe non è una
cosa che `cmd /C` possa ricevere. Sono scritte nel commento della funzione.

**Il secondo difetto, trovato guardando i «comandi appesi».** Lo stdin era già chiuso
(`Stdio::null()`, riga 68), quindi un `python` che legge stdin prende EOF subito: non era quello.
Il vero guaio è che **il timeout non uccideva niente**. Il commento diceva «tokio's Child kills on
drop by default»: non è vero, `kill_on_drop` è falso di default e tokio *abbandona* il figlio, che
resta a girare e viene solo raccolto in background. Un `python` avviato da `cmd` sopravviveva quindi
al proprio tempo, e continuava a mangiare CPU mentre il modello andava avanti.

Ora: `kill_on_drop(true)` sul figlio, e allo scadere del tempo, **prima** che il figlio venga
lasciato cadere, `taskkill /T /F /PID` percorre l'albero (`process.rs:128-141`). L'ordine è
deliberato: ammazzare la shell per prima lascia i nipoti orfani, ed è proprio il caso normale.
Su Linux e macOS non c'è niente di più per ora — cade `sh`, i figli no — e il commento lo dice
invece di far finta.

Due note di contorno. Il `terminal_run` (`<nonio>/crates/nonio-tools/src/terminal.rs`) **non ha
questo difetto**: lì la shell vive dentro una pty e il comando viene scritto nel suo stdin, non
passato sulla riga di comando. E un `kill_tree` con `taskkill /T` esisteva già lì (riga 575), per
lo stesso motivo: ora ce ne sono due, uno su `std::process::Command` e uno su quella di tokio.
Unificarli è un lavoretto, non l'ho fatto.

**Test aggiunti** (in `process.rs`, modulo `tests`):

- `the_windows_command_line_wraps_once_and_escapes_nothing`: la riga come stringa, per i tre casi
  (virgolette doppie, virgolette annidate, percorso con spazi più `&&`);
- `quoted_commands_reach_the_shell_whole`: gli stessi casi attraverso un `cmd` vero;
- `python_dash_c_prints_what_it_was_asked_to_print`: `python -c` semplice, con virgolette annidate,
  e con più istruzioni separate da `;`. Salta se python non c'è;
- `the_timeout_kills_what_the_shell_started`: il nipote scrive su un file, il timeout scatta, e il
  file smette di crescere. Chiesto al workspace e non alla tabella dei processi, perché un test che
  cerca un processo per nome se lo trova di qualcun altro.

## 3. Riuso del prefisso sui modelli ibridi

Le quattro domande, con la risposta letta nel codice.

### 3.1 La conversazione è davvero solo in coda?

**Sì, per costruzione, con tre eccezioni.**

- `<nonio>/crates/nonio-core/src/conversation.rs:116` — `Conversation::push` è l'unico modo di
  cambiare la coda: niente rimozione, niente riscrittura, niente compattazione in loco.
- `<nonio>/crates/nonio-core/src/prompt.rs:114` e `374` — il segmento `conversation` cresce e basta;
  gli altri sono congelati dopo il primo turno (`Session::set` riga 357 rifiuta).
- Il tetto sull'uscita di uno strumento (`run.rs`, `fit`, riga 893) taglia **al momento in cui il
  risultato entra**, una volta sola, e il resto va in un artefatto. Non c'è nessun troncamento
  retroattivo degli output vecchi dopo N turni.

Le tre eccezioni, in ordine di danno:

1. **Turno tagliato al tetto dell'uscita** — `run.rs:520-524` sul `main` di partenza, oggi riga 610.
   Un turno che l'engine ferma a `length` prima di qualunque chiamata **non entra** nella coda: al
   suo posto va un messaggio `user` con l'avviso. Il testo che il modello ha davvero generato
   sparisce. Sugli ibridi questo è esattamente il caso peggiore: il prompt nuovo diverge dentro il
   generato, il server risale al checkpoint precedente e ri-elabora (M-17 §2). Sulla batteria è
   successo poco (6 corse su 18 del banco a tre sfide), ma quando succede costa.
2. **Chiamate malformate** — `run.rs:498-505` sul `main` di partenza, oggi 587-594. Solo le chiamate valide vengono rimandate indietro
   nel messaggio `assistant`; quella malformata sparisce, **ma il suo risultato d'errore viene
   comunque messo in coda** (oggi `run.rs:679`) con un `tool_call_id` che non corrisponde più a nessuna
   chiamata. Doppio guaio: il prompt non è più quello che il modello ha generato, e il template
   riceve un `tool` che risponde al nulla. Con FC erano 121 chiamate respinte su 186.
3. **Compattazione** — `<nonio>/crates/nonio-session/src/fork.rs`. Scatta **prima** di mandare,
   quando la coda supera il contesto meno la riserva (`run.rs`, `first_past_context`, riga 507):
   il ciclo si ferma con `StopReason::ConversationFull` e non biforca da solo; è `nonio-cli`
   (`main.rs:1139`) ad aprire la sessione che continua. Dove taglia: **non taglia**. Apre un prompt
   nuovo, con il compito originale parola per parola più dei fatti presi da `git diff`, e il
   racconto del modello come primo messaggio della conversazione nuova. Quindi il prefisso è tutto
   diverso e si ri-elabora tutto. Nonio lo sa e lo misura invece di prometterlo: RF-12a e
   `ForkReuse` in `<nonio>/crates/nonio-core/src/cache.rs:37-58`.

### 3.2 Prompt di sistema e lista degli strumenti identici byte per byte?

**Sì, ed è testato.**

- Ordine invariante dei segmenti e separatore fisso: `prompt.rs:15` e `render_prefix` riga 141.
- Nessuna data, nessuna ora, nessun `cwd`: le uniche `SystemTime::now()` del crate di sessione sono
  nell'id di sessione (`compose.rs:423`) e nel registro dei progetti, e non toccano il prompt.
- Ordine degli strumenti: `ToolSet::freeze` ordina per nome (`tool.rs:305-308`), e la
  serializzazione degli schemi passa da `serde_json` senza `preserve_order`, cioè mappe ordinate.
- Due test lo tengono fermo: `compose.rs` `two_executions_produce_the_same_prompt_byte_for_byte`
  (riga 873, scritto proprio contro il seme casuale degli `HashMap`) e
  `the_workspace_it_ran_in_does_not_change_the_prompt` (riga 894).

**Niente da correggere qui.** Una cosa da tenere d'occhio: `chat_template_kwargs.enable_thinking`
cambia da un turno all'altro quando il ciclo spegne il ragionamento dopo un taglio
(`run.rs`, `reasoning_off`, righe 522 e 607-608; famiglia in
`<nonio>/crates/nonio-family/src/qwen.rs:42`). Sul template di Qwen quell'argomento si traduce in
testo aggiunto in coda al prompt, non nel sistema, quindi in teoria non tocca il prefisso: **da
confermare sul template vero** guardando `cache_n` nel turno seguente.

### 3.3 La risposta del modello viene rimandata identica?

Tre risposte distinte.

- **`reasoning_content`**: viene rimandato **solo se** `/props` dice
  `supports_preserve_reasoning: true` (`<nonio>/crates/nonio-family/src/openai.rs:99-104`,
  capacità letta in `<nonio>/crates/nonio-backend/src/props.rs:52`). Sui modelli in cui è falso —
  cioè quelli su cui abbiamo misurato — **il ragionamento generato non torna nel prompt**. È la
  rottura peggiore che ci sia: ogni turno in cui il modello ragiona fa divergere il prompt dentro
  il generato, e il server riparte da un checkpoint. Spiega a posteriori perché G1 e G3 sono stati
  messi a `thinking = false`. Ho reso esplicita la conseguenza nella nota di capacità
  (`<nonio>/crates/nonio-core/src/backend.rs:159-172`, commit `2a8e71b`): diceva solo che il
  ragionamento «è contato e tenuto ma mai rimandato», e non che il turno dopo non coincide più con
  la cache.
- **Chiamate agli strumenti ri-serializzate**: sì, ma **non è una cosa che Nonio possa sistemare
  da solo, e forse non va sistemata.** `openai.rs:85` manda `call.arguments.to_string()`, cioè il
  JSON ri-serializzato da `serde_json` (chiavi ordinate, niente spazi), non i byte che il modello
  ha scritto. Però il pezzo di prompt che conta non è questo: è quello che il **template Jinja**
  produce a partire da qui, e llama.cpp passa al template l'oggetto già analizzato, che rende con
  il proprio dump. Quindi una normalizzazione c'è comunque, dal lato del server, e rimandare i byte
  grezzi del modello potrebbe peggiorare invece di migliorare. **Non ho toccato niente**, e la cosa
  giusta da fare è misurarla: Nonio registra già i token riusati e l'attribuzione
  (`cache.rs`, `Invalidation`), quindi un turno che diverge sulle chiamate si vede nella traccia
  senza indovinare.
- **Spazi e a capo finali**: il contenuto non viene tagliato. `finalize`
  (`openai.rs:246-310`) usa `trim()` solo per **decidere** se il canale è vuoto, e poi mette in
  coda `content` intero. Nei messaggi del prompt invece `messages()` fa `.trim()` sui segmenti
  `system`, `repo_map`, `memory`, `task` (`openai.rs:34` e `46`): sono segmenti congelati, quindi
  il trim è stabile e non è un problema.

### 3.4 `cache_prompt`, `id_slot`, `/slots`?

**No a tutti e tre.**

- Nessuna occorrenza di `cache_prompt` né di `id_slot` in tutto il codice: il corpo della richiesta
  è quello di `openai::base_body` (`openai.rs:130-169`) più il `chat_template_kwargs` della
  famiglia. Nonio si affida quindi al valore di default del server (oggi `cache_prompt` è acceso di
  suo), senza dirlo e senza poterlo scegliere.
- `/slots` è usato **solo per sapere se esiste** (`<nonio>/crates/nonio-backend/src/llamacpp.rs:124-134`):
  serve a decidere se le àncore di RF-8 sono disponibili. Nessun `POST /slots/{id}?action=save` o
  `restore`.
- **Riprendere una sessione**: esiste `nonio resume`
  (`<nonio>/crates/nonio-session/src/resume.rs`), e fa la cosa giusta dal lato del prompt —
  rilegge i testi congelati dalla traccia invece di rigenerarli, proprio perché un byte diverso
  romperebbe il prefisso, e non ripristina un turno monco. Ma è un ripristino **del prompt**, non
  **dello slot**: se il server è stato riavviato, il prompt va rielaborato da zero, e a 128-256k
  sono i minuti di cui parla M-17.

### 3.5 Progetti, per le cose troppo grosse per essere fatte qui

Nessuno di questi è stato implementato.

1. **Salvataggio e ripristino dello slot su disco.** A fine sessione
   `POST /slots/{id}?action=save&filename=<sessione>.bin`, alla ripresa `restore` prima del primo
   turno, con `id_slot` fissato per tutta la sessione. Guadagno: `nonio resume` contro un server
   riavviato diventa gratis invece di costare un prefill intero. Costi e trappole: il file pesa
   quanto lo stato (su Coder-Next l'ordine è quello dei checkpoint, decine di MiB, ma qui è lo
   stato **intero**, non il solo ricorrente); il server deve essere lo stesso build e lo stesso
   modello, quindi serve l'impronta che RF-10 già prevede; e va deciso che cosa fare quando il
   ripristino fallisce (ricadere sul prefill, dicendolo).
2. **Turno tagliato: tenere il testo invece di sostituirlo.** Oggi il testo generato sparisce e al
   suo posto va un avviso `user`. L'alternativa è tenere il testo tagliato come messaggio
   `assistant` e mettere l'avviso **dopo**: la coda resta in sola aggiunta e il prefisso regge.
   Non è gratis: il testo tagliato è spesso mezzo ragionamento, che il template non riprende
   comunque. Da decidere con una misura, non a tavolino.
3. **Chiamate malformate: rimandarle indietro come le ha scritte il modello.** Nel formato OpenAI
   `arguments` è una **stringa**, quindi una chiamata malformata si potrebbe rimandare tale e
   quale. Richiede di portare il testo grezzo dentro `ToolCall`
   (`<nonio>/crates/nonio-core/src/tool.rs:31`), che è un campo nuovo su una struttura costruita in
   24 punti fra crate e test: meccanico ma largo, e non l'ho fatto senza poter compilare.
4. **Compattazione solo ai confini dei messaggi utente.** Oggi non si pone: la compattazione è un
   fork, cioè un prompt nuovo. Una compattazione «in coda» — che tenesse il prefisso e riscrivesse
   solo la parte vecchia — sugli ibridi **non ha senso**, perché riscrivere la parte vecchia è
   proprio la divergenza che costa di più. Se si vuole risparmiare, la leva è non arrivare a
   compattare (tetto sull'uscita degli strumenti, artefatti) — che Nonio già fa.
5. **Dire a che punto si è.** `cache_prompt` e `id_slot` espliciti nel corpo, e nell'intestazione
   di RF-31 una riga che dica quale slot si sta usando. Piccolo, ma è la differenza fra affidarsi
   a un default e sceglierlo.

## 4. Commit, uno per uno

Ramo `aethera/m18-harness`, staccato da `main` (`1e90d0d`). Tutti locali.

| hash | che cosa |
| --- | --- |
| `923309f` | `run_shell` su Windows: riga di comando grezza per `cmd /S /C`; `kill_on_drop` e `taskkill /T /F` al timeout; cinque test |
| `f38fcff` | la revisione al primo `finish` solo se il lavoro non è verificato; `WORKSPACE_CHANGED` e `EXIT_CODE` come fatti; `ReviewGate`; due test nuovi |
| `2a8e71b` | la nota di capacità su RF-29 dice anche che cosa costa in cache |
| `32b8177` | ripulitura: `shell_command` e `kill_tree` come due definizioni per piattaforma invece di blocchi `cfg` in coda, e tre punti fragili tolti |

La modifica non committata dell'operatore ad `AGENTS.md` è rimasta dov'era: non toccata, non messa
in commit, nessuno stash.

## 5. Esiti dei test: **tutto verde**

Scritto con il motore acceso, quindi senza compilare: il vincolo di macchina vieta `cargo` mentre
girano le misure. La verifica è stata fatta dalla sessione principale il 18-09 alle 02:11-02:12,
nella prima finestra a motore fermo (fra M-17 T-03 e i download di M-18):

| comando | esito |
|---|---|
| `cargo check --workspace --all-targets` | ok, 22,15 s, dieci crate |
| `cargo test -j 4 --workspace` | **629 passati, 0 falliti**, 3 ignorati (pre-esistenti) |
| `cargo clippy -j 4 --workspace --all-targets` | nessun avviso e nessun errore |

I tre punti che il rapporto dava per sospetti sono tutti a posto: `raw_arg` su
`tokio::process::Command` compila, il `tokio::select!` con `Box::pin` in `process::run` compila, e
i percorsi `nonio_core::tool::WORKSPACE_CHANGED` nei due strumenti di scrittura sono giusti.

Resta vero che **niente è stato provato contro un motore vero**: i test sono unitari. Le prove con
`llama-server` e la batteria sono la sezione 6.

## 6. Misure prima/dopo che la sessione principale dovrà fare

Tutte sulla batteria di coding, stesso profilo G1 (`qwen3.6-35b-a3b.q4_k_m.vulkan`), stesso motore,
stesso numero di giri del 17-09, per confronto diretto con
`.lmbrain-lite/reports/batteria-coding-2026-09.md`.

**Per la revisione al `finish`** (sezione 1):
- numero di `finish` respinti per giro: atteso da 13 (G1) e 9 (G3) a **vicino a zero** sui compiti
  che il modello verifica, e diverso da zero solo sui compiti chiusi alla cieca;
- compiti arrivati al tetto dei turni o del tempo: 26 su 45 prima;
- turni per compito e secondi per compito, medie;
- **controllo che serve davvero**: i compiti risolti devono restare 12 su 15 per G1. Se scendono,
  la revisione stava rimediando a qualcosa che il criterio non vede, e va allargato.

**Per `run_shell`** (sezione 2):
- comandi usciti con 0 **e muti**: 11 (G1) e 4 (G3) prima, attesi **zero**;
- comandi usciti con codice ≠ 0: 81 su 137 (G1) e 109 su 151 (G3) prima. Non devono andare a zero —
  i test che falliscono di proposito contano — ma la quota dovrebbe scendere;
- numero di `run_shell` per compito: su ts-giorni erano 11 senza nessuna modifica;
- **da guardare a parte**: nessun processo `python.exe` o simile deve restare vivo dopo la fine di
  un compito. Prima del giro, `tasklist` a vuoto; dopo, di nuovo.

**Per il riuso del prefisso** (sezione 3), che non è una misura della batteria ma una lettura della
traccia di Nonio, sugli stessi giri:
- per ogni turno, `cache_n` contro la lunghezza del prompt, e l'attribuzione di `Invalidation`:
  quanti turni riusano tutto, quanti ripartono da `conversation`, quanti da più in su;
- gli stessi numeri con `thinking = true` su un modello che dichiara
  `supports_preserve_reasoning: false`: è la conferma diretta del punto 3.3, e va fatta su **un
  compito solo**, non su una batteria intera, perché costa;
- quante compattazioni vere ci sono state (una sola, il 17-09) e che cosa ha riusato la sessione
  che continua, cioè `ForkReuse` nella traccia.
