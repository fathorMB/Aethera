# int8 coopmat — report per l'operatore (M-16, 17-09-2026)

`index.html` è un solo file, senza risorse esterne: si apre con un doppio clic. Racconta in forma
leggibile il milestone M-16, cioè se la patch int8 coopmat (PR ggml-org/llama.cpp#27952) conviene e
a quale modello.

Dentro: quanto la patch si discosta dalla base (divergenza KL contro due metri, l'ubatch e la CPU),
il prefill per ogni ubatch con e senza patch su G1 e G3, il tempo risparmiato su un prompt a freddo,
l'esito della batteria di compiti, la regola di fedeltà riscritta e le decisioni che restano
all'operatore.

**Non è la fonte dei numeri**: quella è `.lmbrain-lite/reports/int8-coopmat-2026-09.md`, con metodo,
comandi e dettagli. Le misure grezze stanno in `<radice>\m16`.

Il tema segue `data-theme` sull'elemento `<html>`: `dark` come impostato, `light` cambiandolo a mano.
