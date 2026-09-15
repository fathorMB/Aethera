# Aethera v1 — mockup

Un solo file, `index.html`, con le cinque pagine della v1 e una sesta di componenti condivisi (tray, vocabolario degli stati, marcatori). Navigazione dalla colonna sinistra o con i tasti 1–5; il pulsante «tema» in alto a destra passa fra scuro e chiaro.

Dati mostrati: quelli misurati il 15-09 sulla Minisforum (`minis-config/docs/13-launcher.md`). Sono numeri veri messi in una finestra finta: servono a vedere la densità e il linguaggio, non a essere letti come risultato.

## Cosa mostra ogni pagina

| Pagina | Decisioni che incarna |
|---|---|
| Motore | stato + «in uso», memoria prima/stima/misurata con doppia copia, cache del prefisso per turno, contesto dichiarato vs servito, riga di comando esatta, comandi rifiutati mentre in uso, log |
| Avvio | profilo come base con campi modificati in sovrapposizione (●), leve marcate `cache`, riga di comando live con differenze evidenziate, stima di memoria, campionamento della card come dato del modello |
| Catalogo | modelli con stati presente/verificato/scaricabile/in download, SHA-256 contro oid LFS, metadati GGUF (MTP), hard link riconosciuti; build con tag, commit, backend, digest GitHub, `--list-devices` |
| Benchmark | storico degli avvii (uno per riga) con prefill/decode/accettazione/cache/VRAM e condizioni; confronto fra due avvii; decode per richiesta; manifest |
| Impostazioni | radice dati, macchina (VGM letta), comportamento all'uscita, endpoint Aethera, riga da copiare per Nonio e blocco env per banchi |
| Tray e stati | vocabolario degli stati con i comandi permessi, menu della tray, dialogo «Esci» |

## Aperto alla revisione

- Nome e porta dell'endpoint proprio (`:8090`) e i suoi cinque percorsi.
- Regola di «degradato» (24 h o decode < 70 % della mediana dell'avvio): soglie inventate, da confermare.
- Finestra di 30 s per lo stato «in uso» osservato.
- Stato «orfano» (motore sulla porta non avviato da Aethera): adottare in sola lettura o terminare.
