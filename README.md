# Aethera

Aethera accende e spegne un `llama-server`, e tiene il conto di com'è andata.

È un launcher da scrivania per Windows: sceglie i pesi e la build, scrive la riga di comando per
intero, avvia il processo, misura quanta memoria occupa davvero dopo il caricamento, si accorge se
qualcuno lo sta usando e registra ogni avvio con le sue condizioni. Niente di quello che mostra è
stimato dopo il fatto: o è misurato, o è dichiarato «sconosciuto».

**Non è un banco.** Aethera non manda richieste per misurare la qualità di un modello e non stila
classifiche: serve il motore su cui i banchi girano, e dà loro l'indirizzo, l'alias e il contesto
davvero servito. Chi lancia le prove è un altro programma.

## A che cosa serve

- **Un profilo per ogni modo di accendere lo stesso modello.** Contesto, tipi di cache, speculazione,
  slot, porta: tutto esplicito in un file TOML, niente lasciato al default del motore. La riga di
  comando che ne esce racconta per intero com'è stato avviato, e due profili si confrontano leggendo
  due file invece di ricordarsi due sere diverse.
- **La memoria che occupa davvero.** Prima dell'avvio Aethera stima quanto serve (pesi + cache KV al
  contesto scelto + buffer di calcolo, questo misurato sugli avvii passati e non inventato con una
  formula); dopo il caricamento misura VRAM dedicata e condivisa, working set e RAM rimasta, e mette
  stima e misura una accanto all'altra.
- **Lo stato «in uso».** Un client che sta lavorando non si vede interrompere il motore sotto le
  mani: finché ci sono slot attivi, richieste recenti, un lock dichiarato sull'endpoint o la
  protezione manuale, arresto e riavvio vengono rifiutati — dalla finestra, dalla tray e
  dall'endpoint.
- **Un catalogo di pesi e build verificati.** SHA-256 confrontato con l'oid LFS di Hugging Face o con
  il digest di GitHub, metadati letti dal GGUF senza caricarlo, download riprendibili, campionamento
  consigliato letto dalla model card con la fonte e la data.
- **Uno storico degli avvii.** Ogni avvio lascia il suo manifest e la sua telemetria: si confrontano
  due avvii e si vede che cosa è cambiato fra i due, nel profilo e nelle condizioni — build, driver di
  GPU e NPU, profilo di alimentazione, VGM, disco dei pesi. Due avvii con un driver diverso non si
  confrontano senza che la pagina lo dica.
- **Quanto un client costa al motore.** Richiesta per richiesta, quanta parte del prompt il motore ha
  riusato e quanta ha dovuto rielaborare, e quando un client ha compattato la conversazione e quanto
  tempo è costato. Su questa macchina pesa più di qualunque leva del motore.

## Che cosa serve per usarla

- **Windows 10 o 11 a 64 bit.** Le misure di memoria della GPU e il riconoscimento dei processi
  usano contatori di Windows.
- **WebView2**, che su Windows 11 c'è già e su Windows 10 aggiornato quasi sempre anche.
- **Una build di `llama.cpp`** (le release `ggml-org/llama.cpp`, per esempio `b10809`,
  `win-vulkan-x64`). Aethera può scaricarla e verificarla da sola, oppure usare una cartella che hai
  già.
- **Un modello in formato GGUF.** Anche questo Aethera può scaricarlo, o riconoscere quello che hai
  già sul disco.

Per **usare** Aethera non servono né Rust né Node: si installa e basta. Servono solo per
compilarla — vedi [Compilare](#compilare).

## Primo avvio

Alla prima apertura Aethera chiede una cosa sola: **dove tenere i suoi dati**. Da lì in poi la
pagina Motore mostra i passi che mancano, in ordine, con il prossimo in evidenza:

1. **Radice dati** — la cartella scelta all'inizio.
2. **Cartella dei pesi** — dove tieni i `.gguf`. I profili citano i pesi per nome file: la cartella
   è della macchina, non del profilo, così un profilo si può passare a un'altra macchina.
3. **Una build di llama.cpp** — scaricata dal Catalogo o dichiarata da una cartella che hai già.
4. **Un modello** — aggiunto da Hugging Face e scaricato, oppure registrato da un `.gguf` che hai
   già altrove sul disco. Se sta sullo stesso volume della cartella dei pesi viene **collegato**
   (hard link) e non copiato: non occupa un byte in più.
5. **Un profilo** — «Nuovo profilo…» ne scrive uno con valori sensati sul modello scelto.
6. **Il primo avvio** — da qui in poi la pagina Motore misura quello che succede.

Nessuno di questi passi richiede di aprire un editor di testo.

## Dove finiscono i dati

**Nella radice dati** che hai scelto (per esempio `D:\Aethera`):

| | |
| --- | --- |
| `machine.toml` | nome della macchina, cartella dei pesi, build dichiarate, margine di RAM |
| `catalog.toml` | i modelli conosciuti: repository, hash atteso e calcolato, metadati GGUF letti una volta, campionamento consigliato |
| `profiles/*.toml` | un file per profilo; il nome del file è il nome del profilo **ed è l'alias servito** |
| `builds/` | le build scaricate da Aethera (quelle dichiarate a mano restano dove sono) |
| `templates/` | template di chat che un profilo può passare al motore; Aethera ci scrive `qwen3.6-tollerante.jinja` se manca, e non lo sovrascrive se lo cambi |
| `runs/<id>/` | per ogni avvio: `manifest.toml` (com'è stato acceso e in che condizioni), `server.log`, telemetria e slot salvati |

**Fuori dalla radice dati**, in `%APPDATA%\Aethera\settings.toml`, restano solo due cose: dove sta la
radice dati e cosa fare all'uscita. Sono preferenze di questo computer, non del progetto.

I **pesi** non stanno nella radice dati: stanno dove dici tu in `machine.toml`. Spostare la radice
non sposta i modelli, e cancellarla non li tocca.

## Come si cambia build

La build è **fissata nel profilo** (`runtime.build` e `runtime.backend`): non si aggiorna da sola,
perché cambiarla cambia i risultati e deve restare una variabile dichiarata di una prova.

Per provarne un'altra: Catalogo → scheda **Build llama.cpp** → «Cerca le release di ggml-org» →
Installa. Poi duplica il profilo, cambia `runtime.build` nella copia e avvia quella: lo storico degli
avvii terrà le due righe una accanto all'altra, con build e commit di ognuna.

Una cartella già scaricata a mano si dichiara con «Importa cartella…» nella stessa scheda (o da
Impostazioni → «Aggiungi build…»): l'id si ricava dal nome della cartella, `b10809-win-vulkan-x64`.

## Per i client

Con il motore acceso, Impostazioni mostra le righe già pronte da incollare, una scheda per client: il
`profile.toml` di Nonio, l'`opencode.json` di OpenCode, le variabili per Claude Code (PowerShell o
bash) e il blocco d'ambiente per i banchi. Portano l'indirizzo, l'alias, il contesto **servito**
(letto da `/props`, non quello dichiarato) e il campionamento consigliato del modello con la fonte e
la data in cui è stato letto. Accanto, il **budget di contesto** di ogni client: quanto spazio gli
resta per lavorare dopo il suo prompt fisso e l'output riservato.

**Claude Code con Qwen3.6.** Claude Code manda messaggi di sistema anche a metà conversazione, e il
template di chat di Qwen3.6 li rifiuta con un errore 500. Il profilo lo risolve con
`chat_template_file = "qwen3.6-tollerante.jinja"`: è il template del modello con quella sola riga
cambiata. La scheda Claude Code dice se l'avvio acceso lo usa.

Aethera espone anche un endpoint locale su `127.0.0.1:8090` per chi vuole coordinarsi da solo:

| | | |
| --- | --- | --- |
| `GET` | `/status` | stato, alias, porta, id dell'avvio, se è in uso e perché |
| `GET` | `/run` | il manifest dell'avvio corrente |
| `POST` | `/lock` | «sto lavorando»: finché il lock vale, arresto e riavvio sono rifiutati |
| `DELETE` | `/lock` | rilascia |
| `GET` | `/telemetry/recent` | prefill, decode, accettazione e quota di cache delle ultime richieste, ognuna con come ha trattato la conversazione, e le compattazioni |
| `GET` | `/services` | i motori di servizio accesi: che cosa servono, porta, VRAM misurata, stato |

Il lock non cambia il motore: rende solo rifiutati arresto e riavvio. È anche l'unico modo in cui
Aethera sa **quale** client ha mandato una richiesta: il log del motore non lo dice. Le richieste che
arrivano da un browser (con `Origin`) sono rifiutate.

## I motori di servizio

Un `llama-server` serve un modello solo. Chi ha bisogno di **embedding** e **rerank** — per
esempio GalaxyCenter, che li pretende e dichiara che i server li gestisce l'operatore — con un
motore solo non è servito. Da qui i **motori di servizio**: `llama-server` piccoli accesi accanto
al principale, ognuno sulla sua porta, che si accendono e si spengono dalla pagina Motore.

**Non servono ad andare più veloce.** Su questa macchina un modello più piccolo non è più veloce
del grosso: un denso da 8B fa 16,0 tok/s di decode contro i 24,3 del MoE da 35B, perché il decode
è limitato dalla banda e il MoE legge meno byte per token. Un processo separato serve a **non
toccare la cache del prefisso** del motore principale, che è tutto-o-niente: infilare un'altra
conversazione nel suo unico slot farebbe ripartire da zero il client che sta lavorando.

Un servizio non è un motore in piccolo, ed è voluto:

- **non lascia manifest né telemetria** e non compare in Benchmark: non è il soggetto di una
  misura, o risponde o no;
- **non rende il motore principale «in uso»** e non ne blocca arresto o riavvio. Se un embedding
  notturno impedisse di riavviare il motore, avremmo scambiato il servo col padrone;
- **il suo profilo ha meno leve, non una in più.** Niente speculazione, checkpoint, budget del
  client o salvataggio degli slot: un file che le contiene viene rifiutato dicendo quale campo non
  appartiene lì. Si riconosce da `role = "service"`.

La memoria di ogni servizio è **misurata**, per processo, dai contatori di Windows.
`llama-server --list-devices` a questo non serve: riporta il budget dell'heap, e su questa
macchina dice lo stesso «free» a motore spento e a motore carico.

Il **reranker** viene messo alla prova, non solo avviato: i GGUF girati dalla comunità sono spesso
convertiti male e danno punteggi vicini a zero anche al documento giusto. All'avvio Aethera fa un
test di sanità vero e, se fallisce, lo dice invece di lasciare che se ne accorga l'indice.

## Che cosa non fa

- **Non lancia banchi** e non misura la qualità dei modelli.
- **Non aggiorna niente da solo**: né la build, né i pesi, né sé stessa.
- **Non accende più di un motore principale alla volta**: quello sì, resta uno. Accanto però
  possono stare i **motori di servizio**, che sono un'altra cosa (sotto).
- **Non tocca i file che non ha scritto lei**: un `catalog.toml` o un `machine.toml` che non si
  lasciano leggere vengono mostrati e lasciati dov'erano, mai sovrascritti in silenzio.
- **Non serve fuori da questo computer**: l'endpoint ascolta solo su `127.0.0.1`.

## Compilare

Servono [Rust](https://rustup.rs) stabile e [Node](https://nodejs.org) 20 o più recente.

```bash
npm install
npm run tauri build
```

L'installatore NSIS esce in `src-tauri/target/release/bundle/nsis/`.

Per lavorarci sopra: `npm run tauri dev`.

Le prove:

```bash
cd src-tauri && cargo test
```

e le prove sulla macchina vera, che vogliono pesi e build veri e una radice dati esistente da cui
prendere i percorsi (creano la loro radice temporanea e la cancellano):

```bash
cd src-tauri && cargo run --example e2e_m05 -- <radice>
```

`e2e_m03` e `e2e_m04` provano memoria e catalogo, `e2e_m05` il percorso da una radice vuota a un
motore acceso, `e2e_m06` il comportamento quando qualcosa va storto.

## Percorsi nei documenti e negli script dei banchi

Il kit in `.lmbrain-lite/` (log, rapporti, mockup) non riporta i percorsi della macchina su cui è
stato scritto: al loro posto ci sono dei segnaposto.

| segnaposto | che cosa indica | variabile negli script |
|---|---|---|
| `<radice>` | la radice dati di Aethera | `AETHERA_RADICE` |
| `<repo>` | il checkout di questo repository | `AETHERA_REPO` |
| `<pesi>` | la cartella dei pesi (`models_dir`) | `AETHERA_PESI` |
| `<nonio>` | la build di llama.cpp usata dai banchi, o Nonio | `AETHERA_BUILD`, `AETHERA_NONIO_EXE` |
| `<download>` | dove finiscono i download grossi | `AETHERA_DOWNLOAD` |
| `<minis-config>` | il checkout di minis-config | `AETHERA_G1_JSON` (profilo G1) |

Gli script in `.lmbrain-lite/m08`, `m10` e `m11` leggono i percorsi da
`.lmbrain-lite/percorsi.local.sh`, che non si committa: si copia `percorsi.esempio.sh` e si
mettono i valori della propria macchina. I test usano percorsi fittizi (`X:\…`).

## Come si fa una release

L'installatore lo produce GitHub, non la macchina di chi rilascia: un tag `vX.Y.Z` avvia
`.github/workflows/release.yml`, che crea una release **in bozza** con l'installatore NSIS, il suo
`.sha256` e le note. Ogni pull request e ogni push su `main` passano invece da
`.github/workflows/ci.yml` (build del frontend, vitest, `cargo test`, `cargo clippy`; mai gli esempi
che vogliono il motore o la GPU).

1. **La versione, uguale in quattro posti:** `src-tauri/tauri.conf.json` (`version`),
   `src-tauri/Cargo.toml` (`[package] version`), `package.json` (`version`) e `src-tauri/Cargo.lock`,
   che si aggiorna da solo con `cargo check` in `src-tauri`. Il titolo della finestra oggi non
   contiene la versione; se un giorno la contiene, va cambiato anche lì. Il controllo si prova in
   locale:

   ```bash
   python .github/scripts/controlla_versione.py --tag vX.Y.Z
   ```

2. **Le note:** in `RELEASE-NOTES.md` una sezione `## X.Y.Z — …` in cima. Diventa il testo della
   release, con in coda la sezione «Installatore non firmato». Senza la sezione della versione il
   workflow usa tutto il file e lo segnala con un avviso.
3. **Commit su `main` e push**, poi il tag annotato e il suo push:

   ```bash
   git tag -a vX.Y.Z -m "Aethera X.Y.Z"
   git push origin vX.Y.Z
   ```

   Se il tag e i file non dicono la stessa versione il workflow si ferma prima della build e dice
   quale file correggere: si cancella il tag (`git push origin :refs/tags/vX.Y.Z` e
   `git tag -d vX.Y.Z`), si corregge, si rimette.
4. **Si guarda la bozza** nella pagina Releases del repository: installatore presente, `.sha256`
   presente, note giuste. Meglio ancora scaricarlo, controllare l'hash e installarlo.
5. **Si pubblica a mano** con «Publish release». Il workflow non pubblica mai da solo.

**Tag di prova.** Un tag `vX.Y.Z-rc.N` è accettato con i file ancora a `X.Y.Z`: la release in bozza
esce marcata come pre-release e le note dicono che è una prova. Serve a provare il workflow senza
toccare la versione. Altri suffissi (`-beta`, `-alpha`…) sono rifiutati.

**Tempi.** Il primo run su un tag compila Rust in release da zero: la cache di un tag vede solo
quella di `main`, che però è in modalità debug. Contare una ventina di minuti per il job Windows.
