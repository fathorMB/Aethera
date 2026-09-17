# Aethera v2 — interfaccia più leggibile (mockup, 16-09-2026)

Un solo file, `index.html`, con sei pagine: **Analisi** (0) e le cinque della finestra riordinate: Motore (1), Avvio (2), Catalogo (3), Benchmark (4), Impostazioni (5). Tasti 0–5 o colonna sinistra; «tema» passa fra scuro e chiaro; «nascondi “cosa cambia”» spegne i bordi tratteggiati e mostra la pagina come la vedrebbe l'operatore.

È un **riordino**, non un milestone di misure: ogni dato del mockup esiste già in `src/api.ts`. Non cambia la tavolozza, i token, il vocabolario degli stati né le regole approvate in M-09. Nessuna modifica al codice: aspetta lo sguardo dell'operatore.

## Da dove viene

Letti `src/App.tsx`, le cinque pagine in `src/pages/`, `src/components.tsx`, `src/styles.css` e i mockup `aethera-v1` e `m09-misure-in-finestra`. La pagina Analisi elenca i problemi trovati, pagina per pagina, e la risposta della v2.

## I sei principi

1. **Lo stato del motore si vede sempre**: una striscia in alto con stato, profilo, velocità, «in uso», Riavvia e Ferma, su ogni pagina.
2. **L'azione principale sta in cima**: azioni nella testa di ogni pagina; in Avvio una fascia fissa con Avvia, Scarta, Aggiorna, Salva come.
3. **Essenziale prima, avanzato dietro un clic**: dodici leve con l'etichetta in italiano e il nome della leva in piccolo; le altre 17 in «Avanzate», chiuse, con i verdetti di M-08 come pillole.
4. **Le spiegazioni si chiedono**: il «?» accanto ai titoli (title) e le righe «come si legge» a scomparsa; testo minimo a 12 px.
5. **Un numero, una barra, una parola**: riga di KPI con giudizio; memoria come barra impilata; scintille in riga in Benchmark.
6. **Quello che era giusto resta**: «sconosciuto» non è zero, il ● sulle modifiche, la riga di comando esatta, le condizioni accanto ai numeri.

## Come è costruito

`index.html` è la concatenazione dei `part*.html`. Per correggere una pagina si modifica il suo `part` e si rigenera:

```
cat part0-head.html part1-analisi.html part2-motore.html part3-avvio.html part4-catalogo.html part5-impostazioni.html part99-tail.html > index.html
```

Controllato a 1440×900 in scuro e in chiaro: nessuna pagina deborda in orizzontale, nessun errore di console.

## Numeri

Veri, da M-08 e M-09: prefill 418 e decode 33,1 del G1 col driver nuovo, riuso 93 %, la compattazione di Claude Code (121 s, 16.822 token di prompt fisso), VRAM 23,13 GiB, le condizioni della macchina, gli avvii della notte del 16-09 (Coder-Next 17,8 tok/s, mmap con doppia copia, n_cpu_moe 16 a 13,8). **Illustrativi**: orari, conteggi intermedi delle tabelle, il download di Q3_K_XL al 41 %, la riga «se il contesto fosse 32k».

## Aperto alla revisione

- La striscia del motore in alto toglie 24 px alla barra e mette Ferma/Riavvia a portata di clic da ogni pagina: è un bene, o è pericoloso?
- Le dodici leve «essenziali» sono una scelta: quali mancano, quali sono di troppo.
- Le etichette in italiano al posto dei nomi delle leve: il nome resta in piccolo sotto, ma chi conosce llama.cpp potrebbe preferire il contrario.
- Il log del motore chiuso di default (si apre da solo dopo un'uscita con errore).
- Impostazioni in tre schede: Macchina, Client, App. L'endpoint sta in App, a scomparsa.

## Com'è andata nel codice (M-12, 17-09)

Approvato così com'era (punti aperti compresi). Nel codice, branch `m12-finestra-v2`, è cambiato
questo rispetto al mockup, e perché:

- **Verdetti sulle leve.** Tenuti solo quelli con una fonte: q8_0 (M-08 T-06), mmap (T-09), e i tre
  della v1 (n_cpu_moe, cache_reuse, checkpoint). «+21 % di prefill» su ubatch, «tutti, fino a
  70 GB» sui layer e «negativo su A3B» sul draft esterno non stanno in nessun rapporto: tolti.
- **Template di chat** è una casella, non una tendina: `api.ts` non elenca i template.
- **Striscia**: profilo, build e porta, ma non il nome del modello (`RunInfo` non lo porta).
- **Download**: niente velocità né tempo stimato (`TaskView` ha solo fatto/totale). «Togli»
  vale per tutti i lavori finiti insieme, come l'API.
- **Memoria** come barra su VGM + RAM vista: dedicata, VRAM non usata, condivisa, RAM di Windows e
  altri processi (RAM vista − disponibile − condivisa), RAM disponibile. Senza VGM la barra resta
  vuota e tratteggiata.
- **KPI**: il prefill non ha giudizio (non c'è una mediana di riferimento per il prefill); il decode
  è giudicato solo contro il riferimento degli avvii con le stesse condizioni.
- **Avvio**: il nome non è più un campo; si dà con «Salva come…» (dialogo) e gli errori sul nome
  compaiono nella fascia. «Avanzate» contiene anche fit/fit_target, runtime.kind e le note del
  profilo, che il mockup non elencava: nessun campo della v1 sparisce (lo prova un test). La build
  per l'importazione da minis-config si chiede in un dialogo. Il suggerimento sta sotto il
  controllo, non a destra: a 1280 px schiacciava i campi.
- **Catalogo**: la rimozione è un dialogo della finestra con la casella «Cancella anche il file»
  (prima erano due `window.confirm`). I filtri mostrano solo gli stati presenti.
- **Benchmark**: il confronto aperto compare sotto la tabella; l'accettazione, le condizioni e il
  commit stanno nel dettaglio; sotto circa 1400 px di pagina Build e Richieste escono dalla tabella
  (e sotto 1250 anche la VRAM), restando nel dettaglio.
- **Impostazioni**: «Salva machine.toml» si accende solo con modifiche; un errore dell'endpoint si
  vede fuori dalla sezione a scomparsa. Niente riga «se il contesto fosse 32k», che era
  illustrativa.
- **Token**: i fondi tenui del tema scuro sono a .08 invece di .12 (a .12 il grigio e il rosso sulle
  righe tinte scendevano a 4,4:1). Il Seg attivo e la pillola «proposte» hanno testo `--fg`.
- **Contrasto misurato** con il banco nel browser su cinque pagine, due temi, 1280/1440/1920 px:
  nessun testo nuovo sotto 4,5:1. Restano due casi **della v1**, non toccati perché sono della
  tavolozza: il bianco dei pulsanti primari su `--acc` scuro (3,1:1) e `badge acc` / il numero del
  passo attivo su `--bg3` chiaro (3,8:1), più il ✓ bianco su `--ok` scuro nel Primo avvio.

## Se approvato

Diventa un milestone di sola interfaccia: nessun comando Tauri nuovo, solo `App.tsx`, le cinque pagine, `components.tsx` e `styles.css`. Stima: la striscia e le teste di pagina un giorno; Avvio (essenziali/avanzate, fascia fissa) un giorno; Catalogo e Benchmark (lista + dettaglio, fascia di confronto) un giorno; Impostazioni a schede mezza giornata; banco nel browser con i dati veri e prova dell'operatore mezza giornata.
