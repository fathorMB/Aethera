# Log

One line per action. Format: `timestamp | by:<who> | <target or -> | <kind> | <message>`.

2026-09-15T21:59 | by:lead | - | note | Sessione di analisi avviata: letto minis-config/docs/13-launcher.md (requisiti del launcher), il progetto è vuoto; si parte con le domande di progetto una per volta, poi mockup in .lmbrain-lite/design/ prima di implementare.
2026-09-15T21:59 | by:lead | - | decision | Stack scelto: Tauri 2 + SolidJS, allineato alla finestra di Nonio (job object per llama-server, tray nativa, backend Rust per Vulkan/GPU/download).
2026-09-15T22:00 | by:lead | - | decision | Profili in TOML con schema proprio: Aethera non è legata al metodo dei test di minis-config, definisce una gestione ordinata del motore; i profili JSON esistenti si importano, non si ereditano come formato.
2026-09-15T22:02 | by:lead | - | decision | Confine banchi: Aethera non lancia banchi; ha profili di avvio multipli più configurazione manuale libera da UI (stile LM Studio) e una pagina Benchmark che riporta le performance dallo storico già eseguito.
2026-09-15T22:02 | by:lead | - | decision | Pagina Benchmark alimentata dalla telemetria propria di Aethera (registro degli avvii: prefill/decode, token accettati, quota cache, memoria, tempo acceso), nessun import di file esterni nella v1.
2026-09-15T22:02 | by:lead | - | decision | Schermata di avvio: profilo come base, campi modificati evidenziati in sovrapposizione, riga di comando aggiornata in tempo reale, avvio con differenze registrate nel manifest oppure «salva come nuovo profilo».
2026-09-15T22:03 | by:lead | - | decision | Stato «in uso»: osservazione di /slots e /metrics più interruttore manuale «proteggi il motore» per tutti i client, e in aggiunta un endpoint proprio di Aethera (stato, id dell'avvio, lock) per i client che vogliono dichiararsi.
2026-09-15T22:04 | by:lead | - | decision | Dati su disco: radice dati scelta dall'utente al primo avvio (machine.toml, profiles/, builds/, runs/<id>/, telemetria); i profili non contengono percorsi assoluti e riferiscono i pesi per hash e nome file, così restano portabili fra Turing e Minisforum.
2026-09-15T22:04 | by:lead | - | decision | Ciclo di vita: la X riduce nella tray con motore acceso; «Esci» chiede se fermare o lasciare orfano il motore (preferenza ricordata); un solo motore attivo nella v1, schema pronto per più motori.
2026-09-15T22:05 | by:lead | - | decision | Portabilità: Windows prima con letture di sistema dietro un'interfaccia Rust; su altri OS i valori sono «sconosciuto», nessuna schermata assume Windows.
2026-09-15T22:05 | by:lead | - | decision | Download nella v1: build llama.cpp da release ggml-org con SHA-256 dal digest GitHub e pesi da Hugging Face (resolve/main, riprendibile, SHA-256 contro oid LFS); import da disco per hash con riconoscimento degli hard link.
2026-09-15T22:06 | by:lead | - | decision | Navigazione della v1: cinque pagine (Motore, Avvio, Catalogo, Benchmark, Impostazioni) più tray e riga da copiare per i client; sono le schermate dei mockup.
2026-09-15T22:06 | by:lead | M-01 | milestone | created 'Mockup di design della v1' with 7 tasks (proposed)
2026-09-15T22:16 | by:lead | M-01/T-01 | decision | Linguaggio visivo: strumento tecnico scuro e denso (tema chiaro secondario), monospazio per riga di comando/hash/numeri, badge di stato, ogni misura con unità e condizione, «sconosciuto» esplicito.
2026-09-15T22:19 | by:operator | M-01 | status | proposed -> approved
2026-09-15T22:19 | by:lead | M-01 | status | approved -> active
2026-09-15T22:25 | by:lead | M-01/T-01 | task | done: Tema scuro/chiaro con token, monospazio per riga di comando e numeri, badge di stato, marcatori ● modificato / cache / sconosciuto in design/aethera-v1/index.html — Linguaggio visivo e navigazione: tema, densità, palette, componenti condivisi (stato motore, badge memoria, riga di comando)
2026-09-15T22:25 | by:lead | M-01/T-02 | task | done — Mockup pagina Motore: stato, memoria misurata, cache del prefisso, log, riga di comando, protezione e riavvio
2026-09-15T22:25 | by:lead | M-01/T-03 | task | done — Mockup pagina Avvio: profilo base, leve in sovrapposizione, riga di comando live, salva come profilo
2026-09-15T22:25 | by:lead | M-01/T-04 | task | done — Mockup pagina Catalogo: modelli e build, stati presente/verificato/scaricabile, stime di memoria, download
2026-09-15T22:25 | by:lead | M-01/T-05 | task | done — Mockup pagina Benchmark: storico degli avvii, prefill/decode/accettazione/cache per profilo
2026-09-15T22:25 | by:lead | M-01/T-06 | task | done — Mockup pagina Impostazioni e tray: radice dati, macchina, comportamento all'uscita, riga da copiare per i client
2026-09-15T22:25 | by:lead | M-01/T-07 | finding | Mockup aethera-v1 scritto e controllato nel browser (sei pagine in un file); soglie inventate da confermare in revisione: degradato 24 h / decode < 70 %, finestra «in uso» 30 s, endpoint :8090, stato «orfano».
2026-09-15T22:33 | by:lead | M-01/T-07 | task | done: Operatore: mockup approvati senza correzioni; soglie confermate (degradato 24 h / decode < 70 %, in uso 30 s, endpoint :8090, stato orfano) — Revisione con l'operatore e registrazione delle correzioni nel milestone
2026-09-15T22:33 | by:lead | M-01 | status | active -> done: Mockup rivisti e approvati dall'operatore il 2026-09-15
2026-09-15T22:33 | by:lead | M-02 | milestone | created 'Scheletro Tauri, schema dei profili e ciclo di vita del motore' with 10 tasks (proposed)
2026-09-15T22:33 | by:lead | M-01 | decision | Soglie del mockup confermate dall'operatore: degradato 24 h / decode < 70 % della mediana, finestra «in uso» 30 s, endpoint Aethera :8090, stato «orfano» adottabile in sola lettura.
2026-09-15T22:55 | by:operator | M-02 | status | proposed -> approved
2026-09-15T22:56 | by:lead | - | commit | Commit e push su origin/main di kit lmbrain-lite, decisioni, mockup aethera-v1 e milestone M-01 (done) / M-02 (proposed); sessione di analisi chiusa.
