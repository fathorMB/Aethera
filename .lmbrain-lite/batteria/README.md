# Batteria di coding agentico (M-15)

Quindici compiti scritti per questa batteria, su dodici piccoli repository (Rust, Python, TypeScript
eseguito da Node 24 senza build). Servono a dire quale modello **risolve più compiti per ora di
macchina** su questa macchina, con Nonio come agente e il motore acceso da Aethera.

| cartella / file | cosa contiene |
|---|---|
| `compiti.toml` | enunciati (inglese; tre gemelli in italiano), livello, tipo, tetti, verifica, file protetti |
| `fixture/` | i repository come li vede l'agente |
| `nascosti/` | test in più, usati solo dal verificatore |
| `soluzioni/` | soluzioni di riferimento, usate solo da `congela.py` |
| `congelato.json` | hash di fixture, test nascosti, soluzioni, enunciati e verifiche |
| `batteria.py` | caricamento, copia di lavoro, verificatore |
| `congela.py` | prova ogni compito (fixture fallisce, soluzione passa, file protetto o test toccato fallisce) e scrive gli hash |
| `esegui.py` | il runner |

## Uso

```
set AETHERA_RADICE=<radice>
python congela.py --solo-controllo          # la batteria è quella congelata?
python esegui.py --modello G1 --attendi     # G1, FC, G3, FN: vedi MODELLI in esegui.py
```

Il runner vuole `m15_hold` compilato (`cargo build --release --example m15_hold` in `src-tauri`)
e Nonio (`AETHERA_NONIO_EXE`, altrimenti dal PATH). Scrive in `<radice>/m15/risultati/<sessione>/`
una riga JSONL per compito (`risultati.jsonl`), la traccia di Nonio e il riassunto di ogni compito;
le copie di lavoro restano in `<radice>/m15/lavoro/<sessione>/` per guardarle dopo.

Regole: un solo motore, acceso solo tramite Aethera, lock sull'endpoint per ogni compito, niente
avvio se c'è un altro `llama-server`, una build in corso o meno di 16 GiB liberi. Un valore che non
si può misurare è `null`, mai zero. Il rapporto è `reports/batteria-coding-2026-09.md`.
