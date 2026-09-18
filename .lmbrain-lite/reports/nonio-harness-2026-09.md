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
- **Compilato e testato**, dopo che la macchina si è liberata a fine sessione: `cargo check`
  pulito su tutto il workspace (desktop compreso), **616 test verdi, 0 falliti**, `clippy -D
  warnings` pulito. Le due correzioni sono state anche verificate al contrario, rimettendo per un
  momento il codice vecchio: senza la riga grezza `python -c "print('hello')"` esce con 0 e
  stdout vuoto — il sintomo della batteria, riprodotto; senza il `taskkill` il nipote continua a
  scrivere dopo il timeout. Dettagli in sezione 5.

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
   tocca quattro crate e non era chiesta; è mezz'ora di lavoro meccanico.
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
   24 punti fra crate e test: meccanico ma largo, e va deciso prima se serve davvero (vedi
   sopra: il server normalizza comunque).
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

## 5. Esiti dei test: verdi

La macchina si è liberata a fine sessione (`llama-server.exe` e `m08_bench.exe` spenti, controllato
con `tasklist` prima di ogni invocazione, tutte con `-j 4`). Risultati veri:

| comando | esito |
| --- | --- |
| `cargo check --workspace --all-targets` | pulito, `nonio-desktop` compreso |
| `cargo test -p nonio-tools process::` | **8 passati, 0 falliti** |
| `cargo test -p nonio-core run::` | **22 passati, 0 falliti** |
| `cargo test` (default-members) | **616 passati, 0 falliti, 3 ignorati** |
| `cargo clippy --workspace --all-targets -- -D warnings` | pulito |

I test nuovi sono tutti fra questi, e nessuno è saltato: python c'è (3.13.15), quindi
`python_dash_c_prints_what_it_was_asked_to_print` e `the_timeout_kills_what_the_shell_started`
hanno girato davvero.

**Controprova, che vale più dei verdi.** Un test che passa non dice che stava misurando qualcosa.
Ho rimesso per un momento il codice vecchio e rilanciato:

- con `.arg("/C").arg(command)` al posto della riga grezza,
  `python -c "print('hello')"` torna
  `Finished { exit_code: Some(0), stdout: "", stderr: "", duration: 56.8ms }`. È **esattamente** il
  sintomo della batteria — esce con 0 e non dice niente — riprodotto dentro una prova automatica;
- senza la chiamata a `kill_tree`, il file del nipote passa da 26 a 44 byte **dopo** che il timeout
  ha ucciso la shell: il `python` sopravvive al proprio tempo. Il test fallisce con
  «the grandchild outlived the timeout and kept writing».

Il codice vecchio è stato poi ripristinato da git e i test rilanciati verdi. Nessuna modifica
residua: l'unica cosa non committata nel repository resta `AGENTS.md`, quella dell'operatore.

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

## 7. Il riuso: che cosa è stato fatto

> **M-18, seguito, 18-09-2026.** Lavoro fatto su un ramo `aethera/m18-riuso` nella copia di
> sviluppo dell'operatore, staccato da `main` (`608a67a`, che ha già dentro le tre correzioni
> della sessione precedente). Quattro commit, tutti locali, nessun push, nessuna PR. La modifica
> non committata dell'operatore ad `AGENTS.md` non è stata toccata.
>
> **Verifica finale in corso, non ancora vista da chi scrive.** `cargo test` (workspace) e
> `cargo clippy --workspace --all-targets -- -D warnings` sono stati messi in coda
> dall'operatore dietro la batteria che sta girando sulla macchina; questa sezione non dichiara
> verdi risultati che non sono ancora arrivati. Quello che segue è stato verificato prima che la
> macchina tornasse occupata: `cargo check --workspace --all-targets` pulito;
> `cargo test -p nonio-core -p nonio-family -p nonio-session -p nonio-conformance` con **tutti i
> pacchetti verdi** (622 test contando le cifre viste una per una, 0 falliti) dopo aver corretto
> un indice di test rimasto dal comportamento vecchio (sotto, commit `4d51ba1`); un run completo
> di `cargo test -j 4` su tutto il workspace, verde ovunque, fatto **prima** che l'ultima
> correzione venisse committata la seconda volta — quindi ripetuto per intero solo sui pacchetti
> toccati, non sull'intero workspace, dopo quella correzione. `cargo clippy` non è stato rilanciato
> dopo l'ultimo commit. I numeri veri, sui tre comandi per intero, li scrive l'operatore quando la
> coda libera la macchina.

### 7.1 I tre punti, in ordine

**Punto 2 — il turno tagliato al tetto dell'uscita** (commit `c031c90`). `run.rs`, dentro
`execute`: quando l'engine ferma il modello al tetto dell'uscita prima di qualunque chiamata, il
testo che ha davvero generato non spariva più dietro un messaggio `user` di avviso — restava,
come proprio turno `assistant`, e l'avviso lo seguiva come messaggio nuovo. La coda cresce
soltanto; niente di già mandato viene riscritto. `nonio resume` ricostruiva ancora la forma
vecchia (solo l'avviso) leggendo la traccia: corretto nello stesso giro (commit `3d306e3`), perché
altrimenti una sessione ripresa dopo un taglio avrebbe rimandato un prompt diverso da quello che
la stessa sessione, non interrotta, avrebbe mandato — la stessa divergenza che questo lavoro esiste
per togliere, reintrodotta dalla parte sbagliata.

Una cosa che **non** è cambiata, e valeva la pena dirlo esplicitamente perché è dove un intreccio
del genere di solito si rompe: la salvaguardia che spegne il ragionamento sul turno subito dopo un
taglio fatto con il ragionamento acceso (`reasoning_off`, perché altrimenti il turno seguente
rischia di essere tagliato di nuovo prima della chiamata) non è stata toccata. `off_next` si
calcola esattamente come prima; il commit non tocca quella logica, solo che cosa succede al testo
del turno tagliato prima di lei. Il criterio che decide "quando spegnere il ragionamento" resta lo
stesso; è cambiato solo se il testo del turno tagliato entra o no nella coda.

**Punto 3 — le chiamate malformate** (stesso commit `c031c90`). Il messaggio `assistant` includeva
solo le chiamate valide (`ParsedToolCall::Valid`), ma il ciclo di esecuzione gira su *tutte* le
chiamate — anche le malformate — e mette in coda un risultato d'errore per ciascuna. Una chiamata
malformata finiva quindi per avere un risultato in coda che risponde a un `tool_call_id` che il
messaggio `assistant` precedente non nomina più: un `tool` che risponde al nulla, dal punto di
vista del template. Corretto rendendo *tutte* le chiamate nel messaggio `assistant`, valide o no:
quella malformata con il suo `id` e `name` esatti — gli unici due campi che il modello ha davvero
scritto in un modo che si può ancora identificare — e un oggetto vuoto al posto di argomenti che
non erano JSON (non c'è un modo fedele di renderli: se lo fossero, non sarebbero malformati). Non
è la soluzione "grossa" della sezione 3.5 punto 3 (portare i byte grezzi dentro `ToolCall`, 24
punti di costruzione in tutto il workspace): è la correzione minima che chiude la divergenza reale
— l'`id` mancante — senza pretendere una fedeltà byte-per-byte che il server ri-normalizza comunque
dal suo lato (§3.3).

**Punto 1 — il ragionamento non rimandato** (commit `30f832b`). Qui il lavoro è stato soprattutto
di lettura, non di scrittura, e la conclusione dello studio vale più del codice che ne è seguito.

### 7.2 Che cosa dice davvero il template, e perché cambia la lettura di `supports_preserve_reasoning`

Il file confrontato è `tokenizer.chat_template` estratto dal GGUF vero che gira sul banco
(`Qwen_Qwen3.6-35B-A3B-Q4_K_M.gguf`, letto con `gguf-py`) contro `qwen3.6-tollerante.jinja`: sono
**byte per byte identici** a parte la tolleranza sul secondo messaggio di sistema che Aethera ci ha
aggiunto (M-09). La parte che riguarda il ragionamento non è stata toccata da quella modifica: è
quella che il modello Qwen porta di suo.

Il template decide se rendere il blocco `<think>…</think>` di un turno `assistant` **precedente**
con questa regola (semplificata):

```
(preserve_thinking is defined and preserve_thinking is true) or (loop.index0 > ns.last_query_index)
```

`ns.last_query_index` è l'indice dell'**ultimo messaggio `user` vero** nella conversazione (non un
risultato di strumento travestito da `user`). La prima metà della condizione è un interruttore
esplicito che Nonio non manda mai. La seconda è quella che conta per noi: è vera per **ogni turno
che viene dopo l'ultimo messaggio utente**.

Le sessioni di Nonio sono fatte esattamente così: un solo messaggio di compito all'inizio, poi solo
turni `assistant` e `tool`, senza altri messaggi `user` in mezzo (a parte, occasionalmente, l'avviso
del tetto d'uscita o quello della revisione — vedi sotto). Quindi, per la stragrande maggioranza dei
turni di una sessione tipica, `loop.index0 > ns.last_query_index` è **già vera indipendentemente da
tutto il resto**: il template renderebbe il `reasoning_content` di un turno precedente se glielo
mandassimo, senza bisogno di nessun interruttore.

Questo è il punto che cambia la lettura della capacità misurata. `supports_preserve_reasoning:
false` (misurata su b10809 con questo stesso modello, riportata in `evidenze-esterne.md` §1.2b
insieme all'help di `llama-server`: il flag `--reasoning-preserve` è acceso di default, «compatible
with certain templates having `supports_preserve_reasoning` capability» — cioè non è un flag
dimenticato, è il template a rispondere «per questa domanda, no») risponde a una domanda **più
stretta** di quella che serve a Nonio: se il template tiene il ragionamento attraverso un vero
confine fra turni utente, **diversi messaggi indietro nella storia** — lo si vede nel modo in cui
llama.cpp costruisce il proprio banco di prova (`common/jinja/caps.cpp`, il caso «preserve
reasoning»): mette un turno `assistant` con `reasoning_content` **due messaggi utente prima
dell'ultimo**, e controlla se quel testo sopravvive. Per sopravvivere a quella prova, il template
*deve* prendere la prima metà della condizione (l'interruttore esplicito, che richiede un
`chat_template_kwargs` che Nonio non manda). La forma delle sessioni di Nonio — un compito solo,
mai un secondo messaggio utente vero — non ha bisogno di quell'interruttore: le basta la seconda
metà della condizione, che è già vera quasi sempre.

Questo spiega perché finora si teneva `thinking = false` su questi modelli (decisione datata,
citata in `evidenze-esterne.md`): la capacità misurata diceva di no, e RF-27 dice di fidarsi delle
capacità rilevate, non di configurarle. Ma quel «no» era la risposta a una domanda che le sessioni
di Nonio non fanno. Con `thinking` acceso e `send_reasoning` spento (lo stato di oggi, prima di
questo lavoro), il prezzo si pagava comunque: il modello ragionava, il template renderebbe un
blocco vuoto per il turno prima (perché `reasoning_content` non arriva mai mandato), e quel blocco
vuoto diverge dal blocco pieno che il motore ha davvero generato — la rottura che il rapporto
descrive fin dall'inizio. `send_reasoning` non aggira `supports_preserve_reasoning`: gli dà la
possibilità di essere vero anche quando il motore, rispondendo a una domanda diversa, ha detto di
no.

**Il rischio dei byte, verificato e non solo temuto.** `--reasoning-format deepseek`
(`common_chat_peg_mapper::map`, `chat-peg-parser.cpp` del sorgente locale, build b10991)
restituisce `reasoning_content` come il testo grezzo trovato fra `<think>` e `</think>`, senza
tagliarlo — l'unica eccezione è azzerare un blocco fatto di soli spazi. Quindi quello che Nonio
conserva è quello che il modello ha scritto, non un riassunto. Il template, però, lo trimma di
nuovo e lo re-incapsula nella propria forma fissa (`<think>\n…\n</think>\n\n`): il rimando torna
identico ai byte generati solo se gli spazi del modello ai due bordi del blocco coincidevano già
con quella forma. Quando non coincidono, la divergenza resta **nello stesso punto** in cui diverge
già oggi (il primo carattere dopo `<think>\n`, dove oggi c'è un blocco vuoto): non può quindi
peggiorare rispetto a spegnere il rimando, solo lasciare le cose come stanno o chiudere la
divergenza per il resto del blocco. Non verificato su famiglie diverse da Qwen (gpt-oss, generic):
un campo che un template non nomina è inerte in Jinja per come sono scritti gli altri template
letti in questo lavoro, ma è una lettura, non una misura fatta motore alla mano.

### 7.3 L'opzione: `[family] send_reasoning`

Default assente (si legge come spento), non ancora acceso in nessun profilo spedito. Filo:
`FamilyConfig::send_reasoning` (profilo TOML) → `EffectiveConfig::send_reasoning` →
`RunOptions::send_reasoning` → `RenderRequest::send_reasoning` → `openai::messages`, che ora manda
`reasoning_content` quando la capacità rilevata lo dice **o** quando l'opzione lo chiede. Tre test
nuovi: uno in `profile.rs` (il campo si legge dal TOML ed è opzionale, non rompe i profili già
spediti), uno in `qwen.rs` (l'opzione manda `reasoning_content` anche con
`supports_preserve_reasoning: No` dichiarato esplicitamente), uno end-to-end in `compose.rs` (dal
profilo TOML fino al payload renderizzato, passando per tutta la catena).

### 7.4 Il progetto grosso: salvataggio e ripristino dello slot

**Non implementato**, come previsto dall'istruzione originale se l'implementazione risultava
grossa. Resta al livello di progetto scritto in §3.5 punto 1 del rapporto precedente: `POST
/slots/{id}?action=save` a fine sessione, `restore` prima del primo turno di una ripresa, `id_slot`
fissato per tutta la sessione. Motivo per cui non è stato fatto qui, oltre alla dimensione: tocca
un'area che i tre punti sopra non toccano (le chiamate a `/slots`, oggi usate solo per sapere se
esistono — `nonio-backend/src/llamacpp.rs:124-134` — mai per salvare o ripristinare uno stato), non
ha test possibili senza un motore acceso per davvero (il vincolo di questa sessione era di non
avviarne uno), e il progetto scritto già elenca le tre decisioni che servirebbero prima di scrivere
codice: il formato dell'impronta motore+modello per rifiutare un ripristino contro l'engine
sbagliato (RF-10 la prevede ma non la implementa ancora), che cosa fare quando il ripristino fallisce
(ricadere sul prefill dicendolo, o rifiutare), e se il file dello stato — che qui pesa quanto lo
stato *intero* del motore, non solo il ricorrente — è accettabile sul disco che ospita le sessioni.
Nessuna di queste tre è stata decisa in questo lavoro.

### 7.5 Che cosa deve misurare la sessione principale

**Per il punto 2 (turno tagliato).** Sulla batteria di coding, stesso profilo e stesso numero di
giri usati per il confronto con `main`: contare quante volte un turno tagliato al tetto dell'uscita
è seguito, nel turno successivo, da un ricalcolo completo invece che da un proseguimento dalla
cache — cioè `cache_n` del turno N+1 confrontato con la lunghezza del prompt fino al turno N incluso.
Prima di questo lavoro quella divergenza è garantita ogni volta (il testo generato viene sostituito
da un avviso). Dopo, per i turni che *non* contenevano ragionamento (thinking spento, o risposta
breve), `cache_n` dovrebbe salire fino a coprire anche il turno tagliato — il numero da guardare è
la quota di turni-dopo-un-taglio il cui `cache_n` è "il prompt intero meno la sola parte nuova",
invece di "molto meno di quello". Deve salire, non restare uguale.

**Per il punto 3 (chiamate malformate).** Con FC, dove il rapporto misura 121 chiamate respinte su
186: contare, sulla richiesta immediatamente successiva a una chiamata malformata, se `cache_n`
copre anche il turno con la chiamata malformata o si ferma prima di lui. Prima di questo lavoro la
seconda cosa è garantita; dopo, dovrebbe diventare la prima. Un numero solo, non una batteria
intera, basta a confermarlo: la traccia già registra `tool_called` (assente per le malformate, per
costruzione — nota in `resume.rs`) e `tool_result`, quindi il turno da guardare si trova cercando un
`ToolResult` con `outcome.error.kind == "invalid_arguments"` e leggendo il `cache_n` del turno
seguente.

**Per il punto 1 (ragionamento), la misura che conta di più.** Un giro della batteria di coding,
stesso profilo G1 (`qwen3.6-35b-a3b.q4_k_m.vulkan`), con **due impostazioni** da confrontare sugli
stessi compiti:

1. `thinking = true`, `send_reasoning` assente (spento) — lo stato di oggi;
2. `thinking = true`, `send_reasoning = true` — lo stato con l'opzione accesa.

Il numero da guardare, turno per turno, è `cache_n` (token riusati) confrontato con la lunghezza del
prompt fino a quel turno, esattamente come RF-7 chiede di leggerlo (mai stimato, solo quello che il
motore dichiara). **La direzione attesa**: nel caso 1, ogni turno in cui il turno *precedente*
conteneva ragionamento dovrebbe mostrare `cache_n` molto più basso della lunghezza del prompt fino
al turno precedente incluso — la divergenza dentro il generato di cui parla `M-17 T-03`. Nel caso 2,
per gli stessi turni, `cache_n` dovrebbe avvicinarsi alla lunghezza intera del prompt fino al turno
precedente (riuso quasi completo), con un margine di errore dato esattamente dal rischio dei byte
di §7.2 — se il margine è grande e sistematico (non occasionale), è la prova sperimentale che gli
spazi del modello ai bordi del blocco non seguono la forma che il template si aspetta, e va scritta
come tale. **Il compito su cui farla**: uno solo che ragiona per più turni di fila (non l'intera
batteria, perché il caso 2 rischia — se il rimando peggiorasse, cosa che l'analisi di §7.2 esclude
ma non misura — di pagare una ri-elaborazione per ogni turno). Il segnale che chiude la domanda:
se il numero di turni "quasi completamente riusati" sale dal caso 1 al caso 2, il punto 1 è
confermato utile sui modelli che Nonio guida oggi; se resta uguale, il template si comporta
diversamente da come è stato letto qui e va riletto.

### 7.6 Commit, uno per uno

Ramo `aethera/m18-riuso`, staccato da `main` (`608a67a`). Tutti locali.

| hash | che cosa |
| --- | --- |
| `c031c90` | il turno tagliato al tetto resta come proprio turno assistant invece di essere sostituito dall'avviso; ogni chiamata (valida o malformata) entra nel messaggio assistant che la precede; due test aggiornati, uno nuovo |
| `3d306e3` | `nonio resume` ricostruisce lo stesso turno tagliato che `execute` ora lascia in coda, invece della forma vecchia (solo l'avviso); un test aggiornato |
| `30f832b` | `[family] send_reasoning`: manda `reasoning_content` anche quando la capacità rilevata dice no, dietro un'opzione di profilo spenta di default; tre test nuovi |
| `4d51ba1` | corretto un indice di test (`messages()[0]` → `[1]`) rimasto dalla forma vecchia del turno tagliato, trovato dalla compilazione dell'operatore |

La modifica non committata dell'operatore ad `AGENTS.md` è rimasta dov'era: non toccata, non messa
in commit, nessuno stash.
