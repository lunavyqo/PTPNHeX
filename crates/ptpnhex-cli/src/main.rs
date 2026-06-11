//! `ptpnhex` — command-line interface for the PTPNHEX save editor.

use clap::Parser;

/// Save editor for Patapon (PSP).
#[derive(Parser)]
#[command(name = "ptpnhex", version, about, long_about = None)]
struct Cli {}

fn main() -> anyhow::Result<()> {
    let _cli = Cli::parse();
    println!("ptpnhex: no commands implemented yet; see --help");
    Ok(())
}
