---
updated: 2026-09-20
by: lead
---
**Sessione chiusa il 20-09 pomeriggio.** Motore spento, VGM 64.

**Il profilo principale non si tocca:** G1 Q8 con `thinking = false` — ora confermato dalla misura (acceso costa un terzo della produttività e non risolve un compito in più). M-18 T-05 chiuso.

**Ling-3.0-flash scartato:** 4,6 compiti/ora contro 34,9. Verdetto dal banco (prefill 0,30×, decode 0,84×), non dalla batteria. I 59 GB in `C:\models\ling-3.0-flash\` si possono liberare.

**Due cose che valgono oltre Ling:**
- Il riuso del prefisso è **tutto-o-niente**: `llama-server` riusa solo se il prompt nuovo è un'estensione *esatta*; alla prima divergenza è **zero**, non parziale. Alza molto il valore di M-18 T-14 — che però va riscritto, perché il sospetto su `serde_json` è smentito (`preserve_order` c'è già) e l'attribuzione «from system» di Nonio è circolare.
- Nella speculazione il collo di bottiglia **non è l'accettazione**: l'81,5% emette 3,45 token per verifica, ma una verifica costa 3,06× un passaggio semplice → netto 1,13×. Durante la verifica la macchina non è limitata dalla banda. Quindi T-03 (bozze più lunghe) va misurato aspettandosi un peggioramento.

**M-19 attivo**, T-01 e T-02 chiusi dalla sola telemetria. Sul G3 il netto stimato è 0,96× (MTP che costa il 4%): l'operatore ha deciso che la misura A-B non vale la pena adesso.

**Nonio:** ramo `famiglia-argomenti-strumenti` pubblicato e non unito; Qwen invariato byte per byte, verificato.

**Non spiegato:** la perdita di cache di Ling (0% a ogni turno in multi-turno con strumenti, 87% a due turni). Si riproduce con `curl` da solo — non è l'harness. Due ipotesi proposte e smentite entrambe dalla misura: non proporne una terza senza misurarla.
