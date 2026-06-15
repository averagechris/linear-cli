use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use tabled::Table;

use crate::cache::{Cache, CacheType};

#[derive(Subcommand)]
pub enum CacheCommands {
    /// Clear all cached data
    Clear {
        /// Only clear a specific cache type (teams, users, statuses, labels, projects)
        #[arg(short, long)]
        r#type: Option<String>,
    },
    /// Show cache status and sizes
    Status,
}

struct CacheStatusRow {
    cache_type: String,
    valid: String,
    age: String,
    size: String,
    items: String,
}

impl_tabled!(CacheStatusRow {
    cache_type => "Type",
    valid => "Valid",
    age => "Age",
    size => "Size",
    items => "Items",
});

pub async fn handle(cmd: CacheCommands) -> Result<()> {
    match cmd {
        CacheCommands::Clear { r#type } => clear_cache(r#type).await,
        CacheCommands::Status => show_status().await,
    }
}

async fn clear_cache(cache_type: Option<String>) -> Result<()> {
    let cache = Cache::new()?;

    if let Some(type_str) = cache_type {
        let cache_type = match type_str.to_lowercase().as_str() {
            "teams" => CacheType::Teams,
            "users" => CacheType::Users,
            "statuses" | "states" => CacheType::Statuses,
            "labels" => CacheType::Labels,
            "projects" => CacheType::Projects,
            _ => {
                anyhow::bail!(
                    "Unknown cache type: '{}'. Valid types: teams, users, statuses, labels, projects",
                    type_str
                );
            }
        };
        cache.clear_type(cache_type)?;
        println!(
            "{} Cleared {} cache",
            "+".green(),
            cache_type.display_name()
        );
    } else {
        cache.clear_all()?;
        println!("{} Cleared all caches", "+".green());
    }

    Ok(())
}

async fn show_status() -> Result<()> {
    let cache = Cache::new()?;
    let statuses = cache.status();

    println!("{}", "Cache Status".bold());
    println!("{}", "-".repeat(50));

    let rows: Vec<CacheStatusRow> = statuses
        .iter()
        .map(|s| CacheStatusRow {
            cache_type: s.cache_type.display_name().to_string(),
            valid: if s.valid {
                "Yes".green().to_string()
            } else {
                "No".dimmed().to_string()
            },
            age: s.age_display(),
            size: s.size_display(),
            items: s
                .item_count
                .map(|c| c.to_string())
                .unwrap_or_else(|| "-".to_string()),
        })
        .collect();

    let table = Table::new(rows).to_string();
    println!("{}", table);

    // Summary
    let valid_count = statuses.iter().filter(|s| s.valid).count();
    let total_size: u64 = statuses.iter().filter_map(|s| s.size_bytes).sum();

    println!();
    println!("{} of {} caches valid", valid_count, CacheType::all().len());
    if total_size > 0 {
        let size_display = if total_size < 1024 {
            format!("{} B", total_size)
        } else if total_size < 1024 * 1024 {
            format!("{:.1} KB", total_size as f64 / 1024.0)
        } else {
            format!("{:.1} MB", total_size as f64 / (1024.0 * 1024.0))
        };
        println!("Total cache size: {}", size_display);
    }

    Ok(())
}
