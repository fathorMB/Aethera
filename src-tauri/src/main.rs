// Nessuna console accanto alla finestra nelle build di rilascio.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    aethera_lib::run()
}
