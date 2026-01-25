mod app;
mod config;
mod db;
mod detect;
mod errors;
mod gh;
mod git_status;
mod machine;
mod ports;
mod process;
mod scanner;
mod sync;
mod tui;
mod ui;

use crate::errors::AppError;
use clap::Parser;

#[derive(Parser)]
#[command(name = "claude-manager")]
#[command(about = "Personal project dashboard across machines")]
struct Cli {
    /// Initialize Claude Manager (run once per machine)
    #[arg(long)]
    init: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Always check gh auth first
    if !gh::check_auth()? {
        return Err(AppError::GhNotAuthenticated.into());
    }

    if cli.init {
        run_init()?;
    } else {
        run_tui()?;
    }

    Ok(())
}

fn run_init() -> anyhow::Result<()> {
    use std::io::{self, Write};

    println!("Initializing Claude Manager...\n");

    let machine_id = machine::get_or_create_machine_id()?;
    println!("Machine ID: {}", machine_id);

    sync::init()?;
    println!("Sync repo ready.");

    let db_path = sync::db_path()?;
    let db = db::Database::open(&db_path)?;
    println!("Database initialized.");

    // Offer to scan for projects
    print!("\nScan for existing git repos in common directories? [Y/n] ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    if input.trim().is_empty() || input.trim().to_lowercase() == "y" {
        println!("\nScanning...");
        let found = scanner::scan_directories();

        if found.is_empty() {
            println!("No git repositories found.");
        } else {
            println!("\nFound {} repositories:\n", found.len());

            for (i, proj) in found.iter().enumerate() {
                let url = proj.remote_url.as_deref().unwrap_or("(no remote)");
                println!("  [{}] {} - {}", i + 1, proj.name, url);
            }

            print!("\nEnter numbers to import (comma-separated), 'all', or 'none': ");
            io::stdout().flush()?;
            input.clear();
            io::stdin().read_line(&mut input)?;

            let to_import: Vec<usize> = if input.trim() == "all" {
                (0..found.len()).collect()
            } else if input.trim() == "none" {
                Vec::new()
            } else {
                input
                    .trim()
                    .split(',')
                    .filter_map(|s| s.trim().parse::<usize>().ok())
                    .filter(|&n| n > 0 && n <= found.len())
                    .map(|n| n - 1)
                    .collect()
            };

            for idx in to_import {
                let proj = &found[idx];
                let url = proj.remote_url.as_deref().unwrap_or("");

                match db.add_project(&proj.name, url) {
                    Ok(id) => {
                        db.set_location(id, &machine_id, proj.path.to_str().unwrap_or(""))?;
                        println!("  Imported: {}", proj.name);
                    }
                    Err(e) => {
                        println!("  Skipped {} ({})", proj.name, e);
                    }
                }
            }

            // Sync to GitHub
            sync::push("Import projects from scan")?;
            println!("\nProjects synced to GitHub.");
        }
    }

    println!("\nInitialization complete! Run 'claude-manager' to start.");
    Ok(())
}

fn run_tui() -> anyhow::Result<()> {
    if !sync::is_initialized()? {
        return Err(AppError::NotInitialized.into());
    }

    sync::pull()?;

    let machine_id = machine::get_machine_id()?.ok_or(AppError::NotInitialized)?;
    let db_path = sync::db_path()?;
    let db = db::Database::open(&db_path)?;

    let mut app = app::App::new(db, machine_id)?;
    let mut tui = tui::Tui::new()?;
    tui.run(&mut app)?;

    Ok(())
}
