// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[global_allocator]
static GLOBAL: _scaffold_lib::monitoring::CountingAlloc = _scaffold_lib::monitoring::CountingAlloc;

fn main() {
    _scaffold_lib::run()
}
