//! `ptpnhex` — command-line interface for the PTPNHEX save editor.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use ptpnhex_core::keys::KeyProvider;
use ptpnhex_core::save::materials::MATERIAL_MAX;
use ptpnhex_core::save::Material;
use ptpnhex_core::SaveSlot;

/// Save editor for Patapon (PSP).
#[derive(Parser)]
#[command(name = "ptpnhex", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Show information about a save directory.
    Info {
        /// Path to the save directory (for example `.../UCES00995_DATA01`).
        dir: PathBuf,
    },
    /// Set the ka-ching (currency) value and write the save back.
    SetKaching {
        /// Path to the save directory.
        dir: PathBuf,
        /// New ka-ching value (0–99999).
        value: u32,
        /// Do not write `*.bak` backups of the originals.
        #[arg(long)]
        no_backup: bool,
    },
    /// List the save's crafting materials and their counts.
    Materials {
        /// Path to the save directory.
        dir: PathBuf,
    },
    /// Set a material's count (0–99) and write the save back.
    SetMaterial {
        /// Path to the save directory.
        dir: PathBuf,
        /// Material slug (for example `hard-alloy`), or `all` for every
        /// material present in the save.
        material: String,
        /// New count (0–99).
        value: u32,
        /// Do not write `*.bak` backups of the originals.
        #[arg(long)]
        no_backup: bool,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Info { dir } => info(&dir),
        Command::SetKaching {
            dir,
            value,
            no_backup,
        } => set_kaching(&dir, value, no_backup),
        Command::Materials { dir } => materials(&dir),
        Command::SetMaterial {
            dir,
            material,
            value,
            no_backup,
        } => set_material(&dir, &material, value, no_backup),
    }
}

fn open(dir: &Path) -> Result<SaveSlot> {
    SaveSlot::open(dir, &KeyProvider::Embedded)
        .with_context(|| format!("opening save {}", dir.display()))
}

fn info(dir: &Path) -> Result<()> {
    let slot = open(dir)?;
    println!("Region:   {}", slot.region().serial());
    if let Some(title) = slot.sfo().get_str("SAVEDATA_TITLE") {
        println!("Title:    {title}");
    }
    if let Some(detail) = slot.sfo().get_str("SAVEDATA_DETAIL") {
        println!("Detail:   {}", detail.trim().replace('\n', " / "));
    }
    match slot.kaching() {
        Some(k) => println!("Ka-ching: {k}"),
        None => println!("Ka-ching: (not mapped for this region/save)"),
    }
    Ok(())
}

fn set_kaching(dir: &Path, value: u32, no_backup: bool) -> Result<()> {
    let mut slot = open(dir)?;
    let before = slot.kaching();
    slot.set_kaching(value)?;
    if no_backup {
        slot.save_without_backup()?;
    } else {
        slot.save()?;
    }
    match before {
        Some(old) => println!("Ka-ching: {old} -> {value}"),
        None => println!("Ka-ching set to {value}"),
    }
    if !no_backup {
        println!("Originals backed up to *.bak");
    }
    Ok(())
}

fn materials(dir: &Path) -> Result<()> {
    let slot = open(dir)?;
    for (material, count) in slot.materials() {
        println!("{:18} {count}", material.name());
    }
    Ok(())
}

fn set_material(dir: &Path, material: &str, value: u32, no_backup: bool) -> Result<()> {
    if value > MATERIAL_MAX {
        bail!("count {value} exceeds the maximum of {MATERIAL_MAX}");
    }
    let mut slot = open(dir)?;

    let edited = if material == "all" {
        // Set every material present in the save; absent ones are skipped.
        let n = Material::all()
            .filter(|&m| slot.set_material(m, value).is_ok())
            .count();
        if n == 0 {
            bail!("no editable materials found in this save");
        }
        format!("{n} materials")
    } else {
        let m = Material::from_slug(material)
            .with_context(|| format!("unknown material `{material}` (try `materials` to list)"))?;
        slot.set_material(m, value)?;
        m.name().to_string()
    };

    if no_backup {
        slot.save_without_backup()?;
    } else {
        slot.save()?;
    }
    println!("{edited}: set to {value}");
    if !no_backup {
        println!("Originals backed up to *.bak");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }
}
