# Studio motore, NPU, Qwen3.8-Flash-Next e SSD (16-09-2026)

Un solo file, `index.html`, con nove pagine: Sintesi e proposte, Macchina e fisica, NPU XDNA 2, Motore llama.cpp, Qwen3.8-Flash-Next, Streaming da SSD, Mappa dei modelli, Piano di prova, Fonti. Navigazione dalla colonna sinistra o con i tasti 1–9; «tema» in alto a destra.

**Aggiornato il 16-09 con le misure di M-08** (vedi il riquadro in cima alla Sintesi e gli esiti in corsivo nel Piano di prova). In origine era uno **studio**, non una decisione né un milestone: nessuna misura nuova è stata fatta sulla macchina (un'altra sessione stava lavorando sui milestone e il motore non andava toccato). I numeri locali vengono dagli avvii in `<radice>\runs` e dal banco del 15-09 di minis-config; i numeri esterni da cinque ricerche web parallele condotte il 16-09-2026, con oltre 400 fra ricerche e pagine lette.

Ogni dato porta un marcatore: **V** verificato dalla fonte, **I** inferito o calcolato, **M** misurato su questa macchina. Le pagine di amd.com sono andate in timeout più volte: dove un dato AMD viene da fonti terze è detto.

## Come è stato costruito

`index.html` è la concatenazione dei file `part*.html` (testa con stile e navigazione, una sezione per pagina, coda con lo script). Per correggere una pagina si modifica il suo `part` e si rigenera:

```
cat part0-head.html part1-sintesi.html part2-macchina.html part3-npu.html part4-motore.html part5-qwen38.html part6-ssd.html part7-modelli.html part8-piano.html part9-fonti.html part99-tail.html > index.html
```

## Cosa chiede ad Aethera

Elencato nella pagina «Piano di prova», ultima tabella: campi profilo per checkpoint, MTP adattivo, `--n-cpu-moe`, `-ot`, `--lazy-mode`, `--fit`; manifest con piano energetico, driver, VGM, disco dei pesi; telemetria che distingue pagine mappate da working set; un secondo slot di motore «ausiliario» (`runtime.kind = "flm"`) per la NPU; build locali non ggml-org dichiarate in `machine.toml`.
