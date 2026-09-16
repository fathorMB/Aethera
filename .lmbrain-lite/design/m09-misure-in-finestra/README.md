# Aethera M-09 — mockup

Un file, `index.html`, con le quattro pagine che cambiano (il Catalogo resta com'è). Tasti 1, 2, 4, 5 o colonna sinistra; «tema» passa fra scuro e chiaro; «nascondi i riquadri nuovo» mostra la pagina come la vedrebbe l'operatore.

Ogni modifica ha un bordo tratteggiato con il task che la realizza.

| Pagina | Che cosa cambia | Task |
|---|---|---|
| Motore | riquadro «Condizioni dell'avvio»; avviso quando le condizioni sono cambiate dall'avvio precedente; «Riuso del prompt» letto dal log al posto della cache da `/metrics`; tabella richiesta per richiesta con estende / a freddo / compattazione e il tempo che è costata | T-02, T-03, T-04, T-05 |
| Avvio | `gpu layers` numero o «fit» con `fit_target`; `n_cpu_moe`, `tensor_overrides`, `lazy_mode`; sezione Cache completa con l'etichetta del verdetto di M-08; scelta del template di chat (tollerante per Claude Code); avviso mmap su un modello che riempie la VGM; profili standard aggiornati | T-06, T-07, T-08 |
| Benchmark | colonna condizioni; riga di separazione dove cambia un driver e la mediana riparte; confronto che avvisa quando A e B hanno condizioni diverse | T-02, T-03 |
| Impostazioni | schede per client nella «Riga per i client», con Claude Code in PowerShell; budget di contesto per client; driver, alimentazione e volume nella Macchina | T-02, T-08, T-09 |

## Decisioni già prese

- Claude Code contro Qwen3.6: **template di chat tollerante** passato con `--chat-template-file`, Claude Code punta dritto al motore (operatore, 16-09). Niente adattatore nell'endpoint.
- Di conseguenza il riuso si legge **dal log del motore** e da `timings.cache_n`, non da un ponte. Il client di una richiesta si conosce solo se ha preso il lock su `/lock`.

## Numeri

Veri, da M-08: driver e condizioni della macchina, prefill e decode degli avvii, la sessione di Claude Code (16.822 a freddo in 48 s, riassunto 18.884 in 79 s, contesto ricostruito 7.911 in 29 s), i 39,2 GiB di mmap sul Coder-Next. **Illustrativi**: orari, conteggi intermedi della tabella, accettazione 71%, prompt fisso di Nonio, output riservato.

## Aperto alla revisione

- Soglie della rilevazione delle compattazioni (riuso < 5% dopo richieste > 90%) e del budget rosso (< 30% del contesto).
- Etichette «M-08: scartata» sulle leve: informano, non bloccano. Troppo invadenti?
- `CLAUDE_CONFIG_DIR` separata proposta come facoltativa.
- Aggiornamento dei profili standard: arriva come proposta con differenze, non sovrascrive un profilo dell'utente.
