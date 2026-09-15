//! Campionamento consigliato letto dalla model card del publisher.
//!
//! È un **dato del modello**, non un default del server: llama-server non lo applica, lo manda il
//! client in ogni richiesta. Aethera lo raccoglie perché altrimenti finisce a memoria, e lo porta
//! dove serve — nel profilo e nei frammenti per i client — sempre con la fonte e la data in cui è
//! stato letto, così chi lo legge sa da dove viene e quanto è vecchio.
//!
//! Qui si riconosce, non si interpreta: se la model card non dice `temperature`, `temperature`
//! resta sconosciuta. Le sezioni della pagina danno la modalità (pensiero, non-pensiero, codice):
//! quando l'intestazione non dice niente, la modalità è `default`.

use crate::profile::Sampling;
use std::collections::BTreeMap;

/// Modalità riconosciute nelle intestazioni della model card.
/// L'ordine conta: vince la prima che corrisponde, quindi la più specifica sta prima.
/// «Thinking mode for precise coding tasks» è un consiglio per il codice, non per il pensiero.
const MODES: &[(&str, &str)] = &[
    ("non-thinking", "non_thinking"),
    ("non thinking", "non_thinking"),
    ("nonthinking", "non_thinking"),
    ("no thinking", "non_thinking"),
    ("instruct", "non_thinking"),
    ("coding", "coding"),
    ("agentic", "agentic"),
    ("creative", "creative"),
    ("translation", "translation"),
    ("thinking", "thinking"),
    ("reasoning", "thinking"),
    ("chat", "chat"),
];

/// Parole che dichiarano una sezione dedicata al campionamento. Senza una di queste — o senza una
/// modalità scritta nella riga stessa — i numeri di una pagina non sono consigli: sono i parametri
/// con cui il publisher ha misurato un banco, o un esempio di codice.
const SECTION_WORDS: &[&str] =
    &["sampling", "best practice", "recommend", "parameter", "generation config", "hyperparameter"];

#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub sampling: BTreeMap<String, Sampling>,
    /// Quello che il lettore deve sapere: quante modalità, da dove, che cosa non è stato trovato.
    pub notes: Vec<String>,
}

/// Solo lettere e cifre, minuscole: `Top-P`, `top_p` e `TopP` sono la stessa leva.
fn key_of(raw: &str) -> String {
    raw.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_lowercase()
}

/// Il nome di leva riconosciuto, o niente.
fn lever(raw: &str) -> Option<&'static str> {
    match key_of(raw).as_str() {
        "temperature" | "temp" => Some("temperature"),
        "topp" => Some("top_p"),
        "topk" => Some("top_k"),
        "minp" => Some("min_p"),
        "presencepenalty" | "presence" => Some("presence_penalty"),
        "repeatpenalty" | "repetitionpenalty" => Some("repeat_penalty"),
        _ => None,
    }
}

/// Il primo numero di un frammento di testo, ignorando quello che gli sta attorno.
/// `0.6`, `= 0.7`, `| 20 |`, `0.95 (consigliato)` danno il numero; `N/A` no.
fn number(raw: &str) -> Option<f64> {
    let mut best: Option<f64> = None;
    let mut cur = String::new();
    let flush = |cur: &mut String, best: &mut Option<f64>| {
        if best.is_none() && !cur.is_empty() {
            if let Ok(n) = cur.parse::<f64>() {
                *best = Some(n);
            }
        }
        cur.clear();
    };
    for c in raw.chars() {
        if c.is_ascii_digit() || c == '.' || (c == '-' && cur.is_empty()) {
            cur.push(c);
        } else {
            flush(&mut cur, &mut best);
        }
    }
    flush(&mut cur, &mut best);
    best
}

/// Valori fuori scala non sono consigli: sono numeri finiti lì per caso.
fn plausible(lever: &str, v: f64) -> bool {
    match lever {
        "temperature" => (0.0..=2.0).contains(&v),
        "top_p" | "min_p" => (0.0..=1.0).contains(&v),
        "top_k" => (1.0..=1000.0).contains(&v) && v.fract() == 0.0,
        "presence_penalty" => (-2.0..=2.0).contains(&v),
        "repeat_penalty" => (0.0..=2.0).contains(&v),
        _ => false,
    }
}

fn put(s: &mut Sampling, lever: &str, v: f64) {
    match lever {
        "temperature" => s.temperature = Some(v),
        "top_p" => s.top_p = Some(v),
        "top_k" => s.top_k = Some(v as i32),
        "min_p" => s.min_p = Some(v),
        "presence_penalty" => s.presence_penalty = Some(v),
        "repeat_penalty" => s.repeat_penalty = Some(v),
        _ => {}
    }
}

fn is_empty(s: &Sampling) -> bool {
    s.temperature.is_none()
        && s.top_p.is_none()
        && s.top_k.is_none()
        && s.min_p.is_none()
        && s.presence_penalty.is_none()
        && s.repeat_penalty.is_none()
}

/// Modalità dichiarata da un'intestazione, se ne dichiara una.
fn mode_of(heading: &str) -> Option<&'static str> {
    let h = heading.to_ascii_lowercase();
    MODES.iter().find(|(word, _)| h.contains(word)).map(|(_, mode)| *mode)
}

/// Le coppie leva/valore di una riga, in tutte le forme in cui le model card le scrivono:
/// `Temperature = 0.6`, `temperature: 0.6`, `--temp 0.6`, `| Temperature | 0.7 |`,
/// `temperature=0.7, top_p=0.8`.
///
/// La riga si riduce a parole (i separatori sono solo punteggiatura) e si cerca una leva
/// riconosciuta seguita da vicino da un numero plausibile: due parole di distanza, non di piu,
/// così una frase che nomina la temperatura e poi cita un numero qualsiasi non diventa un consiglio.
fn pairs_in(line: &str) -> Vec<(&'static str, f64)> {
    let words: Vec<&str> = line
        .split(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | '=' | ':' | '|' | '`' | '*' | '(' | ')' | '·' | '•'))
        .filter(|w| !w.is_empty())
        .collect();
    let mut out = Vec::new();
    for (i, w) in words.iter().enumerate() {
        let Some(l) = lever(w) else { continue };
        if out.iter().any(|(k, _)| *k == l) {
            continue;
        }
        if let Some(v) = words[i + 1..].iter().take(2).find_map(|x| number(x)).filter(|v| plausible(l, *v)) {
            out.push((l, v));
        }
    }
    out
}

/// Scarica la model card di un repository di Hugging Face (`raw/main/README.md`).
/// Un repository senza README non è un errore da nascondere: lo si dice.
pub fn fetch(repo: &str) -> Result<(String, String), String> {
    let repo = repo.trim_matches('/');
    let url = format!("{}/{repo}/raw/main/README.md", crate::download::HF);
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(std::time::Duration::from_secs(20)))
        .timeout_global(Some(std::time::Duration::from_secs(60)))
        .http_status_as_error(false)
        .user_agent("Aethera (launcher llama-server)")
        .build()
        .into();
    let mut resp = agent.get(&url).call().map_err(|e| format!("{url}: {e}"))?;
    let status = resp.status().as_u16();
    if status != 200 {
        return Err(format!("{url}: risposta {status} (model card assente, repository privato o rinominato?)"));
    }
    let text = resp.body_mut().read_to_string().map_err(|e| format!("{url}: {e}"))?;
    Ok((text, format!("huggingface.co/{repo} · model card")))
}

/// Una riga che annuncia una modalità senza portare valori: un'intestazione in grassetto o una
/// voce di elenco che finisce con i due punti.
fn mode_line(t: &str) -> Option<&'static str> {
    let announces = t.contains("**") || t.trim_end().ends_with(':');
    announces.then(|| mode_of(t)).flatten()
}

fn says_sampling(t: &str) -> bool {
    let low = t.to_ascii_lowercase();
    SECTION_WORDS.iter().any(|w| low.contains(w))
}

/// Legge la model card e ne ricava il campionamento consigliato per modalità.
/// `source` è da dove viene (per esempio `huggingface.co/<repo> · model card`), `today` la data
/// in cui è stato letto: senza fonte e data un consiglio non si distingue da un'invenzione.
pub fn sample(card: &str, source: &str, today: &str) -> Found {
    let mut by_mode: BTreeMap<String, Sampling> = BTreeMap::new();
    let mut mode = "default".to_string();
    // La sezione parla di campionamento: fuori da qui i numeri non sono consigli.
    let mut relevant = false;
    let mut in_code_fence = false;

    for line in card.lines() {
        let t = line.trim();
        if t.starts_with("```") {
            in_code_fence = !in_code_fence;
            continue;
        }
        // Dentro un blocco di codice c'è un esempio, non un consiglio.
        if in_code_fence {
            continue;
        }
        if t.starts_with('#') {
            match mode_of(t) {
                Some(m) => {
                    mode = m.to_string();
                    relevant = true;
                }
                None => {
                    mode = "default".to_string();
                    relevant = says_sampling(t);
                }
            }
            continue;
        }

        let pairs = pairs_in(t);
        if pairs.is_empty() {
            if let Some(m) = mode_line(t) {
                mode = m.to_string();
                relevant = true;
            }
            continue;
        }
        // La riga può dire da sola di che modalità parla: allora vale a prescindere dalla sezione.
        let (target, accept) = match mode_of(t) {
            Some(m) => (m.to_string(), true),
            None => (mode.clone(), relevant || says_sampling(t)),
        };
        if !accept {
            continue;
        }
        let s = by_mode.entry(target).or_default();
        for (lever, v) in pairs {
            put(s, lever, v);
        }
    }

    by_mode.retain(|_, s| !is_empty(s));
    for s in by_mode.values_mut() {
        s.source = Some(source.to_string());
        s.verified = Some(today.to_string());
    }

    let mut notes = Vec::new();
    if by_mode.is_empty() {
        notes.push(format!("nessun campionamento riconosciuto in {source}: la model card non lo scrive in una forma leggibile"));
    } else {
        notes.push(format!(
            "{} modalità lette da {source} il {today}: {}",
            by_mode.len(),
            by_mode.keys().cloned().collect::<Vec<_>>().join(" · ")
        ));
        notes.push("dato del modello, non un default del server: il campionamento lo manda il client".into());
    }
    Found { sampling: by_mode, notes }
}

/// `base_model:` dichiarato nel front matter YAML, in forma singola o in elenco.
fn base_model(card: &str) -> Option<String> {
    let mut lines = card.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    let mut waiting_for_item = false;
    for line in lines {
        let t = line.trim();
        if t == "---" {
            return None;
        }
        if waiting_for_item {
            return t.strip_prefix('-').map(|v| v.trim().trim_matches('"').to_string()).filter(|v| !v.is_empty());
        }
        if let Some(v) = t.strip_prefix("base_model:") {
            let v = v.trim().trim_matches('"');
            if v.is_empty() {
                waiting_for_item = true;
                continue;
            }
            return Some(v.to_string());
        }
    }
    None
}

/// Campionamento consigliato di un repository. Una riquantizzazione (bartowski, unsloth, …) non
/// ripete i consigli del modello originale: dichiara `base_model` e basta. Allora si va a leggere
/// quella, **dicendolo**, così la fonte scritta accanto ai valori è quella giusta.
pub fn from_repo(repo: &str, today: &str) -> Result<Found, String> {
    let (card, source) = fetch(repo)?;
    let mut found = sample(&card, &source, today);
    if !found.sampling.is_empty() {
        return Ok(found);
    }
    let Some(base) = base_model(&card) else { return Ok(found) };
    if base.eq_ignore_ascii_case(repo) {
        return Ok(found);
    }
    match fetch(&base) {
        Ok((base_card, base_source)) => {
            let mut from_base = sample(&base_card, &base_source, today);
            from_base.notes.insert(
                0,
                format!("«{repo}» è una riquantizzazione e non ripete i consigli: letta la model card del modello originale, {base}"),
            );
            if from_base.sampling.is_empty() {
                found.notes.extend(from_base.notes);
                return Ok(found);
            }
            Ok(from_base)
        }
        Err(e) => {
            found.notes.push(format!("model card del modello originale «{base}» non letta: {e}"));
            Ok(found)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CARD: &str = r#"
# Qwen3.6-35B-A3B

Un modello di prova.

## Best Practices

### Thinking Mode

We recommend `Temperature = 0.6`, `TopP = 0.95`, `TopK = 20`, `MinP = 0`.

### Non-Thinking Mode

| Parameter | Value |
| --- | --- |
| Temperature | 0.7 |
| Top-P | 0.8 |
| Top-K | 20 |
| Presence Penalty | 1.5 |

## Deployment

```bash
llama-server -m model.gguf --temp 9.9 --top-p 0.1
```

Nothing else here.
"#;

    #[test]
    fn reads_one_sampling_per_mode_with_source_and_date() {
        let f = sample(CARD, "huggingface.co/prova · model card", "2026-09-16");
        assert_eq!(f.sampling.keys().cloned().collect::<Vec<_>>(), vec!["non_thinking", "thinking"]);

        let t = &f.sampling["thinking"];
        assert_eq!((t.temperature, t.top_p, t.top_k, t.min_p), (Some(0.6), Some(0.95), Some(20), Some(0.0)));
        assert_eq!(t.source.as_deref(), Some("huggingface.co/prova · model card"));
        assert_eq!(t.verified.as_deref(), Some("2026-09-16"));

        let n = &f.sampling["non_thinking"];
        assert_eq!((n.temperature, n.top_p, n.top_k, n.presence_penalty), (Some(0.7), Some(0.8), Some(20), Some(1.5)));
    }

    #[test]
    fn a_command_line_inside_a_code_fence_is_not_a_recommendation() {
        let f = sample(CARD, "x", "2026-09-16");
        // `--temp 9.9` sta in un blocco di codice di esempio: non è un consiglio di campionamento.
        assert!(f.sampling.values().all(|s| s.temperature != Some(9.9)));
        assert!(!f.sampling.contains_key("default"), "la sezione Deployment non porta campionamento");
    }

    #[test]
    fn a_card_that_says_nothing_produces_nothing_and_lo_dice() {
        let f = sample("# Modello\n\nNessun consiglio qui.\n", "carta", "2026-09-16");
        assert!(f.sampling.is_empty());
        assert!(f.notes[0].contains("nessun campionamento riconosciuto"));
    }

    #[test]
    fn implausible_numbers_are_not_advice() {
        let f = sample("## Recommended sampling\n\nTemperature: 42\nTop-P: 7\nTop-K: 20\n", "carta", "2026-09-16");
        let d = &f.sampling["default"];
        assert_eq!((d.temperature, d.top_p, d.top_k), (None, None, Some(20)));
    }

    #[test]
    fn numbers_outside_a_sampling_section_are_not_advice() {
        // I parametri con cui il publisher ha misurato un banco non sono un consiglio per chi serve.
        let card = "## Performance\n\n* SWE-Bench: agent scaffold; temp=1.0, top_p=0.95, 200K context window.\n";
        assert!(sample(card, "carta", "2026-09-16").sampling.is_empty());
    }

    #[test]
    fn a_line_that_names_its_own_mode_is_read_wherever_it_is() {
        // La forma delle model card di Qwen: tre righe in un blocco citato, una per modalità.
        let card = concat!(
            "## Note\n\n",
            "> - Thinking mode for general tasks: `temperature=1.0, top_p=0.95, top_k=20, min_p=0.0, presence_penalty=1.5`\n",
            "> - Thinking mode for precise coding tasks: `temperature=0.6, top_p=0.95, top_k=20`\n",
            "> - Instruct (or non-thinking) mode: `temperature=0.7, top_p=0.80, top_k=20`\n",
        );
        let f = sample(card, "carta", "2026-09-16");
        assert_eq!(f.sampling.keys().cloned().collect::<Vec<_>>(), vec!["coding", "non_thinking", "thinking"]);
        assert_eq!(f.sampling["thinking"].temperature, Some(1.0));
        assert_eq!(f.sampling["coding"].temperature, Some(0.6), "«precise coding» è un consiglio per il codice");
        assert_eq!(f.sampling["non_thinking"].top_p, Some(0.8));
    }

    #[test]
    fn a_mode_announced_on_the_line_before_its_values() {
        let card = "### Best Practices\n\n1. **Thinking mode** (default):\n   `temperature=1.0`, `top_p=0.95`\n";
        let f = sample(card, "carta", "2026-09-16");
        assert_eq!(f.sampling["thinking"].top_p, Some(0.95));
    }

    #[test]
    fn a_requantisation_points_at_the_original_model() {
        let card = "---\nquantized_by: bartowski\nbase_model: Qwen/Qwen3.6-35B-A3B\nlicense: apache-2.0\n---\n\n## Quants\n";
        assert_eq!(base_model(card).as_deref(), Some("Qwen/Qwen3.6-35B-A3B"));
        // Anche quando il front matter lo scrive come elenco.
        let list = "---\nbase_model:\n- Qwen/Qwen3.6-35B-A3B\n---\n";
        assert_eq!(base_model(list).as_deref(), Some("Qwen/Qwen3.6-35B-A3B"));
        // Una model card senza front matter non dichiara niente.
        assert_eq!(base_model("# Modello\n"), None);
    }

    #[test]
    fn values_written_on_one_line_are_read_too() {
        let f = sample("Sampling: temperature=0.7, top_p=0.8, repetition_penalty=1.05\n", "carta", "2026-09-16");
        let d = &f.sampling["default"];
        assert_eq!((d.temperature, d.top_p, d.repeat_penalty), (Some(0.7), Some(0.8), Some(1.05)));
    }
}
