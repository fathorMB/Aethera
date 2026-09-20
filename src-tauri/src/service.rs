//! Profili di servizio (M-20): `llama-server` piccoli accesi accanto al motore principale.
//!
//! Un motore di servizio **serve**, non viene misurato. Non ha manifest, telemetria per richiesta,
//! stato «in uso» né storico in Benchmark: quelle cose esistono per confrontare due avvii dello
//! stesso motore, e un servizio non si confronta con niente — o risponde o no.
//!
//! Per questo il suo profilo è un tipo a parte e non una variante di [`crate::profile::Profile`]:
//! ha **meno** campi, non un campo in più. Le leve che un servizio non può avere (speculazione,
//! checkpoint e riuso della cache, budget di contesto del client, salvataggio degli slot,
//! campionamento consigliato) non sono presenti e non si possono nemmeno scrivere:
//! `deny_unknown_fields` le rifiuta al caricamento, dicendo quale campo non appartiene qui.
//!
//! Resta invece la regola di sempre per quello che c'è: contesto, porta e layer sono espliciti, e
//! la riga di comando che ne esce si legge per intero. Dove si accetta il default del motore —
//! ubatch, batch, flash attention, tipo di cache — è perché su un modello da 0,6 GB quelle leve
//! non spostano niente di misurabile, e questo commento è la dichiarazione che il default è una
//! scelta e non una dimenticanza.
//!
//! Perché un processo separato invece di mandare il lavoro al motore principale: il riuso del
//! prefisso è tutto-o-niente (misura del 20-09) e il profilo principale ha `n_parallel = 1`;
//! infilare un'altra conversazione nello stesso slot farebbe ripartire da zero il client che sta
//! lavorando, e a 262144 un prefill a freddo arriva a ~28 minuti.

use crate::profile::Issue;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: u32 = 1;

/// Il valore di `role` che distingue un profilo di servizio da uno principale.
pub const ROLE: &str = "service";

/// Che cosa serve un motore di servizio. Decide i flag che lo rendono quel servizio e basta.
pub const KINDS: &[&str] = &["embedding", "rerank", "chat"];

fn one() -> u32 {
    1
}
fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ServiceProfile {
    pub schema_version: u32,
    pub name: String,
    /// Sempre `"service"`: è il campo su cui si distingue il file senza leggerlo tutto.
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub model: Model,
    pub service: Service,
    pub runtime: Runtime,
    pub server: Server,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Model {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    pub file: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_gb: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quant: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Service {
    /// `embedding`, `rerank` o `chat`.
    pub kind: String,
    /// Si accende insieme al motore principale. Il valore giusto lo decide M-20 T-02: se tenere
    /// un servizio carico costa al principale, questo diventa `false` di serie e i servizi si
    /// accendono a richiesta.
    #[serde(default = "yes")]
    pub autostart: bool,
    /// Numero di dimensioni attese dall'embedding. Non cambia l'avvio: serve a far fallire presto
    /// una configurazione sbagliata, perché chi consuma i vettori (GalaxyCenter) le fissa in
    /// configurazione e cambiarle obbliga a reindicizzare tutto.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embed_dim: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Runtime {
    pub kind: String,
    pub backend: String,
    pub build: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Server {
    pub host: String,
    pub port: u16,
    pub ctx: u32,
    #[serde(default = "one")]
    pub n_parallel: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub n_gpu_layers: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threads: Option<i32>,
    #[serde(default = "yes")]
    pub metrics: bool,
}

fn issue(field: &str, message: impl Into<String>) -> Issue {
    Issue { field: field.into(), message: message.into() }
}

/// Dice se un file di `profiles/` è un profilo di servizio, senza deserializzarlo tutto: così un
/// profilo principale illeggibile non diventa «servizio» per sbaglio, e viceversa.
pub fn is_service(text: &str) -> bool {
    #[derive(Deserialize)]
    struct Peek {
        #[serde(default)]
        role: String,
    }
    toml::from_str::<Peek>(text).map(|p| p.role == ROLE).unwrap_or(false)
}

pub fn validate(p: &ServiceProfile) -> Vec<Issue> {
    let mut v = Vec::new();
    if p.schema_version != SCHEMA_VERSION {
        v.push(issue("schema_version", format!("atteso {SCHEMA_VERSION}")));
    }
    if p.name.is_empty() || !p.name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')) {
        v.push(issue("name", "nome vuoto o con caratteri non ammessi (lettere, cifre, . - _)"));
    }
    if p.role != ROLE {
        v.push(issue("role", format!("un profilo di servizio ha role = «{ROLE}»")));
    }
    if !KINDS.contains(&p.service.kind.as_str()) {
        v.push(issue("service.kind", format!("«{}» non ammesso: {}", p.service.kind, KINDS.join(" · "))));
    }
    if p.service.embed_dim.is_some() && p.service.kind != "embedding" {
        v.push(issue("service.embed_dim", "ha senso solo per un servizio di embedding"));
    }
    if p.service.kind == "embedding" && p.service.embed_dim.is_none() {
        v.push(issue(
            "service.embed_dim",
            "dichiara le dimensioni attese: chi consuma i vettori le fissa, e cambiarle obbliga a reindicizzare tutto",
        ));
    }

    let f = &p.model.file;
    if f.trim().is_empty() || f.contains(['/', '\\']) {
        v.push(issue("model.file", "solo il nome del file, senza percorso: la cartella dei pesi è della macchina"));
    } else if !f.to_ascii_lowercase().ends_with(".gguf") {
        v.push(issue("model.file", "atteso un file .gguf"));
    }
    if let Some(h) = &p.model.sha256 {
        if h.len() != 64 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
            v.push(issue("model.sha256", "atteso uno SHA-256 di 64 cifre esadecimali"));
        }
    }

    if p.runtime.kind != "llama.cpp" {
        v.push(issue("runtime.kind", "obbligatorio (llama.cpp)"));
    }
    if p.runtime.backend.trim().is_empty() {
        v.push(issue("runtime.backend", "obbligatorio (per esempio vulkan)"));
    }
    if p.runtime.build.trim().is_empty() {
        v.push(issue("runtime.build", "obbligatorio: il motore è scelto e fissato (per esempio b10809)"));
    }

    let s = &p.server;
    if s.host.trim().is_empty() {
        v.push(issue("server.host", "obbligatorio"));
    }
    if s.port == 0 {
        v.push(issue("server.port", "porta esplicita obbligatoria: il default di llama-server cambierà"));
    }
    if s.ctx < 256 {
        v.push(issue("server.ctx", "contesto troppo piccolo (minimo 256)"));
    }
    if s.n_parallel == 0 {
        v.push(issue("server.n_parallel", "almeno 1 slot"));
    }
    if matches!(s.threads, Some(n) if n == 0 || n < -1) {
        v.push(issue("server.threads", "atteso -1 (automatico) o un numero positivo"));
    }
    v
}

/// Legge un profilo di servizio. Come per i profili principali, restituisce il profilo anche se ha
/// errori di validazione, così la finestra può mostrarlo e farlo correggere.
pub fn load(text: &str, stem: &str) -> (Option<ServiceProfile>, Vec<Issue>) {
    let p: ServiceProfile = match toml::from_str(text) {
        Ok(p) => p,
        Err(e) => return (None, vec![issue("", e.to_string())]),
    };
    let mut issues = validate(&p);
    if p.name != stem {
        issues.insert(
            0,
            issue(
                "name",
                format!("il nome «{}» non coincide con il file «{stem}.toml»: l'alias servito è il nome del profilo", p.name),
            ),
        );
    }
    (Some(p), issues)
}

pub fn save(p: &ServiceProfile) -> Result<String, String> {
    toml::to_string_pretty(p).map_err(|e| e.to_string())
}

/// Riga di comando di un servizio: corta per costruzione, ma scritta per intero come quella del
/// motore principale.
pub fn build_args(p: &ServiceProfile, model: &Path) -> Vec<String> {
    let s = &p.server;
    let mut a: Vec<String> = Vec::new();
    let mut kv = |flag: &str, value: String| {
        a.push(flag.to_string());
        a.push(value);
    };
    kv("--model", model.display().to_string());
    kv("--host", s.host.clone());
    kv("--port", s.port.to_string());
    kv("--ctx-size", s.ctx.to_string());
    kv("--parallel", s.n_parallel.to_string());
    if let Some(n) = s.n_gpu_layers {
        kv("--n-gpu-layers", n.to_string());
    }
    if let Some(t) = s.threads {
        kv("--threads", t.to_string());
    }
    kv("--alias", p.name.clone());
    match p.service.kind.as_str() {
        // `--pooling last` è quello che vuole Qwen3-Embedding: senza, i vettori escono da una
        // media e non corrispondono a quelli con cui l'indice è stato costruito.
        "embedding" => {
            a.push("--embedding".into());
            a.push("--pooling".into());
            a.push("last".into());
        }
        // `--reranking` accende `/v1/rerank`. Il pooling lo decide il modello: la conversione
        // ufficiale lo porta dentro, e quella della comunità spesso no — per questo dopo l'avvio
        // ci vuole il test di sanità, non solo un `/health` verde.
        "rerank" => a.push("--reranking".into()),
        // Una chat serve a un agente, quindi le servono gli strumenti, quindi il template.
        "chat" => a.push("--jinja".into()),
        _ => {}
    }
    if s.metrics {
        a.push("--metrics".into());
    }
    a
}

/// Percorso dei pesi di un servizio nella cartella della macchina.
pub fn model_path(models_dir: &Path, p: &ServiceProfile) -> PathBuf {
    models_dir.join(&p.model.file)
}

pub fn base_url(p: &ServiceProfile) -> String {
    format!("http://{}:{}", p.server.host, p.server.port)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMBED: &str = r#"
schema_version = 1
name = "qwen3-embedding-0.6b.q8"
role = "service"

[model]
file = "Qwen3-Embedding-0.6B-Q8_0.gguf"
sha256 = "06507c7b42688469c4e7298b0a1e16deff06caf291cf0a5b278c308249c3e439"

[service]
kind = "embedding"
embed_dim = 1024

[runtime]
kind = "llama.cpp"
backend = "vulkan"
build = "b10809"

[server]
host = "127.0.0.1"
port = 8081
ctx = 8192
n_gpu_layers = 999
"#;

    #[test]
    fn un_servizio_di_embedding_si_legge_e_vale() {
        let (p, issues) = load(EMBED, "qwen3-embedding-0.6b.q8");
        let p = p.expect("leggibile");
        assert!(issues.is_empty(), "{issues:?}");
        assert_eq!(p.service.kind, "embedding");
        assert_eq!(p.service.embed_dim, Some(1024));
        assert!(p.service.autostart, "di serie un servizio si accende col principale");
        assert_eq!(p.server.n_parallel, 1);
    }

    #[test]
    fn la_riga_di_comando_dice_che_servizio_e() {
        let (p, _) = load(EMBED, "qwen3-embedding-0.6b.q8");
        let a = build_args(&p.unwrap(), Path::new(r"X:\pesi\Qwen3-Embedding-0.6B-Q8_0.gguf"));
        let riga = a.join(" ");
        assert!(riga.contains("--embedding --pooling last"), "{riga}");
        assert!(riga.contains("--port 8081"), "{riga}");
        assert!(riga.contains("--alias qwen3-embedding-0.6b.q8"), "{riga}");
        // Le leve del motore principale non devono comparire: un servizio non specula e non
        // salva slot, e la riga è la prova che non lo fa.
        for vietato in ["--spec-type", "--slot-save-path", "--ctx-checkpoints", "--cache-type-k"] {
            assert!(!riga.contains(vietato), "«{vietato}» non appartiene a un servizio: {riga}");
        }
    }

    #[test]
    fn rerank_e_chat_hanno_i_loro_flag() {
        let rerank = EMBED.replace("kind = \"embedding\"", "kind = \"rerank\"").replace("embed_dim = 1024\n", "");
        let (p, issues) = load(&rerank, "qwen3-embedding-0.6b.q8");
        assert!(issues.is_empty(), "{issues:?}");
        let riga = build_args(&p.unwrap(), Path::new("X:/m.gguf")).join(" ");
        assert!(riga.contains("--reranking"), "{riga}");
        assert!(!riga.contains("--embedding"), "{riga}");

        let chat = EMBED.replace("kind = \"embedding\"", "kind = \"chat\"").replace("embed_dim = 1024\n", "");
        let (p, _) = load(&chat, "qwen3-embedding-0.6b.q8");
        let riga = build_args(&p.unwrap(), Path::new("X:/m.gguf")).join(" ");
        assert!(riga.contains("--jinja"), "una chat di ruolo ha bisogno degli strumenti: {riga}");
    }

    #[test]
    fn una_leva_del_motore_principale_viene_rifiutata_dicendo_quale() {
        let con_spec = format!("{EMBED}\n[speculative]\ntype = \"draft-mtp\"\n");
        let (p, issues) = load(&con_spec, "qwen3-embedding-0.6b.q8");
        assert!(p.is_none(), "un servizio con la speculazione non deve nemmeno leggersi");
        assert!(issues[0].message.contains("speculative"), "deve dire quale campo: {:?}", issues[0]);
    }

    #[test]
    fn un_embedding_senza_dimensioni_dichiarate_e_un_errore() {
        let senza = EMBED.replace("embed_dim = 1024\n", "");
        let (_, issues) = load(&senza, "qwen3-embedding-0.6b.q8");
        assert!(
            issues.iter().any(|i| i.field == "service.embed_dim"),
            "cambiarle obbliga a reindicizzare: vanno dichiarate, {issues:?}"
        );
    }

    #[test]
    fn is_service_distingue_i_due_tipi_di_file() {
        assert!(is_service(EMBED));
        assert!(!is_service("schema_version = 1\nname = \"x\"\n"));
        assert!(!is_service("non e' toml ["), "un file rotto non diventa un servizio per sbaglio");
    }

    #[test]
    fn il_nome_deve_coincidere_col_file() {
        let (_, issues) = load(EMBED, "un-altro-nome");
        assert_eq!(issues[0].field, "name");
    }
}
