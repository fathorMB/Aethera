---
updated: 2026-09-16
by: lead
---
**La lista di quello che devi provare tu è in [`reports/da-provare-operatore.md`](reports/da-provare-operatore.md).** L'app è **aperta adesso**: finestra «Aethera», endpoint 8090 che risponde, nessun motore acceso, porte libere.

In breve, in ordine di costo per te:

1. **Alla finestra, 20 minuti (M-05 T-08).** Nuovo profilo, duplica, rinomina, elimina; «Avvia…» dal Catalogo; «Importa da disco…»; «Importa cartella…»; model card. I dialoghi che chiedono un nome sono in-app (`window.prompt` non esiste nella WebView): va confermato che si vedano e rispondano.
2. **Alla finestra, i guasti, 15 minuti (M-06 T-09).** Uscita con un download in corso; radice dati rinominata mentre l'app è aperta; «Perché è uscito»; «Ricollega…»; `machine.toml` rotto a mano.
3. **Uno sguardo, 5 minuti (M-07 T-04 e T-05).** Stati vuoti e tema chiaro nell'app vera: finora li ho guardati solo montando la finestra nel browser con dati finti.
4. **Serve una VM (M-07 T-02).** Qui ho provato tutto tranne la clausola che conta, «senza Rust né Node».
5. **Serve Adrenalin e un riavvio (M-08 T-08).** VGM a 64 GB, poi rimetterla a 48. Non l'ho fatto da solo di notte.
6. **Due client su tre (M-08 T-10).** OpenCode non è installato e non l'ho installato al posto tuo; Claude Code non parla con un endpoint OpenAI, quindi contro questo motore non ci si punta — da correggere anche nello studio.

**Tre decisioni tue:** il contrasto del tema scuro (`--fg3` a 3,6:1 — `#838a94` lo mette a norma); il tag della v1; e se la condizione di FastFlowLM (uso libero sotto i 10 M$ di fatturato) vale per te.

M-08 è a undici task su quattordici: resta la NPU (T-11), il rapporto finale (T-12) e i due client.
