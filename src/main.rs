mod app;
mod detect;
mod gh;
mod git_status;
mod git_worker;
mod theme;
mod ports;
mod process;
mod scanner;
mod store;
mod tui;
mod ui;

use crate::store::{ProjectEntry, ProjectStore};

fn main() -> anyhow::Result<()> {
    let mut store = ProjectStore::load()?;
    let first_run = store.is_first_run();

    // On first run, offer to scan for projects
    if first_run {
        run_first_time_setup(&mut store)?;
    }

    // Check gh auth (non-fatal)
    let gh_available = gh::check_auth();

    let mut app = app::App::new(store, gh_available)?;
    let mut tui = tui::Tui::new()?;
    tui.run(&mut app)?;

    Ok(())
}

fn run_first_time_setup(store: &mut ProjectStore) -> anyhow::Result<()> {
    use std::io::{self, Write};

    println!("Welcome to Claude Manager!\n");
    print!("Scan for existing git repos in common directories? [Y/n] ");
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
                store.add(ProjectEntry {
                    name: proj.name.clone(),
                    repo_url: proj.remote_url.clone(),
                    path: proj.path.to_string_lossy().to_string(),
                    run_command: None,
                });
                println!("  Imported: {}", proj.name);
            }
        }
    }

    store.save()?;
    println!("\nSetup complete! Starting Claude Manager...\n");
    Ok(())
}
