//! `ptpnhex` — command-line interface for the PTPNHEX save editor.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use ptpnhex_core::keys::KeyProvider;
use ptpnhex_core::save::items::ITEM_MAX;
use ptpnhex_core::save::materials::MATERIAL_MAX;
use ptpnhex_core::save::{Item, Material};
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
        /// Copy the original files into this directory before saving.
        /// Must be outside the save directory.
        #[arg(long, value_name = "DIR")]
        backup_dir: Option<PathBuf>,
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
        /// Copy the original files into this directory before saving.
        /// Must be outside the save directory.
        #[arg(long, value_name = "DIR")]
        backup_dir: Option<PathBuf>,
    },
    /// List the save's items (stews, Memories, weapons, gear) and their counts.
    Items {
        /// Path to the save directory.
        dir: PathBuf,
    },
    /// Set an item's count (0–99), adding it if never obtained, and write back.
    SetItem {
        /// Path to the save directory.
        dir: PathBuf,
        /// Item slug (for example `divine-sword`), or `all` for every item.
        item: String,
        /// New count (0–99).
        value: u32,
        /// Copy the original files into this directory before saving.
        /// Must be outside the save directory.
        #[arg(long, value_name = "DIR")]
        backup_dir: Option<PathBuf>,
    },
    /// Set the save's title — the bold line shown in the PSP save list.
    SetTitle {
        /// Path to the save directory.
        dir: PathBuf,
        /// New title text.
        title: String,
        /// Copy the original files into this directory before saving.
        /// Must be outside the save directory.
        #[arg(long, value_name = "DIR")]
        backup_dir: Option<PathBuf>,
    },
    /// Set the save's detail — the smaller line shown in the PSP save list.
    ///
    /// Note: the game regenerates this from its own data the next time it
    /// saves, so this changes the displayed label, not the in-game values.
    SetDetail {
        /// Path to the save directory.
        dir: PathBuf,
        /// New detail text.
        detail: String,
        /// Copy the original files into this directory before saving.
        /// Must be outside the save directory.
        #[arg(long, value_name = "DIR")]
        backup_dir: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Info { dir } => info(&dir),
        Command::SetKaching {
            dir,
            value,
            backup_dir,
        } => set_kaching(&dir, value, backup_dir.as_deref()),
        Command::Materials { dir } => materials(&dir),
        Command::SetMaterial {
            dir,
            material,
            value,
            backup_dir,
        } => set_material(&dir, &material, value, backup_dir.as_deref()),
        Command::Items { dir } => items(&dir),
        Command::SetItem {
            dir,
            item,
            value,
            backup_dir,
        } => set_item(&dir, &item, value, backup_dir.as_deref()),
        Command::SetTitle {
            dir,
            title,
            backup_dir,
        } => set_label(
            &dir,
            "SAVEDATA_TITLE",
            "Title",
            &title,
            backup_dir.as_deref(),
        ),
        Command::SetDetail {
            dir,
            detail,
            backup_dir,
        } => set_label(
            &dir,
            "SAVEDATA_DETAIL",
            "Detail",
            &detail,
            backup_dir.as_deref(),
        ),
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

/// Sets a `PARAM.SFO` display string (`SAVEDATA_TITLE` / `SAVEDATA_DETAIL`).
fn set_label(
    dir: &Path,
    key: &str,
    label: &str,
    value: &str,
    backup_dir: Option<&Path>,
) -> Result<()> {
    let mut slot = open(dir)?;
    slot.sfo_mut()
        .set_str(key, value)
        .with_context(|| format!("setting {label} to {value:?}"))?;
    back_up_and_save(&slot, backup_dir)?;
    println!("{label} set to: {value}");
    Ok(())
}

/// Backs up the originals if a destination was given, then saves.
fn back_up_and_save(slot: &SaveSlot, backup_dir: Option<&Path>) -> Result<()> {
    if let Some(dest) = backup_dir {
        slot.back_up_to(dest)
            .with_context(|| format!("backing up originals to {}", dest.display()))?;
        println!("Originals backed up to {}", dest.display());
    }
    slot.save()?;
    Ok(())
}

fn set_kaching(dir: &Path, value: u32, backup_dir: Option<&Path>) -> Result<()> {
    let mut slot = open(dir)?;
    let before = slot.kaching();
    slot.set_kaching(value)?;
    back_up_and_save(&slot, backup_dir)?;
    match before {
        Some(old) => println!("Ka-ching: {old} -> {value}"),
        None => println!("Ka-ching set to {value}"),
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

fn set_material(dir: &Path, material: &str, value: u32, backup_dir: Option<&Path>) -> Result<()> {
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

    back_up_and_save(&slot, backup_dir)?;
    println!("{edited}: set to {value}");
    Ok(())
}

fn items(dir: &Path) -> Result<()> {
    let slot = open(dir)?;
    let mut category = "";
    for (item, count) in slot.items() {
        if item.category() != category {
            category = item.category();
            println!("[{category}]");
        }
        println!("  {:28} {count}", item.name());
    }
    Ok(())
}

fn set_item(dir: &Path, item: &str, value: u32, backup_dir: Option<&Path>) -> Result<()> {
    if value > ITEM_MAX {
        bail!("count {value} exceeds the maximum of {ITEM_MAX}");
    }
    let mut slot = open(dir)?;

    let edited = if item == "all" {
        for i in Item::all() {
            slot.set_item(i, value)?;
        }
        format!("{} items", Item::all().count())
    } else {
        let i = Item::from_slug(item)
            .with_context(|| format!("unknown item `{item}` (try `items` to list)"))?;
        slot.set_item(i, value)?;
        i.name().to_string()
    };

    back_up_and_save(&slot, backup_dir)?;
    println!("{edited}: set to {value}");
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
