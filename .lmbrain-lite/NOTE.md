---
updated: 2026-09-16
by: lead
---
**M-05, M-06 e M-07 scritti, provati e pubblicati** (`5b6464a`, `b5ce08f`, `69f5228`). 91 prove unitarie verdi, due nuovi E2E reali sulla Minisforum (`e2e_m05` 36 controlli, `e2e_m06` 22), installatore costruito, installato, avviato e disinstallato.

## Dipende da te

**1. Guarda la finestra.** È l'unica cosa che manca davvero: stanotte non l'ha aperta nessuno.

- **M-05 T-08** — «Nuovo profilo…», «Duplica…», «Rinomina…», «Elimina…» coi loro dialoghi; «Avvia…» da una riga del Catalogo; «Importa da disco…»; «Importa cartella…» nella scheda Build; «Leggi la model card» e «Prendi dal catalogo». I dialoghi sono in-app perché `window.prompt` nella WebView non esiste: vanno visti aprirsi e rispondere.
- **M-06 T-09** — dialogo d'uscita con un download in corso, pagina «la radice dati non risponde» (stacca un disco o rinomina la cartella), riquadro «Perché è uscito» sul Motore, «Ricollega…» nel Catalogo, riparazione di `machine.toml`.
- **M-07 T-04 e T-05** sono chiusi, ma li ho guardati montando la finestra nel browser con dati finti: un secondo sguardo nell'app vera non è sprecato.

**2. M-07 T-02 — una macchina pulita.** Provato tutto il resto (installazione silenziosa, avvio dal percorso installato, disinstallazione senza tracce), ma la clausola «senza Rust né Node» qui non è provabile: serve una VM o un altro computer.

**3. M-07 T-07 — il tag.** Note di rilascio scritte (`RELEASE-NOTES.md`). Il tag si mette dopo i punti 1 e 2: `git tag -a v0.1.0` e `git push origin v0.1.0`. Per chiamarla `1.0.0` la versione va cambiata in `tauri.conf.json`, `Cargo.toml` e nel titolo della finestra.

## Decidi tu

- **Contrasto del tema scuro:** `--fg3` a 3,6:1 e «Ferma» a 4,42:1, sotto soglia. Non l'ho toccato perché è la palette che hai approvato in M-01; `#838a94` la porta a norma senza cambiarne l'aria.
- **`.lmbrain-lite/design/studio-motore-2026-09/`** ha due file modificati che non ho scritto io (la correzione sui ~177B). Lasciati fuori dai miei commit.
