// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod commands;

use std::path::PathBuf;

use jsoncv_core::{config, library::Library};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // MCP mode must be decided before anything Tauri runs: no window, nothing on stdout but JSON-RPC.
    if args.get(1).map(String::as_str) == Some("mcp") {
        if let Err(e) = run_mcp(&args[2..]) {
            eprintln!("jsoncv-desktop mcp: {e:#}");
            std::process::exit(1);
        }
        return;
    }
    app::run();
}

fn run_mcp(args: &[String]) -> anyhow::Result<()> {
    let dir = match args {
        [] => config::load().library,
        [flag, dir] if flag == "--dir" => PathBuf::from(dir),
        _ => anyhow::bail!("usage: jsoncv-desktop mcp [--dir <path>]"),
    };
    let lib = Library::open(&dir)?;
    tokio::runtime::Runtime::new()?.block_on(jsoncv_mcp::serve_stdio(lib))
}
