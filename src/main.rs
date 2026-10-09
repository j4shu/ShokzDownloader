use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use dialoguer::{Confirm, Select};
use sysinfo::Disks;

fn pick(prompt: &str, paths: Vec<PathBuf>) -> Result<PathBuf> {
    let names: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
    let selection = Select::new()
        .with_prompt(prompt)
        .items(&names)
        .default(0)
        .interact()
        .context("Selection failed")?;
    Ok(paths[selection].clone())
}

fn select_device() -> Result<PathBuf> {
    let disks = Disks::new_with_refreshed_list();
    let mut removable: Vec<PathBuf> = disks
        .list()
        .iter()
        .filter(|d| d.is_removable())
        .map(|d| d.mount_point().to_path_buf())
        .collect();
    removable.sort();

    if removable.is_empty() {
        bail!("No removable devices found. Please connect your Shokz device and try again.");
    }
    pick("Select Shokz device", removable)
}

fn select_folder(dir: &Path, label: &str) -> Result<PathBuf> {
    let mut folders: Vec<PathBuf> = std::fs::read_dir(dir)
        .with_context(|| format!("Cannot read {}", dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    folders.sort();

    if folders.is_empty() {
        bail!("No folders found in {}.", dir.display());
    }
    pick(&format!("Select {label}"), folders)
}

const MUSIC_EXTENSIONS: &[&str] = &["mp3", "m4a", "flac", "wav", "ogg", "wma", "aac"];
fn collect_music_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            bail!(
                "Album folder contains a subdirectory: '{}'. Only flat folders are supported.",
                path.file_name().unwrap_or_default().to_string_lossy()
            );
        } else if path.is_file()
            && path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| MUSIC_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        {
            files.push(path);
        }
    }

    files.sort_by(|a, b| natord::compare(&a.to_string_lossy(), &b.to_string_lossy()));
    Ok(files)
}

const DELAY_MS: u64 = 100;
fn transfer_files(files: &[PathBuf], source_root: &Path, target_root: &Path) -> Result<()> {
    let mut total_bytes: u64 = 0;
    let mut failures: usize = 0;

    let source_folder = source_root.file_name().unwrap_or_default();
    let dest_dir = target_root.join(source_folder);
    std::fs::create_dir_all(&dest_dir)?;

    for (i, file) in files.iter().enumerate() {
        let name = file.file_name().unwrap_or_default();
        println!(
            "[{}/{}] Copying: {}",
            i + 1,
            files.len(),
            name.to_string_lossy()
        );

        match std::fs::copy(file, dest_dir.join(name)) {
            Ok(bytes) => total_bytes += bytes,
            Err(e) => {
                eprintln!("  Failed to copy {}: {e}", file.display());
                failures += 1;
            }
        }

        // Delay between files to ensure distinct timestamps on the target device
        if i < files.len() - 1 {
            std::thread::sleep(std::time::Duration::from_millis(DELAY_MS));
        }
    }

    let total_mb = total_bytes as f64 / (1024.0 * 1024.0);
    println!(
        "Done! Transferred {} file(s) ({:.1} MB) to {}.",
        files.len() - failures,
        total_mb,
        target_root.display()
    );

    if failures > 0 {
        eprintln!("{failures} file(s) failed to copy.");
    }

    Ok(())
}

fn main() -> Result<()> {
    let target = select_device()?;
    println!("Selected device: {}\n", target.display());

    let desktop = dirs::desktop_dir().context("Could not find Desktop directory.")?;
    let source = select_folder(&desktop, "album")?;
    println!("Selected album: {}\n", source.display());

    let files = collect_music_files(&source)?;
    if files.is_empty() {
        println!("No music files found in '{}'.", source.display());
        return Ok(());
    }
    println!("Found {} music file(s) to transfer.\n", files.len());

    for f in &files {
        println!("  {}", f.file_name().unwrap_or_default().to_string_lossy());
    }
    println!();

    let proceed = Confirm::new()
        .with_prompt("Proceed with transfer?")
        .default(false)
        .interact()
        .context("Selection failed")?;
    if !proceed {
        println!("Transfer cancelled.");
        return Ok(());
    }

    transfer_files(&files, &source, &target)
}
