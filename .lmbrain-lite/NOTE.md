---
updated: 2026-09-16
by: lead
---
**M-03 e M-04 chiusi.** La v1 ha motore, profili, memoria misurata, telemetria, stato «in uso», endpoint, catalogo, stima, hash, download e build verificate. Pubblicato fino a `b2afcba`.

**Tre milestone proposti, in attesa che tu li approvi nell'app:**

- **M-05 — Uso quotidiano senza toccare i file a mano.** Oggi un profilo nuovo si fa solo importando i JSON di minis-config o duplicandone uno, e non si cancella dalla finestra. Qui dentro finiscono anche **due debiti miei**: «Avvia…» dal Catalogo e i pulsanti «Importa da disco…» / «Importa cartella…» erano nel titolo di M-04 T-07 e non li ho realizzati.
- **M-06 — Comportamento quando qualcosa va storto.** Configurazioni illeggibili, pesi spariti, disco pieno, rete caduta, app chiusa con un download in corso. Oggi non è mai stato provato niente di tutto questo.
- **M-07 — Confezionamento, prima esecuzione e documentazione.** Il percorso guidato dal nulla al motore acceso, il tema chiaro mai guardato, e il README che oggi è di **9 byte**.

Ordine consigliato: 5 → 6 → 7, con M-07 per ultimo perché note di rilascio e tag si scrivono alla fine. Se preferisci un altro ordine, riordinali nell'app.

**Buona notizia sul confezionamento**: ho provato `tauri build` e funziona già — esce `Aethera_0.1.0_x64-setup.exe` (3,5 MB) in 2m13s. Quindi M-07 T-01 è verifica e rifinitura, non lavoro da zero. Resta intatto **T-02**, che è la prova che conta: quell'installatore non l'ha ancora eseguito nessuno, e «si costruisce» non vuol dire «si installa e parte su una macchina senza Rust né Node».
