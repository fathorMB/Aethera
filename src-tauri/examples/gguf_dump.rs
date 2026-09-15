//! Legge l'intestazione dei GGUF veri e stampa quello che il catalogo mostrerà (M-04 T-01).
//! Serve anche a vedere quali chiavi dichiara davvero un file, senza caricare il modello.
//!
//! Uso: `cargo run --example gguf_dump -- <file.gguf> [altri.gguf …]`

use aethera_lib::gguf::{self, Value};
use std::path::PathBuf;
use std::time::Instant;

fn short(v: &Value) -> String {
    match v {
        Value::Str(s) if s.chars().count() > 60 => {
            format!("«{}…» ({} caratteri)", s.chars().take(60).collect::<String>(), s.chars().count())
        }
        Value::Str(s) => format!("«{s}»"),
        Value::Arr(a) => format!("[{} elementi] {a:?}", a.len()),
        Value::Skipped { elements } => format!("[{elements} elementi, saltati]"),
        other => format!("{other:?}"),
    }
}

fn main() -> Result<(), String> {
    let files: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    if files.is_empty() {
        return Err("uso: gguf_dump <file.gguf> […]".into());
    }
    for path in files {
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        println!("\n=== {} · {:.2} GB ===", path.display(), size as f64 / 1e9);
        let t = Instant::now();
        let g = match gguf::read(&path) {
            Ok(g) => g,
            Err(e) => {
                println!("non leggibile: {e}");
                continue;
            }
        };
        let info = gguf::info(&g);
        println!(
            "letto in {} ms · GGUF v{} · {} tensori · {} chiavi",
            t.elapsed().as_millis(),
            g.version,
            g.tensor_count,
            g.kv.len()
        );
        println!("--- chiavi ---");
        for (k, v) in &g.kv {
            println!("  {k} = {}", short(v));
        }
        println!("--- quello che vedrà il catalogo ---");
        println!("{info:#?}");
    }
    Ok(())
}
