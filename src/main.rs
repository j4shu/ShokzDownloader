use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use dialoguer::Select;
use indicatif::{ProgressBar, ProgressStyle};

// --- Selection ---

fn select_folder(dir: &Path, label: &str) -> Result<PathBuf> {
    let entries =
        std::fs::read_dir(dir).with_context(|| format!("Cannot read {}", dir.display()))?;

    let mut folders: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    folders.sort();

    if folders.is_empty() {
        bail!("No folders found in {}.", dir.display());
    }

    let names: Vec<String> = folders.iter().map(|f| f.display().to_string()).collect();

    let selection = Select::new()
        .with_prompt(format!("Select {label}"))
        .items(&names)
        .default(0)
        .interact()
        .context("Selection failed")?;

    Ok(folders[selection].clone())
}

// --- File Discovery ---

const MUSIC_EXTENSIONS: &[&str] = &["mp3", "m4a", "flac", "wav", "ogg", "wma", "aac"];

fn collect_music_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            bail!(
                "Album folder contains a subdirectory: '{}'. Only flat folders are supported.",
                path.file_name().unwrap_or_default().to_string_lossy()
            );
        } else if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if MUSIC_EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
                    files.push(path);
                }
            }
        }
    }

    files.sort_by(|a, b| natord::compare(&a.to_string_lossy(), &b.to_string_lossy()));
    Ok(files)
}

// --- Copy Engine ---

const DELAY_MS: u64 = 100;

fn transfer_files(files: &[PathBuf], source_root: &Path, target_root: &Path) -> Result<()> {
    let total = files.len() as u64;
    let overall_pb = ProgressBar::new(total);
    overall_pb.set_style(
        ProgressStyle::default_bar()
            .template("[{pos}/{len}] {msg}")?
            .progress_chars("##-"),
    );

    let mut total_bytes: u64 = 0;
    let mut failures: usize = 0;

    let source_folder = source_root.file_name().unwrap_or_default();
    let dest_dir = target_root.join(source_folder);
    std::fs::create_dir_all(&dest_dir)?;

    for (i, file) in files.iter().enumerate() {
        let dest = dest_dir.join(file.file_name().unwrap_or_default());
        let file_name = file
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        overall_pb.set_message(format!("Copying: {file_name}"));

        match copy_with_progress(file, &dest) {
            Ok(bytes) => {
                total_bytes += bytes;
            }
            Err(e) => {
                overall_pb.suspend(|| {
                    eprintln!("  Failed to copy {}: {e}", file.display());
                });
                failures += 1;
            }
        }

        overall_pb.inc(1);

        // Delay between files to ensure distinct timestamps on the target device
        if i < files.len() - 1 {
            std::thread::sleep(std::time::Duration::from_millis(DELAY_MS));
        }
    }

    overall_pb.finish_and_clear();

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

fn copy_with_progress(src: &Path, dest: &Path) -> Result<u64, std::io::Error> {
    let metadata = std::fs::metadata(src)?;
    let file_size = metadata.len();

    let file_pb = ProgressBar::new(file_size);
    file_pb.set_style(
        ProgressStyle::default_bar()
            .template("{bar:40.cyan/blue} {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .progress_chars("##-"),
    );

    let mut reader = std::io::BufReader::new(std::fs::File::open(src)?);
    let mut writer = std::io::BufWriter::new(std::fs::File::create(dest)?);
    let mut buffer = [0u8; 65536];
    let mut copied: u64 = 0;

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        writer.write_all(&buffer[..bytes_read])?;
        copied += bytes_read as u64;
        file_pb.set_position(copied);
    }

    writer.flush()?;
    file_pb.finish_and_clear();

    Ok(copied)
}

// --- Main ---

fn main() -> Result<()> {
    // Select target device to transfer to
    let target = select_folder(Path::new("/Volumes"), "Shokz OpenSwim device")?;
    println!("Selected device: {}\n", target.display());

    // Select folder to transfer from
    let desktop = dirs::desktop_dir().context("Could not find Desktop directory.")?;
    let source = select_folder(&desktop, "album")?;
    println!("Selected album: {}\n", source.display());

    // Collect music files from the selected folder
    let files = collect_music_files(&source)?;
    if files.is_empty() {
        println!("No music files found in '{}'.", source.display());
        return Ok(());
    }
    println!("Found {} music file(s) to transfer.\n", files.len());

    // List the files to be transferred
    for f in &files {
        println!("  {}", f.file_name().unwrap_or_default().to_string_lossy());
    }
    println!();

    // Confirm or cancel transfer
    let choice = Select::new()
        .with_prompt("Proceed with transfer?")
        .items(&["No", "Yes"])
        .default(0)
        .interact()
        .context("Selection failed")?;
    if choice == 0 {
        println!("Transfer cancelled.");
        return Ok(());
    }

    // Start the transfer process
    transfer_files(&files, &source, &target)?;
    Ok(())
}
