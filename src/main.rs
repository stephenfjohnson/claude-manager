mod errors;
mod machine;

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

    if cli.init {
        let machine_id = machine::get_or_create_machine_id()?;
        println!("Machine ID: {}", machine_id);
    } else {
        match machine::get_machine_id()? {
            Some(id) => println!("Machine ID: {}", id),
            None => println!("Not initialized. Run with --init first."),
        }
    }

    Ok(())
}
