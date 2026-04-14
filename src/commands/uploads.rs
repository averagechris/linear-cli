use anyhow::{Context, Result};
use clap::{Subcommand, ValueHint};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::api::{parse_linear_upload_url, LinearClient};

#[cfg(unix)]
fn create_private_file(path: &Path) -> Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;

    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .with_context(|| format!("Failed to create file: {}", path.display()))
}

#[cfg(not(unix))]
fn create_private_file(path: &Path) -> Result<std::fs::File> {
    Ok(std::fs::File::create(path)
        .with_context(|| format!("Failed to create file: {}", path.display()))?)
}

fn sibling_temp_path(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("download");
    parent.join(format!(".{}.tmp", file_name))
}

fn replace_file_atomically(temp_path: &Path, final_path: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        let _ = fs::remove_file(final_path);
    }

    fs::rename(temp_path, final_path)
        .with_context(|| format!("Failed to move {} into place", final_path.display()))?;
    Ok(())
}

#[derive(Subcommand)]
pub enum UploadCommands {
    /// Fetch an upload from Linear's upload storage
    #[command(alias = "get")]
    Fetch {
        /// The Linear upload URL (e.g., https://uploads.linear.app/...)
        url: String,

        /// Output file path (if not specified, outputs to stdout)
        #[arg(short = 'f', long = "file", value_hint = ValueHint::FilePath)]
        file: Option<String>,
    },
}

pub async fn handle(cmd: UploadCommands) -> Result<()> {
    match cmd {
        UploadCommands::Fetch { url, file } => fetch_upload(&url, file).await,
    }
}

async fn fetch_upload(url: &str, file: Option<String>) -> Result<()> {
    parse_linear_upload_url(url)?;

    let client = LinearClient::new()?;

    if let Some(file_path) = file {
        let final_path = PathBuf::from(&file_path);
        let temp_path = sibling_temp_path(&final_path);
        // Stream directly to file
        let mut file = create_private_file(&temp_path)?;
        let bytes_written = client
            .fetch_to_writer(url, &mut file)
            .await
            .context("Failed to fetch upload from Linear")?;
        file.sync_all()?;
        drop(file);
        replace_file_atomically(&temp_path, &final_path)?;
        eprintln!("Downloaded {} bytes to {}", bytes_written, file_path);
    } else {
        // Stream directly to stdout
        let mut stdout_handle = io::stdout().lock();
        let bytes_written = client
            .fetch_to_writer(url, &mut stdout_handle)
            .await
            .context("Failed to fetch upload from Linear")?;
        stdout_handle.flush()?;
        drop(stdout_handle);
        eprintln!("Downloaded {} bytes", bytes_written);
    }

    Ok(())
}
