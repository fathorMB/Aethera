# Note di rilascio

## 0.1.0 — la prima versione utilizzabile

Aethera accende e spegne un `llama-server` e tiene il conto di com'è andata. Questa è la prima
versione che si installa e si usa dall'inizio alla fine senza aprire un editor di testo.

### Che cosa c'è dentro

**Il motore.** Un profilo TOML descrive un avvio senza percorsi assoluti: pesi per nome file, build
e backend fissati, ogni leva esplicita. Aethera ne ricava la riga di comando, la mostra prima di
usarla, avvia il processo e segue il suo stato — spento, in caricamento, pronto, uscito con errore —
dalla finestra e dalla tray. Chiudere la finestra con il motore acceso lo riduce nella tray: un
motore non si ferma chiudendo una finestra.

**La memoria, misurata.** Prima dell'avvio: la stima al contesto scelto, con i pesi dal file, la
cache KV calcolata sui soli blocchi ad attenzione piena e il buffer di calcolo **misurato sugli
avvii passati**, non ricavato da una formula. Dopo il caricamento: VRAM dedicata e condivisa,
working set, RAM rimasta, doppia copia dei pesi, margine rispettato o no. Stima e misura restano una
accanto all'altra nel manifest dell'avvio.

**Lo stato «in uso».** Slot attivi su `/slots`, richieste negli ultimi 30 secondi, un lock dichiarato
sull'endpoint o la protezione manuale: finché una di queste vale, arresto e riavvio sono rifiutati
dalla finestra, dalla tray e dall'endpoint. Un client che sta lavorando non si vede staccare il
motore sotto le mani.

**Il catalogo.** Modelli e build presenti sulla macchina, con lo stato (scaricabile, in download,
presente da verificare, verificato, hash diverso). SHA-256 confrontato con l'oid LFS di Hugging Face
o con il digest di GitHub; metadati letti dal GGUF senza caricarlo; download riprendibili; build
`ggml-org` scaricate e verificate prima di essere estratte. Un file già presente viene riconosciuto
anche quando è un hard link creato da un altro strumento, e non viene mai cancellato senza dirlo.

**Lo storico.** Ogni avvio lascia il suo manifest e la sua telemetria; la pagina Benchmark li elenca
con le condizioni e ne confronta due, mostrando che cosa è cambiato nel profilo e nella macchina.
Aethera non lancia banchi: prepara il motore su cui girano.

**Uso quotidiano senza toccare i file.** Un profilo nuovo si scrive dalla finestra, con valori
sensati sul modello scelto; si duplica, si rinomina e si cancella. Un `.gguf` che sta fuori dalla
cartella dei pesi si registra collegandolo (hard link) invece di copiarlo. Il campionamento
consigliato si legge dalla model card del publisher — seguendo `base_model` quando il repository è
una riquantizzazione — e arriva fino alle righe da incollare nei client, con la fonte e la data.

**Quando qualcosa va storto.** Ogni guasto prevedibile dà una frase che dice cosa è successo e cosa
fare: file di configurazione illeggibili (mostrati e mai sovrascritti), pesi rinominati (riconosciuti
e ricollegabili), disco pieno e rete caduta a metà download (il `.part` resta e la ripresa riparte
dal punto giusto, anche dopo aver chiuso l'app), motore che esce con errore (le righe di log che lo
spiegano, messe in evidenza), chiusura con un lavoro in corso (si chiede, e si aspetta che i file
vengano chiusi), radice dati su un disco che non risponde.

### Che cosa serve

Windows 10 o 11 a 64 bit con WebView2, una build di `llama.cpp` e un modello GGUF. Né Rust né Node:
quelli servono solo per compilare.

### Che cosa non fa ancora

- Un motore alla volta.
- Nessun aggiornamento automatico: né della build, né dei pesi, né dell'app.
- Nessuna esecuzione di banchi e nessuna misura di qualità dei modelli.
- Solo Windows: le misure di memoria della GPU e il riconoscimento dei processi usano contatori di
  Windows.

### Limiti noti di questa versione

- L'installatore è **senza firma digitale**: SmartScreen avvisa alla prima esecuzione.
- L'endpoint `127.0.0.1:8090` non ha autenticazione. Ascolta solo in locale e rifiuta le richieste
  che arrivano da un browser, ma chiunque possa eseguire codice su questo computer può usarlo.
- La lettura della model card riconosce le forme più comuni; un publisher che scrive il
  campionamento in un modo inconsueto non viene letto, e lo dichiara invece di inventare valori.

## Installatore non firmato e Windows SmartScreen

Questa sezione vale per ogni versione e il workflow di release la aggiunge alle note di ciascuna.

L'installatore (`Aethera_X.Y.Z_x64-setup.exe`) **non ha firma digitale**, per scelta: la firma resta
fuori finché non serve. Accanto all'installatore, nella release, c'è il file `.sha256`: prima di
eseguirlo si controlla che l'hash coincida, per esempio in PowerShell con
`Get-FileHash .\Aethera_X.Y.Z_x64-setup.exe -Algorithm SHA256`.

Alla prima esecuzione Windows SmartScreen mostra la schermata blu **«Windows ha protetto il PC»**,
con l'editore «Sconosciuto». Per andare avanti:

1. fare clic su **«Ulteriori informazioni»**;
2. controllare che il nome del file sia quello scaricato dalla release;
3. fare clic su **«Esegui comunque»**.

L'avviso non dice che il file è dannoso: dice che non è firmato e che Windows non lo ha ancora visto
abbastanza volte.
