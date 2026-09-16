//! Stampa le condizioni della macchina come finirebbero nel manifest di un avvio.
//! Uso: cargo run --example conditions -- [cartella dei pesi]

use aethera_lib::system;
use std::path::PathBuf;

fn main() {
    let dir = std::env::args().nth(1).map(PathBuf::from);
    let c = system::probe().conditions(dir.as_deref());
    print!("{}", toml::to_string_pretty(&c).unwrap());
    println!("# {}", c.short());
}
