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
  due avvii e si vede che cosa è cambiato fra i due, nel profilo e nelle condizioni.

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
| `runs/<id>/` | per ogni avvio: `manifest.toml` (com'è stato acceso), `server.log`, telemetria e slot salvati |

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

Con il motore acceso, Impostazioni mostra le righe già pronte da incollare: il `profile.toml` di
Nonio, il blocco d'ambiente per i banchi, la versione PowerShell. Portano l'indirizzo, l'alias, il
contesto **servito** (letto da `/props`, non quello dichiarato) e il campionamento consigliato del
modello con la fonte e la data in cui è stato letto.

Aethera espone anche un endpoint locale su `127.0.0.1:8090` per chi vuole coordinarsi da solo:

| | | |
| --- | --- | --- |
| `GET` | `/status` | stato, alias, porta, id dell'avvio, se è in uso e perché |
| `GET` | `/run` | il manifest dell'avvio corrente |
| `POST` | `/lock` | «sto lavorando»: finché il lock vale, arresto e riavvio sono rifiutati |
| `DELETE` | `/lock` | rilascia |
| `GET` | `/telemetry/recent` | prefill, decode, accettazione e quota di cache delle ultime richieste |

Il lock non cambia il motore: rende solo rifiutati arresto e riavvio. Le richieste che arrivano da un
browser (con `Origin`) sono rifiutate.

## Che cosa non fa

- **Non lancia banchi** e non misura la qualità dei modelli.
- **Non aggiorna niente da solo**: né la build, né i pesi, né sé stessa.
- **Non avvia più motori insieme**: nella v1 se ne accende uno alla volta.
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
cd src-tauri && cargo run --example e2e_m05 -- C:\AetheraData
```

`e2e_m03` e `e2e_m04` provano memoria e catalogo, `e2e_m05` il percorso da una radice vuota a un
motore acceso, `e2e_m06` il comportamento quando qualcosa va storto.
