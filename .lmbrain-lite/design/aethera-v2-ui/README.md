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

## Se approvato

Diventa un milestone di sola interfaccia: nessun comando Tauri nuovo, solo `App.tsx`, le cinque pagine, `components.tsx` e `styles.css`. Stima: la striscia e le teste di pagina un giorno; Avvio (essenziali/avanzate, fascia fissa) un giorno; Catalogo e Benchmark (lista + dettaglio, fascia di confronto) un giorno; Impostazioni a schede mezza giornata; banco nel browser con i dati veri e prova dell'operatore mezza giornata.
