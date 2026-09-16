# Quello che devi provare tu

Aggiornato il 2026-09-16, dopo la notte di misure (M-08). L'app è **già aperta**: finestra
«Aethera», endpoint `127.0.0.1:8090` che risponde. Nessun motore acceso, porte 8080/8081/8090
libere.

Ordinato per quanto costa a te, non per numero di milestone. I primi tre gruppi si fanno alla
finestra, adesso. Gli ultimi due chiedono qualcos'altro.

---

## 1 · Alla finestra, subito (circa 20 minuti) — M-05 T-08

È l'unica cosa che manca a M-05: il codice è scritto e provato, ma **la finestra non l'ha ancora
guardata nessuno**. I dialoghi che chiedono un nome sono componenti in-app, non `window.prompt`
(nella WebView di Windows non esiste): la cosa da confermare è che **si vedano e rispondano**.

- [ ] **Avvio → «Nuovo profilo…»**: crearne uno da zero. L'alias deve seguire il nome mentre lo
      scrivi, e la validazione deve parlare mentre scrivi, non al salvataggio.
- [ ] **«Duplica…»**, **«Rinomina…»**, **«Elimina…»** sullo stesso profilo. L'eliminazione chiede
      conferma e **non** deve toccare gli avvii già registrati che lo citano (controlla Benchmark
      dopo).
- [ ] **Catalogo → «Avvia…»** su una riga: deve aprire la pagina Avvio con quel modello già scelto.
- [ ] **Catalogo → «Importa da disco…»** su un `.gguf` che sta fuori dalla cartella dei pesi: deve
      riconoscerlo per hash e **non** duplicarlo.
- [ ] **Catalogo → scheda Build → «Importa cartella…»**: gemello di quello nelle Impostazioni.
- [ ] **«Leggi la model card»** e **«Prendi dal catalogo»**: il campionamento consigliato deve
      arrivare fino ai frammenti per i client.

## 2 · Alla finestra, i guasti (circa 15 minuti) — M-06 T-09

Ognuno di questi deve dire **cosa è successo e cosa fare**, senza perdere dati.

- [ ] **Uscita con un download in corso**: fai partire un download dal Catalogo e chiudi l'app. Deve
      aprirsi il dialogo che elenca i lavori, qualunque sia la preferenza d'uscita impostata.
- [ ] **Radice dati che non risponde**: rinomina `C:\AetheraData` mentre l'app è aperta. Deve
      comparire una pagina intera al posto delle altre, con «Riprova» e «Scegli un'altra cartella…».
      (Poi rinominala indietro.)
- [ ] **«Perché è uscito»** sulla pagina Motore: avvia un profilo con una leva inesistente (mettila
      in `extra_args`) e guarda il riquadro che mette in evidenza le righe di log che spiegano.
- [ ] **«Ricollega…»** nel Catalogo: rinomina un `.gguf` e ricollegalo.
- [ ] **Riparazione di `machine.toml`** dalle Impostazioni: rompilo a mano (una riga a caso) e
      guarda cosa propone.

## 3 · Alla finestra, uno sguardo di conferma (5 minuti) — M-07 T-04 e T-05

Questi due li ho già chiusi, ma li ho guardati montando la finestra nel browser con dati finti.
Un secondo sguardo nell'app vera non è sprecato.

- [ ] Gli **stati vuoti** delle cinque pagine (una radice dati nuova li mostra tutti).
- [ ] Il **tema chiaro**, pagina per pagina, con l'interruttore.

**Una decisione che è tua, non mia:** in tema **scuro** il grigio delle condizioni (`--fg3`,
`#6f7680`) sta a 3,6:1 e l'etichetta «Ferma» a 4,42:1, sotto la soglia di 4,5 per testo piccolo.
Non l'ho toccato perché la palette scura è quella che hai approvato in M-01. `#838a94` la porta a
norma senza cambiarne l'aria: dimmi se la cambio.

---

## 4 · Serve una macchina pulita — M-07 T-02

Di questo ho provato tutto il resto: installazione silenziosa in un percorso pulito, voce di
disinstallazione con editore e versione giusti, app avviata da lì con configurazione vuota,
disinstallazione che non lascia né cartella né registro né collegamenti. Quello che **non** si può
provare qui è proprio la clausola che conta: «senza Rust né Node sulla macchina», e questa li ha
entrambi.

- [ ] Installare `src-tauri\target\release\bundle\nsis\Aethera_0.1.0_x64-setup.exe` su una VM o su un
      secondo computer, e arrivare al motore acceso.

## 5 · Serve la GUI Adrenalin e un riavvio — M-08 T-08

La VGM a 64 GB non si cambia da riga di comando. **Non l'ho fatto da solo di notte**: chiede la
finestra di Adrenalin e un riavvio, e va rimessa a 48 dopo.

- [ ] Portare la VGM a 64 GB, riavviare, rilanciare
      `m08_bench.exe C:/AetheraData .lmbrain-lite/m08/T-08-oltre48.toml`, poi **rimetterla a 48**.
      Serve a chiudere la domanda «il decode torna al tetto se tutto è dedicato?». Quello che già
      sappiamo: a 48 GB il Coder-Next da 48,53 GB **ci sta** (46,10 GiB dedicati, 0,37 condivisi) e
      fa 17,75 tok/s, quindi la VGM a 64 serve solo a sapere se si guadagna ancora, non a farlo
      funzionare.

## 6 · Le misure nella finestra (circa 25 minuti) — M-09 T-11

Aggiunto il 16-09 sera. Apri la versione nuova: `src-tauri\target\release\aethera.exe` (o
l'installatore in `src-tauri\target\release\bundle\nsis\`). L'E2E l'ha già provata dal codice con
i tre client; qui serve il tuo sguardo sulla finestra.

- [ ] **Avvio → G1** (`qwen3.6-35b-a3b…`): sotto il nome compare la proposta «runtime.build
      b10809 → b10991». «Applica come modifiche» la mette sopra il profilo senza salvarlo.
- [ ] Nello stesso profilo scrivi `qwen3.6-tollerante.jinja` in **chat_template_file** e porta
      **ctx** a 65536: la riga di comando mostra `--chat-template-file …\templates\…`. Guarda le
      etichette «M-08: scartata» nella sezione Cache (il motivo è al passaggio del mouse). Avvia.
- [ ] **Motore**: il riquadro «Condizioni dell'avvio» con driver GPU e NPU, AMD Software,
      «Massime prestazioni», VGM e il disco Kingston.
- [ ] **Impostazioni → Riga per i client → Claude Code**: badge verde «template tollerante attivo».
      «Copia come PowerShell», incolla in un terminale in una cartella di prova, lancia `claude` e
      fagli fare qualcosa di vero per qualche turno. Non deve comparire nessun errore 500.
- [ ] Durante quella sessione, **Motore → Richiesta per richiesta**: le righe arrivano una alla
      volta, «estende» dopo la prima. Il client resta «sconosciuto», perché Claude Code non prende
      il lock: è previsto. Se la conversazione si allunga fino a una compattazione, compare
      l'avviso con il tempo che è costata.
- [ ] **Impostazioni → Budget di contesto**: Claude Code, OpenCode e Nonio con i prompt fissi
      misurati; passa il mouse su un numero per la fonte.
- [ ] **Benchmark**: gli avvii di oggi pomeriggio hanno la colonna delle condizioni, quelli di prima
      dicono «condizioni sconosciute». Selezionane uno di ciascun tipo: il confronto avvisa che il Δ
      non è del solo profilo.

**Da sapere prima.** Nel primo giro dell'E2E OpenCode non ha caricato la configurazione di Aethera
(per un difetto del banco, non della riga) ed è ricaduto sul suo modello in cloud «big-pickle»,
mandandogli `src-tauri/src/conditions.rs`. Il banco ora forza il modello con `-m aethera/…`.
Quando usi OpenCode a mano, controlla che l'intestazione dica `build · qwen3.6-…` e non un modello
in cloud.

---

## 7 · Due cose da decidere, non da provare

- [ ] **Il tag della v1.** Le note di rilascio ci sono (`RELEASE-NOTES.md`, 0.1.0). Il tag lo metti tu
      dopo i punti 1, 2 e 4: `git tag -a v0.1.0 -m "..."` e `git push origin v0.1.0`. Se prima la
      vuoi chiamare `1.0.0`, la versione va cambiata in `tauri.conf.json`, in `Cargo.toml` e nella
      riga del titolo della finestra.
- [ ] **FastFlowLM (M-08 T-11).** La CLI è MIT, ma i kernel per la NPU sono binari proprietari,
      liberi solo per uso non commerciale **o per aziende sotto i 10 M$ di fatturato**. Uso la
      versione portatile in zip, senza installare niente e senza toccare il driver NPU: la
      condizione sul fatturato però la puoi confermare solo tu.
