use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use serde_json::json;
use tabled::Table;

use crate::api::LinearClient;
use crate::display_options;
use crate::output::{
    ensure_non_empty, filter_values, print_json, print_json_owned, sort_values, OutputOptions,
};
use crate::text::truncate;

/// Initiative status/health updates (`initiativeUpdateCreate` etc.), the
/// initiative counterpart of `project-updates`.
#[derive(Subcommand)]
pub enum InitiativeUpdateCommands {
    /// List updates for an initiative
    #[command(alias = "ls")]
    List {
        /// Initiative ID
        initiative: String,
    },
    /// Get a specific update
    Get {
        /// Update ID
        id: String,
    },
    /// Create an initiative update
    Create {
        /// Initiative ID
        initiative: String,
        /// Update body (markdown)
        #[arg(short, long)]
        body: String,
        /// Initiative health: onTrack, atRisk, offTrack
        #[arg(short = 'H', long)]
        health: Option<String>,
    },
    /// Update an initiative update
    Update {
        /// Update ID
        id: String,
        /// New body
        #[arg(short, long)]
        body: Option<String>,
        /// New health status
        #[arg(short = 'H', long)]
        health: Option<String>,
    },
    /// Archive an initiative update
    Archive {
        /// Update ID
        id: String,
    },
    /// Unarchive an initiative update
    Unarchive {
        /// Update ID
        id: String,
    },
}

struct UpdateRow {
    health: String,
    author: String,
    date: String,
    body: String,
    id: String,
}

impl_tabled!(UpdateRow {
    health => "Health",
    author => "Author",
    date => "Date",
    body => "Body",
    id => "ID",
});

fn format_health(health: Option<&str>) -> String {
    match health {
        Some("onTrack") => "On Track".green().to_string(),
        Some("atRisk") => "At Risk".yellow().to_string(),
        Some("offTrack") => "Off Track".red().to_string(),
        Some(other) => other.to_string(),
        None => "-".to_string(),
    }
}

pub async fn handle(cmd: InitiativeUpdateCommands, output: &OutputOptions) -> Result<()> {
    match cmd {
        InitiativeUpdateCommands::List { initiative } => list_updates(&initiative, output).await,
        InitiativeUpdateCommands::Get { id } => get_update(&id, output).await,
        InitiativeUpdateCommands::Create {
            initiative,
            body,
            health,
        } => create_update(&initiative, &body, health, output).await,
        InitiativeUpdateCommands::Update { id, body, health } => {
            update_update(&id, body, health, output).await
        }
        InitiativeUpdateCommands::Archive { id } => archive_update(&id, output).await,
        InitiativeUpdateCommands::Unarchive { id } => unarchive_update(&id, output).await,
    }
}

async fn list_updates(initiative: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query($initiativeId: String!) {
            initiative(id: $initiativeId) {
                name
                initiativeUpdates(first: 50) {
                    nodes {
                        id
                        body
                        health
                        createdAt
                        user { name }
                    }
                }
            }
        }
    "#;

    let result = client
        .query(query, Some(json!({ "initiativeId": initiative })))
        .await?;
    let initiative_data = &result["data"]["initiative"];

    if initiative_data.is_null() {
        anyhow::bail!("Initiative not found: {}", initiative);
    }

    let initiative_name = initiative_data["name"].as_str().unwrap_or(initiative);
    let updates = initiative_data["initiativeUpdates"]["nodes"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    if output.is_json() || output.has_template() {
        print_json_owned(
            json!({
                "initiative": initiative_name,
                "updates": updates
            }),
            output,
        )?;
        return Ok(());
    }

    if updates.is_empty() {
        println!("No updates found for initiative '{}'.", initiative_name);
        return Ok(());
    }

    let mut filtered: Vec<serde_json::Value> = updates;
    filter_values(&mut filtered, &output.filters);

    if let Some(sort_key) = output.json.sort.as_deref() {
        sort_values(&mut filtered, sort_key, output.json.order);
    }

    ensure_non_empty(&filtered, output)?;
    if filtered.is_empty() {
        println!("No updates match the filter criteria.");
        return Ok(());
    }

    let width = display_options().max_width(50);
    let rows: Vec<UpdateRow> = filtered
        .iter()
        .map(|u| UpdateRow {
            health: format_health(u["health"].as_str()),
            author: truncate(
                u["user"]["name"].as_str().unwrap_or("-"),
                display_options().max_width(20),
            ),
            date: u["createdAt"]
                .as_str()
                .map(|s| s.get(..10).unwrap_or(s).to_string())
                .unwrap_or_else(|| "-".to_string()),
            body: truncate(u["body"].as_str().unwrap_or(""), width),
            id: u["id"].as_str().unwrap_or("").to_string(),
        })
        .collect();

    println!(
        "{}",
        format!("Initiative Updates for '{}'", initiative_name).bold()
    );
    println!("{}", "-".repeat(40));

    let rows_len = rows.len();
    let table = Table::new(rows).to_string();
    println!("{}", table);
    println!("\n{} updates shown", rows_len);

    Ok(())
}

async fn get_update(id: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query($id: String!) {
            initiativeUpdate(id: $id) {
                id
                body
                health
                createdAt
                updatedAt
                url
                initiative { name }
                user { name }
            }
        }
    "#;

    let result = client.query(query, Some(json!({ "id": id }))).await?;
    let raw = &result["data"]["initiativeUpdate"];

    if raw.is_null() {
        anyhow::bail!("Initiative update not found: {}", id);
    }

    if output.is_json() || output.has_template() {
        print_json(raw, output)?;
        return Ok(());
    }

    println!("{}", "Initiative Update".bold());
    println!("{}", "-".repeat(40));

    if let Some(initiative_name) = raw["initiative"]["name"].as_str() {
        println!("Initiative: {}", initiative_name);
    }
    if let Some(author) = raw["user"]["name"].as_str() {
        println!("Author: {}", author);
    }
    println!("Health: {}", format_health(raw["health"].as_str()));
    println!(
        "Created: {}",
        raw["createdAt"]
            .as_str()
            .map(|s| s.get(..10).unwrap_or(s))
            .unwrap_or("-")
    );
    if let Some(url) = raw["url"].as_str() {
        println!("URL: {}", url);
    }
    println!("ID: {}", id);

    if let Some(body) = raw["body"].as_str() {
        if !body.is_empty() {
            println!("\n{}", body);
        }
    }

    Ok(())
}

async fn create_update(
    initiative: &str,
    body: &str,
    health: Option<String>,
    output: &OutputOptions,
) -> Result<()> {
    let client = LinearClient::new()?;

    let mut input = json!({
        "initiativeId": initiative,
        "body": body,
    });
    if let Some(h) = &health {
        input["health"] = json!(h);
    }

    let mutation = r#"
        mutation($input: InitiativeUpdateCreateInput!) {
            initiativeUpdateCreate(input: $input) {
                success
                initiativeUpdate { id health url }
            }
        }
    "#;

    let result = client
        .mutate(mutation, Some(json!({ "input": input })))
        .await?;

    if result["data"]["initiativeUpdateCreate"]["success"].as_bool() == Some(true) {
        let update = &result["data"]["initiativeUpdateCreate"]["initiativeUpdate"];
        if output.is_json() || output.has_template() {
            print_json(update, output)?;
            return Ok(());
        }
        println!("{} Initiative update created", "+".green());
        println!("  ID: {}", update["id"].as_str().unwrap_or(""));
        if let Some(url) = update["url"].as_str() {
            println!("  URL: {}", url);
        }
    } else {
        anyhow::bail!("Failed to create initiative update");
    }

    Ok(())
}

async fn update_update(
    id: &str,
    body: Option<String>,
    health: Option<String>,
    output: &OutputOptions,
) -> Result<()> {
    let client = LinearClient::new()?;

    let mut input = json!({});
    if let Some(b) = body {
        input["body"] = json!(b);
    }
    if let Some(h) = health {
        input["health"] = json!(h);
    }

    if input.as_object().map(|o| o.is_empty()).unwrap_or(true) {
        println!("No updates specified.");
        return Ok(());
    }

    let mutation = r#"
        mutation($id: String!, $input: InitiativeUpdateUpdateInput!) {
            initiativeUpdateUpdate(id: $id, input: $input) {
                success
                initiativeUpdate { id health }
            }
        }
    "#;

    let result = client
        .mutate(mutation, Some(json!({ "id": id, "input": input })))
        .await?;

    if result["data"]["initiativeUpdateUpdate"]["success"].as_bool() == Some(true) {
        let update = &result["data"]["initiativeUpdateUpdate"]["initiativeUpdate"];
        if output.is_json() || output.has_template() {
            print_json(update, output)?;
            return Ok(());
        }
        println!("{} Initiative update updated", "+".green());
    } else {
        anyhow::bail!("Failed to update initiative update");
    }

    Ok(())
}

async fn archive_update(id: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let mutation = r#"
        mutation($id: String!) {
            initiativeUpdateArchive(id: $id) {
                success
            }
        }
    "#;

    let result = client.mutate(mutation, Some(json!({ "id": id }))).await?;

    let success = result["data"]["initiativeUpdateArchive"]["success"]
        .as_bool()
        .unwrap_or(false);

    if success {
        if output.is_json() || output.has_template() {
            print_json_owned(json!({ "archived": id }), output)?;
            return Ok(());
        }
        println!("{} Initiative update archived", "+".green());
    } else {
        anyhow::bail!("Failed to archive initiative update {}", id);
    }

    Ok(())
}

async fn unarchive_update(id: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let mutation = r#"
        mutation($id: String!) {
            initiativeUpdateUnarchive(id: $id) {
                success
            }
        }
    "#;

    let result = client.mutate(mutation, Some(json!({ "id": id }))).await?;

    let success = result["data"]["initiativeUpdateUnarchive"]["success"]
        .as_bool()
        .unwrap_or(false);

    if success {
        if output.is_json() || output.has_template() {
            print_json_owned(json!({ "unarchived": id }), output)?;
            return Ok(());
        }
        println!("{} Initiative update unarchived", "+".green());
    } else {
        anyhow::bail!("Failed to unarchive initiative update {}", id);
    }

    Ok(())
}
