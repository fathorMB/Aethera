# Note di rilascio

## Non ancora rilasciata

### Motori di servizio: Aethera ne accende più di uno

Accanto al motore principale Aethera può tenere accesi uno o più `llama-server` piccoli, ognuno
sulla sua porta: embedding, rerank, o una chat leggera per lavori che non sono codice. Si vedono
nella pagina Motore, con la porta, la VRAM misurata e lo stato, e si accendono e spengono da lì.

**A che cosa serve.** Un `llama-server` serve un modello solo. Chi ha bisogno di embedding e
rerank — GalaxyCenter, per esempio, che li pretende e dichiara che i server li gestisce
l'operatore — con un motore solo non era servito. Impostazioni › Client ha una scheda
**GalaxyCenter** con il `settings.toml` già pronto, che compare quando i servizi sono accesi
davvero.

**Non è per andare più veloce.** Su questa macchina un modello più piccolo non è più veloce del
grosso: un denso da 8B fa 16,0 tok/s di decode contro i 24,3 del MoE da 35B, perché il decode è
limitato dalla banda e il MoE legge meno byte per token. Un processo separato serve a **non
toccare la cache del prefisso** del motore principale, che è tutto-o-niente: infilare un'altra
conversazione nel suo unico slot farebbe ripartire da zero il client che sta lavorando.

**Un servizio non è un motore in piccolo.** Non lascia manifest né telemetria, non compare in
Benchmark e soprattutto **non rende il motore principale «in uso»**: non ne blocca arresto né
riavvio. Il suo profilo ha meno leve, non una in più — niente speculazione, checkpoint, budget del
client o salvataggio degli slot — e un file che le contiene viene rifiutato dicendo quale campo
non appartiene lì.

**Il reranker viene messo alla prova, non solo avviato.** I GGUF del reranker girati dalla
comunità sono spesso convertiti male: rispondono e poi danno punteggi vicini a zero anche al
documento giusto. All'avvio Aethera fa un test di sanità vero e, se fallisce, lo scrive.

Un'installazione nuova nasce con il profilo di servizio dell'embedding
(`qwen3-embedding-0.6b.q8`), seminato con la stessa regola del profilo principale: solo se il file
non c'è già. I pesi (610 MiB) restano da scaricare e l'app lo dice.

La memoria dei servizi è misurata per processo dai contatori di Windows. `llama-server
--list-devices` non serve a questo: su questa macchina riporta lo stesso «free» a motore spento e
a motore carico, perché è il budget dell'heap e non vede le allocazioni.

### Un profilo solo, e l'app nasce con quello

Aethera scrive `qwen3.6-35b-a3b.q8_0.vulkan.toml` in `profiles/` alla prima esecuzione, come già
faceva con i template: un'installazione nuova è subito capace di accendere il motore invece di
partire con la cartella vuota. Un profilo modificato a mano non viene toccato; cancellarlo lo fa
ricomparire al prossimo avvio. I pesi (37,8 GB) e la build di llama.cpp restano da scaricare, e
l'app lo dice.

È la configurazione misurata come migliore su questa macchina — Qwen3.6-35B-A3B Q8_0, contesto
262144, `-ub 4096 -b 4096`, MTP con bozza 3, checkpoint del server spenti, ragionamento spento dal
client — e le sue note portano dentro il perché di ogni leva e la baseline del prodotto:
16 compiti su 16, 42,8 compiti riusciti per ora, riuso del prefisso 90,3%, prefill 1,61 s per
richiesta. Il dettaglio è in `.lmbrain-lite/reports/baseline-prodotto-2026-09.md`; le sessioni
vere si confrontano con quei numeri con `python .lmbrain-lite/batteria/baseline.py --ultime 5`.

Il contesto 262144 chiede 64 GB di memoria assegnata alla GPU: a VGM 48 il tetto misurato è
131072.

## 0.2.0 — la finestra v2 e il profilo principale

La finestra ridisegnata, la provenienza delle build del fork, e un profilo principale che si apre
da solo e si avvia dal tray: la versione per l'uso quotidiano su questa macchina, dove il
profilo principale è Qwen3.6-35B-A3B Q8_0 (misure in `.lmbrain-lite/design/giornata-2026-09-18`).

### La finestra v2 (M-12)

Stessi dati, disposti meglio. Nessun comando nuovo verso il motore e nessuna misura nuova: cambia
solo la finestra, secondo il mockup approvato il 16-09 (`design/aethera-v2-ui`). Unita senza la
prova dell'operatore dalla finestra, per sua decisione: la prova la fa l'uso.

- **Lo stato del motore si vede da ogni pagina.** Una striscia in alto porta stato, profilo, build
  e porta, prefill, decode e riuso, «in uso» con il motivo, la protezione e i pulsanti **Riavvia**
  e **Ferma**, con gli stessi divieti di prima: finché il motore è in uso sono rifiutati. Il piede
  della colonna dice la radice dati, l'avvio acceso e i lavori in corso.
- **Ogni pagina ha le sue azioni in testa.** In Avvio una fascia che resta in cima mentre si
  scorre porta Avvia, Scarta, Aggiorna il profilo, Salva come… e i motivi per cui l'avvio è
  bloccato.
- **Motore**: un solo riquadro di stato, una fascia di avvisi (degradato, divergenze, condizioni
  cambiate, compattazione) con il pulsante giusto accanto, quattro numeri con un giudizio
  (prefill, decode, riuso, compattazioni), la memoria come barra impilata su VGM più RAM. Riga di
  comando e log sono a scomparsa; il log si apre da solo dopo un'uscita con errore.
- **Avvio**: dodici leve essenziali con l'etichetta in italiano e il nome della leva in piccolo;
  le altre in «Avanzate», chiuse, con i verdetti di M-08. Le proposte di M-08 sono un avviso con
  «Applica». La build si sceglie fra quelle che la macchina conosce. Il nome si dà salvando.
- **Catalogo**: tabella a sei colonne con il dettaglio a destra, filtri per stato con il
  conteggio, ricerca, «Aggiungi da Hugging Face…» come pulsante, lavori in corso come fascia.
  La rimozione è un dialogo della finestra, con la cancellazione del file come scelta esplicita.
- **Benchmark**: decode e riuso con la loro scintilla nella riga, le note di stato in una colonna,
  il dettaglio a destra con il manifest a scomparsa, e una fascia di confronto con i tre Δ appena
  si spuntano due avvii.
- **Impostazioni** in tre schede: Macchina, Client, App. Le soglie stanno in un posto solo;
  l'endpoint è a scomparsa; il tema si sceglie anche da qui.
- Le spiegazioni lunghe stanno nel «?» accanto ai titoli o in una riga «come si legge»; il testo
  minimo sale a 12 px.
- **Build del fork (M-14) in Benchmark**: accanto alla build di ogni avvio la serie di patch quando
  non è quella di ggml-org (`b10991 · moro1 int8-coopmat`), nel dettaglio rami e commit, e nel
  confronto un avviso se i due avvii hanno serie diverse: il confronto misura anche la patch. Una
  serie non registrata resta «sconosciuta», mai ggml-org.
- **Provenienza delle build nelle Impostazioni**: sotto ogni build tag base, serie, rami con commit,
  data e durata della build; «build scaricata da ggml-org» senza file, «provenienza assente» se
  l'id dichiara una serie ma il file manca.
- **Profilo principale**: Avvio si apre da solo su un profilo scelto una volta, che sta in cima
  alla lista con l'etichetta «principale» (poi i profili con `gate`, poi gli altri); si fissa da
  Avvio («Rendi principale») o da Impostazioni → App. Un profilo rinominato o cancellato non fa
  sparire la scelta in silenzio: Avvio lo dice e torna al primo della lista finché non se ne
  sceglie un altro. La tray ha «Avvia `<profilo principale>`», con gli stessi controlli
  dell'avvio dalla finestra.

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

**Quello che le misure hanno insegnato.** Il manifest di ogni avvio registra le condizioni della
macchina (driver, build e serie di patch, overlay di alimentazione, VGM) e Benchmark avvisa quando due
avvii non sono confrontabili. Il riuso del prompt e le compattazioni si leggono dal log del motore,
richiesta per richiesta. Lo schema dei profili conosce le leve misurate (checkpoint, cache su RAM,
lettura pigra, override dei tensori), e una build locale con patch (`b<numero>+moro<n>`) serve un
profilo solo se il profilo la chiede per nome. Le schede dei client danno le righe da incollare e il
budget di contesto; per Claude Code c'è un template di chat tollerante.

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

### Che cosa non è stato guardato da una persona

Questa versione è un punto fermo messo il 18-09-2026 per scelta dell'operatore, prima di cambiare
direzione al progetto. L'uso normale della finestra è stato guardato e va. Questi pezzi invece sono
coperti solo dalle prove automatiche (91 unitarie e gli E2E reali, che li simulano tutti), e nessuno
li ha visti dalla finestra:

- i cinque guasti provocati: dialogo d'uscita con un download in corso, pagina «la radice dati non
  risponde», riquadro «Perché è uscito», «Ricollega…», riparazione di `machine.toml`;
- il riquadro «Condizioni dell'avvio», la riga del riuso durante una sessione vera con un client e
  Claude Code lanciato dalla riga copiata;
- l'installazione su una macchina senza Rust né Node: provata solo su una macchina che li ha, su un
  percorso pulito e con una radice dati nuova.

La finestra ridisegnata (v2) non è in questa versione: arriva subito dopo.

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
