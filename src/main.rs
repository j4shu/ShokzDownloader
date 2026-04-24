use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};

// --- CLI ---

#[derive(Parser)]
#[command(
    name = "shokz-downloader",
    about = "Transfer music to Shokz OpenSwim headphones"
)]
struct Cli {
    /// Show what would be transferred without copying
    #[arg(long)]
    dry_run: bool,
}

// --- Selection ---

fn select_folder(dir: &Path, label: &str) -> Result<PathBuf, String> {
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("Cannot read {}: {e}", dir.display()))?;

    let mut folders: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    folders.sort();

    if folders.is_empty() {
        return Err(format!("No folders found in {}.", dir.display()));
    }

    println!("{label}:");
    for (i, folder) in folders.iter().enumerate() {
        println!("  {}. {}", i + 1, folder.display());
    }

    print!("\nSelect your {label} (1-{}): ", folders.len());
    std::io::stdout().flush().map_err(|e| e.to_string())?;

    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .map_err(|e| format!("Failed to read input: {e}"))?;

    let choice: usize = input
        .trim()
        .parse()
        .map_err(|_| "Invalid selection.".to_string())?;

    if choice < 1 || choice > folders.len() {
        return Err(format!("Selection out of range (1-{}).", folders.len()));
    }

    Ok(folders[choice - 1].clone())
}

// --- File Discovery ---

const MUSIC_EXTENSIONS: &[&str] = &["mp3", "m4a", "flac", "wav", "ogg", "wma", "aac"];

fn collect_music_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();

    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            return Err(format!(
                "Album folder contains a subdirectory: '{}'. Only flat folders are supported.",
                path.file_name().unwrap_or_default().to_string_lossy()
            ));
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

fn transfer_files(
    files: &[PathBuf],
    source_root: &Path,
    target_root: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
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
        "Done! Transferred {} file(s) ({:.1} MB) to {}",
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
            .template("  {bar:40.cyan/blue} {bytes}/{total_bytes} ({eta})")
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

fn main() {
    let cli = Cli::parse();

    let target = match select_folder(Path::new("/Volumes"), "Shokz OpenSwim device") {
        Ok(path) => {
            println!("Selected device: {}\n", path.display());
            path
        }
        Err(msg) => {
            eprintln!("Error: {msg}");
            std::process::exit(1);
        }
    };

    let home = std::env::var("HOME").unwrap_or_else(|_| {
        eprintln!("Error: HOME not set");
        std::process::exit(1);
    });
    let desktop = PathBuf::from(home).join("Desktop");

    let source = match select_folder(&desktop, "album") {
        Ok(path) => {
            println!("Selected album: {}\n", path.display());
            path
        }
        Err(msg) => {
            eprintln!("Error: {msg}");
            std::process::exit(1);
        }
    };

    let files = match collect_music_files(&source) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Error scanning source directory: {e}");
            std::process::exit(1);
        }
    };

    if files.is_empty() {
        println!("No music files found in '{}'", source.display());
        return;
    }

    println!("Found {} music file(s) to transfer:\n", files.len());

    if cli.dry_run {
        for f in &files {
            println!("  {}", f.file_name().unwrap_or_default().to_string_lossy());
        }
        return;
    }

    if let Err(e) = transfer_files(&files, &source, &target) {
        eprintln!("\nTransfer failed: {e}");
        std::process::exit(1);
    }
}
