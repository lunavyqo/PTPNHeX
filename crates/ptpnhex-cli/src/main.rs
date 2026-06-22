//! `ptpnhex` — command-line interface for the PTPNHeX save editor.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use ptpnhex_core::keys::KeyProvider;
use ptpnhex_core::save::items::ITEM_MAX;
use ptpnhex_core::save::materials::MATERIAL_MAX;
use ptpnhex_core::save::{BonusPatapon, Item, KeyItem, Material, Rarepon};
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
    /// List the save's key items (drums, miracles, songs, quest items).
    KeyItems {
        /// Path to the save directory.
        dir: PathBuf,
    },
    /// Unlock or lock a key item (drum, miracle, song, quest item).
    SetKeyItem {
        /// Path to the save directory.
        dir: PathBuf,
        /// Key-item slug (for example `earthquake-miracle`), or `all`.
        key_item: String,
        /// `on` to unlock, `off` to lock.
        state: String,
        /// Copy the original files into this directory before saving.
        /// Must be outside the save directory.
        #[arg(long, value_name = "DIR")]
        backup_dir: Option<PathBuf>,
    },
    /// List the army roster: each unit's index, class, and rarepon.
    Units {
        /// Path to the save directory.
        dir: PathBuf,
    },
    /// Set a unit's rarepon by its roster index (see `units`).
    SetRarepon {
        /// Path to the save directory.
        dir: PathBuf,
        /// Roster index of the unit (from `units`).
        index: usize,
        /// Rarepon slug (for example `mogyoon`), or `list` to show the choices.
        rarepon: String,
        /// Copy the original files into this directory before saving.
        /// Must be outside the save directory.
        #[arg(long, value_name = "DIR")]
        backup_dir: Option<PathBuf>,
    },
    /// Set a unit's weapon tier by its roster index (see `units`). Grants the
    /// weapon in inventory so it stays equipped.
    SetWeapon {
        /// Path to the save directory.
        dir: PathBuf,
        /// Roster index of the unit (from `units`).
        index: usize,
        /// Weapon tier (1 = basic … 8 = Divine; Tatepon swords reach 9), or `max`.
        tier: String,
        /// Copy the original files into this directory before saving.
        /// Must be outside the save directory.
        #[arg(long, value_name = "DIR")]
        backup_dir: Option<PathBuf>,
    },
    /// Open or close the mission-prep loadout slots (miracle and stew, together).
    SetLoadoutSlots {
        /// Path to the save directory.
        dir: PathBuf,
        /// `on` to open the slots, `off` to close them.
        state: String,
        /// Copy the original files into this directory before saving.
        /// Must be outside the save directory.
        #[arg(long, value_name = "DIR")]
        backup_dir: Option<PathBuf>,
    },
    /// Force every confirmed unlock: all drums, buildable units, missions, and
    /// boss missions (does not open the loadout slots — use `set-loadout-slots`).
    UnlockAll {
        /// Path to the save directory.
        dir: PathBuf,
        /// Copy the original files into this directory before saving.
        /// Must be outside the save directory.
        #[arg(long, value_name = "DIR")]
        backup_dir: Option<PathBuf>,
    },
    /// List the bonus Patapons (Patapolis revivals) and whether each is revived.
    BonusPatapons {
        /// Path to the save directory.
        dir: PathBuf,
    },
    /// Revive or remove a bonus Patapon, toggling its minigame (and, for Kimpon,
    /// Kibapon production).
    SetBonusPatapon {
        /// Path to the save directory.
        dir: PathBuf,
        /// Bonus-Patapon slug (for example `kimpon`), or `all`.
        patapon: String,
        /// `on` to revive, `off` to remove.
        state: String,
        /// Copy the original files into this directory before saving.
        /// Must be outside the save directory.
        #[arg(long, value_name = "DIR")]
        backup_dir: Option<PathBuf>,
    },
    /// Mark a bonus Patapon's one-time intro dialog as seen, or clear it so the
    /// intro replays on the next interaction (cosmetic; does not affect the revive).
    SetDialogSeen {
        /// Path to the save directory.
        dir: PathBuf,
        /// Bonus-Patapon slug (for example `kimpon`), or `all`.
        patapon: String,
        /// `on` = seen (intro will not play), `off` = unseen (intro replays).
        state: String,
        /// Copy the original files into this directory before saving.
        /// Must be outside the save directory.
        #[arg(long, value_name = "DIR")]
        backup_dir: Option<PathBuf>,
    },
    /// Mark a bonus Patapon's minigame as played or never-played (cosmetic; does
    /// not affect the revive or minigame availability).
    SetMinigamePlayed {
        /// Path to the save directory.
        dir: PathBuf,
        /// Bonus-Patapon slug (for example `kimpon`), or `all`.
        patapon: String,
        /// `on` = played, `off` = never played.
        state: String,
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
    /// Set the player's name (the "Almighty" name) in the game data, so it
    /// persists in-game (unlike the save-list label).
    SetName {
        /// Path to the save directory.
        dir: PathBuf,
        /// New player name (UTF-16; up to 16 characters).
        name: String,
        /// Copy the original files into this directory before saving.
        /// Must be outside the save directory.
        #[arg(long, value_name = "DIR")]
        backup_dir: Option<PathBuf>,
    },
    /// Set the play time shown in the save list (the `PARAM.SFO` label).
    ///
    /// Play time is not stored in the game data, only this label — which the
    /// game regenerates on its next in-game save; in-game persistence is
    /// unconfirmed.
    SetPlaytime {
        /// Path to the save directory.
        dir: PathBuf,
        /// New play time as HH:MM:SS (for example `12:34:56`).
        time: String,
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
        Command::KeyItems { dir } => key_items(&dir),
        Command::SetKeyItem {
            dir,
            key_item,
            state,
            backup_dir,
        } => set_key_item(&dir, &key_item, &state, backup_dir.as_deref()),
        Command::Units { dir } => units(&dir),
        Command::SetRarepon {
            dir,
            index,
            rarepon,
            backup_dir,
        } => set_rarepon(&dir, index, &rarepon, backup_dir.as_deref()),
        Command::SetWeapon {
            dir,
            index,
            tier,
            backup_dir,
        } => set_weapon(&dir, index, &tier, backup_dir.as_deref()),
        Command::SetLoadoutSlots {
            dir,
            state,
            backup_dir,
        } => set_loadout_slots(&dir, &state, backup_dir.as_deref()),
        Command::UnlockAll { dir, backup_dir } => unlock_all(&dir, backup_dir.as_deref()),
        Command::BonusPatapons { dir } => bonus_patapons(&dir),
        Command::SetBonusPatapon {
            dir,
            patapon,
            state,
            backup_dir,
        } => set_bonus_patapon(&dir, &patapon, &state, backup_dir.as_deref()),
        Command::SetDialogSeen {
            dir,
            patapon,
            state,
            backup_dir,
        } => set_dialog_seen(&dir, &patapon, &state, backup_dir.as_deref()),
        Command::SetMinigamePlayed {
            dir,
            patapon,
            state,
            backup_dir,
        } => set_minigame_played(&dir, &patapon, &state, backup_dir.as_deref()),
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
        Command::SetName {
            dir,
            name,
            backup_dir,
        } => set_name(&dir, &name, backup_dir.as_deref()),
        Command::SetPlaytime {
            dir,
            time,
            backup_dir,
        } => set_playtime(&dir, &time, backup_dir.as_deref()),
    }
}

fn open(dir: &Path) -> Result<SaveSlot> {
    SaveSlot::open(dir, &KeyProvider::Embedded)
        .with_context(|| format!("opening save {}", dir.display()))
}

fn info(dir: &Path) -> Result<()> {
    let slot = open(dir)?;
    println!("Region:   {}", slot.region().serial());
    if let Some(name) = slot.player_name() {
        println!("Name:     {name}");
    }
    if let Some(title) = slot.sfo().get_str("SAVEDATA_TITLE") {
        println!("Title:    {title}");
    }
    if let Some(detail) = slot.sfo().get_str("SAVEDATA_DETAIL") {
        println!("Detail:   {}", detail.trim().replace('\n', " / "));
    }
    if let Some((h, m, s)) = slot.playtime() {
        println!("Playtime: {h:02}:{m:02}:{s:02}");
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

/// Sets the player's name in the game data (persists in-game).
fn set_name(dir: &Path, name: &str, backup_dir: Option<&Path>) -> Result<()> {
    let mut slot = open(dir)?;
    let before = slot.player_name();
    slot.set_player_name(name)
        .with_context(|| format!("setting the player name to {name:?}"))?;
    back_up_and_save(&slot, backup_dir)?;
    match before {
        Some(b) => println!("Player name: {b:?} -> {name:?}"),
        None => println!("Player name set to {name:?}"),
    }
    Ok(())
}

/// Sets the save-list play time (PARAM.SFO label only).
fn set_playtime(dir: &Path, time: &str, backup_dir: Option<&Path>) -> Result<()> {
    let (h, m, s) = parse_hms(time)?;
    let mut slot = open(dir)?;
    slot.set_playtime(h, m, s)
        .with_context(|| format!("setting play time to {time:?}"))?;
    back_up_and_save(&slot, backup_dir)?;
    println!("Play time set to {h:02}:{m:02}:{s:02}");
    Ok(())
}

/// Parses an `HH:MM:SS` string into `(hours, minutes, seconds)`.
fn parse_hms(time: &str) -> Result<(u32, u8, u8)> {
    let parts: Vec<&str> = time.split(':').collect();
    let [h, m, s] = parts[..] else {
        anyhow::bail!("play time must be HH:MM:SS (got {time:?})");
    };
    let h = h.parse().with_context(|| format!("hours in {time:?}"))?;
    let m = m.parse().with_context(|| format!("minutes in {time:?}"))?;
    let s = s.parse().with_context(|| format!("seconds in {time:?}"))?;
    Ok((h, m, s))
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

fn key_items(dir: &Path) -> Result<()> {
    let slot = open(dir)?;
    let mut category = "";
    for (key_item, unlocked) in slot.key_items() {
        if key_item.category() != category {
            category = key_item.category();
            println!("[{category}]");
        }
        let mark = if unlocked { "x" } else { " " };
        println!("  [{mark}] {}", key_item.name());
    }
    Ok(())
}

/// Parses an `on`/`off` (unlock/lock) state argument.
fn parse_state(state: &str) -> Result<bool> {
    match state.to_ascii_lowercase().as_str() {
        "on" | "unlock" | "true" | "1" => Ok(true),
        "off" | "lock" | "false" | "0" => Ok(false),
        other => bail!("expected `on` or `off`, got `{other}`"),
    }
}

fn set_key_item(dir: &Path, key_item: &str, state: &str, backup_dir: Option<&Path>) -> Result<()> {
    let unlocked = parse_state(state)?;
    let mut slot = open(dir)?;

    let edited = if key_item == "all" {
        for k in KeyItem::all() {
            slot.set_key_item(k, unlocked)?;
        }
        format!("{} key items", KeyItem::all().count())
    } else {
        let k = KeyItem::from_slug(key_item)
            .with_context(|| format!("unknown key item `{key_item}` (try `key-items` to list)"))?;
        slot.set_key_item(k, unlocked)?;
        k.name().to_string()
    };

    back_up_and_save(&slot, backup_dir)?;
    println!("{edited}: {}", if unlocked { "unlocked" } else { "locked" });
    Ok(())
}

fn units(dir: &Path) -> Result<()> {
    let slot = open(dir)?;
    let n = slot.army_size();
    println!("Army: {n} units");
    for i in 0..n {
        let class = slot.unit_class(i).unwrap_or("Unknown");
        let rarepon = slot.unit_rarepon(i).map_or_else(
            || {
                slot.unit_rarepon_code(i)
                    .map_or_else(|| "-".to_string(), |c| format!("unknown ({c:#010X})"))
            },
            |r| r.name().to_string(),
        );
        let weapon = slot.unit_weapon(i).unwrap_or("-");
        println!("  [{i:>3}] {class:<8} {rarepon:<10} {weapon}");
    }
    Ok(())
}

fn rarepon_choices() -> String {
    Rarepon::all()
        .map(|r| r.slug())
        .collect::<Vec<_>>()
        .join(", ")
}

fn set_rarepon(dir: &Path, index: usize, rarepon: &str, backup_dir: Option<&Path>) -> Result<()> {
    if rarepon == "list" {
        println!("rarepons: {}", rarepon_choices());
        return Ok(());
    }
    let r = Rarepon::from_slug(rarepon).with_context(|| {
        format!(
            "unknown rarepon `{rarepon}` (choices: {})",
            rarepon_choices()
        )
    })?;
    let mut slot = open(dir)?;
    if slot.unit_class(index).is_none() {
        bail!(
            "no unit at roster index {index} (army has {} units; see `units`)",
            slot.army_size()
        );
    }
    slot.set_unit_rarepon(index, r)?;
    back_up_and_save(&slot, backup_dir)?;
    println!("Unit {index}: rarepon set to {}", r.name());
    Ok(())
}

fn set_weapon(dir: &Path, index: usize, tier: &str, backup_dir: Option<&Path>) -> Result<()> {
    let mut slot = open(dir)?;
    let max = slot.unit_weapon_max_tier(index).with_context(|| {
        format!(
            "no weapon at roster index {index} (army has {} units; see `units`)",
            slot.army_size()
        )
    })?;
    let tier_num: u8 = if tier.eq_ignore_ascii_case("max") {
        max
    } else {
        tier.parse()
            .with_context(|| format!("weapon tier must be a number (1..={max}) or `max`"))?
    };
    slot.set_unit_weapon(index, tier_num)?;
    let weapon = slot.unit_weapon(index).unwrap_or("?").to_owned();
    back_up_and_save(&slot, backup_dir)?;
    println!("Unit {index}: weapon set to {weapon} (tier {tier_num})");
    Ok(())
}

fn set_loadout_slots(dir: &Path, state: &str, backup_dir: Option<&Path>) -> Result<()> {
    let open_slots = parse_state(state)?;
    let mut slot = open(dir)?;
    slot.set_loadout_slots(open_slots)?;
    back_up_and_save(&slot, backup_dir)?;
    println!(
        "Mission-prep loadout slots: {}",
        if open_slots { "open" } else { "closed" }
    );
    Ok(())
}

fn unlock_all(dir: &Path, backup_dir: Option<&Path>) -> Result<()> {
    let mut slot = open(dir)?;
    let changed = slot.unlock_all()?;
    back_up_and_save(&slot, backup_dir)?;
    println!("Forced all progression unlocks ({changed} bytes changed).");
    Ok(())
}

/// Lists the bonus Patapons, whether each is revived (`[x]`), and whether its
/// one-time intro dialog has been seen.
fn bonus_patapons(dir: &Path) -> Result<()> {
    let slot = open(dir)?;
    for (bp, revived) in slot.bonus_patapons() {
        let mark = if revived { "x" } else { " " };
        let intro = if slot.bonus_patapon_dialog_seen(bp) {
            "intro seen"
        } else {
            "intro unseen"
        };
        let played = if slot.bonus_patapon_minigame_played(bp) {
            "minigame played"
        } else {
            "minigame unplayed"
        };
        match bp.minigame() {
            Some(minigame) => {
                println!("  [{mark}] {} ({minigame}) - {intro}, {played}", bp.name())
            }
            None => println!("  [{mark}] {} - {intro}, {played}", bp.name()),
        }
    }
    Ok(())
}

/// Revives or removes a bonus Patapon (or all of them).
fn set_bonus_patapon(
    dir: &Path,
    patapon: &str,
    state: &str,
    backup_dir: Option<&Path>,
) -> Result<()> {
    let revived = parse_state(state)?;
    let mut slot = open(dir)?;

    let edited = if patapon == "all" {
        for bp in BonusPatapon::all() {
            slot.set_bonus_patapon(bp, revived)?;
        }
        format!("{} bonus Patapons", BonusPatapon::all().count())
    } else {
        let bp = BonusPatapon::from_slug(patapon).with_context(|| {
            format!("unknown bonus Patapon `{patapon}` (try `bonus-patapons` to list)")
        })?;
        slot.set_bonus_patapon(bp, revived)?;
        bp.name().to_string()
    };

    back_up_and_save(&slot, backup_dir)?;
    println!("{edited}: {}", if revived { "revived" } else { "removed" });
    Ok(())
}

/// Marks a bonus Patapon's intro dialog as seen, or clears it so the intro replays.
fn set_dialog_seen(
    dir: &Path,
    patapon: &str,
    state: &str,
    backup_dir: Option<&Path>,
) -> Result<()> {
    let seen = parse_state(state)?;
    let mut slot = open(dir)?;

    let edited = if patapon == "all" {
        for bp in BonusPatapon::all() {
            slot.set_bonus_patapon_dialog_seen(bp, seen)?;
        }
        format!("{} bonus Patapons", BonusPatapon::all().count())
    } else {
        let bp = BonusPatapon::from_slug(patapon).with_context(|| {
            format!("unknown bonus Patapon `{patapon}` (try `bonus-patapons` to list)")
        })?;
        slot.set_bonus_patapon_dialog_seen(bp, seen)?;
        bp.name().to_string()
    };

    back_up_and_save(&slot, backup_dir)?;
    println!(
        "{edited}: intro {}",
        if seen { "marked seen" } else { "will replay" }
    );
    Ok(())
}

/// Marks a bonus Patapon's minigame as played or never-played.
fn set_minigame_played(
    dir: &Path,
    patapon: &str,
    state: &str,
    backup_dir: Option<&Path>,
) -> Result<()> {
    let played = parse_state(state)?;
    let mut slot = open(dir)?;

    let edited = if patapon == "all" {
        for bp in BonusPatapon::all() {
            slot.set_bonus_patapon_minigame_played(bp, played)?;
        }
        format!("{} bonus Patapons", BonusPatapon::all().count())
    } else {
        let bp = BonusPatapon::from_slug(patapon).with_context(|| {
            format!("unknown bonus Patapon `{patapon}` (try `bonus-patapons` to list)")
        })?;
        slot.set_bonus_patapon_minigame_played(bp, played)?;
        bp.name().to_string()
    };

    back_up_and_save(&slot, backup_dir)?;
    println!(
        "{edited}: minigame {}",
        if played {
            "marked played"
        } else {
            "marked unplayed"
        }
    );
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
