use anyhow::Result;
use clap::Subcommand;
use serde_json::json;
use tabled::Table;

use crate::api::LinearClient;
use crate::output::{print_json, print_json_owned, OutputOptions};
use crate::pagination::PaginationOptions;
use crate::text::truncate;
use crate::types::Initiative;
use crate::DISPLAY_OPTIONS;
use colored::Colorize;

#[derive(Subcommand, Debug)]
pub enum InitiativeCommands {
    /// List all initiatives
    List,
    /// Get initiative details
    Get {
        /// Initiative ID
        id: String,
    },
    /// Create a new initiative
    Create {
        /// Initiative name
        name: String,
        /// Description
        #[arg(short, long)]
        description: Option<String>,
        /// Status
        #[arg(short, long)]
        status: Option<String>,
    },
    /// Update an existing initiative
    Update {
        /// Initiative ID
        id: String,
        /// New name
        #[arg(short, long)]
        name: Option<String>,
        /// New description
        #[arg(short, long)]
        description: Option<String>,
        /// New status
        #[arg(short, long)]
        status: Option<String>,
        /// Target date (YYYY-MM-DD)
        #[arg(long)]
        target_date: Option<String>,
        /// Initiative owner (user email, name, or UUID)
        #[arg(long)]
        owner: Option<String>,
        /// Preview without updating (dry run)
        #[arg(long)]
        dry_run: bool,
    },
    /// Delete an initiative
    Delete {
        /// Initiative ID
        id: String,
        /// Skip confirmation
        #[arg(long)]
        force: bool,
    },
}

struct InitiativeRow {
    id: String,
    name: String,
    status: String,
    health: String,
    target_date: String,
    project_count: String,
}

impl_tabled!(InitiativeRow {
    id => "ID",
    name => "Name",
    status => "Status",
    health => "Health",
    target_date => "Target",
    project_count => "Projects",
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

pub async fn handle(
    cmd: InitiativeCommands,
    output: &OutputOptions,
    pagination: &PaginationOptions,
) -> Result<()> {
    match cmd {
        InitiativeCommands::List => list_initiatives(output, pagination).await,
        InitiativeCommands::Get { id } => get_initiative(&id, output).await,
        InitiativeCommands::Create {
            name,
            description,
            status,
        } => create_initiative(&name, description, status, output).await,
        InitiativeCommands::Update {
            id,
            name,
            description,
            status,
            target_date,
            owner,
            dry_run,
        } => {
            let dry_run = dry_run || output.dry_run;
            update_initiative(
                &id,
                name,
                description,
                status,
                target_date,
                owner,
                dry_run,
                output,
            )
            .await
        }
        InitiativeCommands::Delete { id, force } => delete_initiative(&id, force).await,
    }
}

async fn list_initiatives(output: &OutputOptions, pagination: &PaginationOptions) -> Result<()> {
    let client = LinearClient::new()?;
    let pagination = pagination.with_default_limit(250);
    let limit = pagination.limit.unwrap_or(250);

    // NB: the live schema removed `Initiative.progress`; selecting it now
    // fails with GRAPHQL_VALIDATION_FAILED, so it must stay out of this query.
    let query = r#"
        query($first: Int) {
            initiatives(first: $first) {
                nodes {
                    id
                    name
                    description
                    status
                    health
                    targetDate
                    sortOrder
                    projects {
                        nodes {
                            id
                        }
                    }
                }
            }
        }
    "#;

    let mut variables = serde_json::Map::new();
    variables.insert("first".to_string(), json!(limit));
    let result = client
        .query(query, Some(serde_json::Value::Object(variables)))
        .await?;
    let initiatives = &result["data"]["initiatives"]["nodes"];

    if output.is_json() {
        print_json(initiatives, output)?;
    } else {
        let display = DISPLAY_OPTIONS.get().cloned().unwrap_or_default();
        let max_width = display.max_width(40);

        let rows: Vec<InitiativeRow> = initiatives
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|v| {
                let i = serde_json::from_value::<Initiative>(v.clone()).ok()?;
                let project_count = v["projects"]["nodes"]
                    .as_array()
                    .map(|a| a.len().to_string())
                    .unwrap_or_else(|| "0".to_string());
                Some(InitiativeRow {
                    id: i.id,
                    name: truncate(&i.name, max_width),
                    status: i.status.as_deref().unwrap_or("-").to_string(),
                    health: format_health(v["health"].as_str()),
                    target_date: i.target_date.as_deref().unwrap_or("-").to_string(),
                    project_count,
                })
            })
            .collect();

        if rows.is_empty() {
            println!("No initiatives found");
        } else {
            println!("{}", Table::new(rows));
        }
    }

    Ok(())
}

async fn get_initiative(id: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query($id: String!) {
            initiative(id: $id) {
                id
                name
                description
                status
                health
                targetDate
                sortOrder
                createdAt
                updatedAt
                projects {
                    nodes {
                        id
                        name
                        state
                        progress
                    }
                }
            }
        }
    "#;

    let result = client.query(query, Some(json!({ "id": id }))).await?;
    let initiative = &result["data"]["initiative"];

    if initiative.is_null() {
        anyhow::bail!("Initiative not found: {}", id);
    }

    print_json(initiative, output)?;
    Ok(())
}

async fn create_initiative(
    name: &str,
    description: Option<String>,
    status: Option<String>,
    output: &OutputOptions,
) -> Result<()> {
    let client = LinearClient::new()?;

    let mut input = json!({ "name": name });
    if let Some(d) = description {
        input["description"] = json!(d);
    }
    if let Some(s) = status {
        input["status"] = json!(s);
    }

    let mutation = r#"
        mutation($input: InitiativeCreateInput!) {
            initiativeCreate(input: $input) {
                success
                initiative { id name }
            }
        }
    "#;

    let result = client
        .mutate(mutation, Some(json!({ "input": input })))
        .await?;

    if result["data"]["initiativeCreate"]["success"].as_bool() == Some(true) {
        let initiative = &result["data"]["initiativeCreate"]["initiative"];
        if output.is_json() || output.has_template() {
            print_json(initiative, output)?;
            return Ok(());
        }
        println!(
            "{} Created initiative: {}",
            "+".green(),
            initiative["name"].as_str().unwrap_or("")
        );
        println!("  ID: {}", initiative["id"].as_str().unwrap_or(""));
    } else {
        anyhow::bail!("Failed to create initiative");
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn update_initiative(
    id: &str,
    name: Option<String>,
    description: Option<String>,
    status: Option<String>,
    target_date: Option<String>,
    owner: Option<String>,
    dry_run: bool,
    output: &OutputOptions,
) -> Result<()> {
    let client = LinearClient::new()?;

    let mut input = json!({});
    if let Some(n) = name {
        input["name"] = json!(n);
    }
    if let Some(d) = description {
        input["description"] = json!(d);
    }
    if let Some(s) = status {
        input["status"] = json!(s);
    }
    if let Some(td) = target_date {
        input["targetDate"] = json!(td);
    }
    if let Some(ref o) = owner {
        // Resolve user name/email to UUID (skip during dry-run to avoid API calls)
        if dry_run {
            input["ownerId"] = json!(o);
        } else {
            let owner_id = crate::api::resolve_user_id(&client, o, &output.cache).await?;
            input["ownerId"] = json!(owner_id);
        }
    }

    if input.as_object().map(|o| o.is_empty()).unwrap_or(true) {
        println!("No updates specified.");
        return Ok(());
    }

    if dry_run {
        if output.is_json() || output.has_template() {
            print_json_owned(
                json!({
                    "dry_run": true,
                    "would_update": { "id": id, "input": input }
                }),
                output,
            )?;
        } else {
            println!("{}", "[DRY RUN] Would update initiative:".yellow().bold());
            println!("  ID: {}", id);
        }
        return Ok(());
    }

    let mutation = r#"
        mutation($id: String!, $input: InitiativeUpdateInput!) {
            initiativeUpdate(id: $id, input: $input) {
                success
                initiative { id name }
            }
        }
    "#;

    let result = client
        .mutate(mutation, Some(json!({ "id": id, "input": input })))
        .await?;

    if result["data"]["initiativeUpdate"]["success"].as_bool() == Some(true) {
        if output.is_json() || output.has_template() {
            print_json(&result["data"]["initiativeUpdate"]["initiative"], output)?;
            return Ok(());
        }
        println!("{} Initiative updated", "+".green());
    } else {
        anyhow::bail!("Failed to update initiative");
    }

    Ok(())
}

async fn delete_initiative(id: &str, force: bool) -> Result<()> {
    if !force && !crate::is_yes() {
        anyhow::bail!(
            "Delete requires --force flag. Use: linear initiatives delete {} --force",
            id
        );
    }

    let client = LinearClient::new()?;

    let mutation = r#"
        mutation($id: String!) {
            initiativeDelete(id: $id) {
                success
            }
        }
    "#;

    let result = client.mutate(mutation, Some(json!({ "id": id }))).await?;

    let success = result["data"]["initiativeDelete"]["success"]
        .as_bool()
        .unwrap_or(false);

    if success {
        println!("Initiative {} deleted.", id);
    } else {
        anyhow::bail!("Failed to delete initiative {}", id);
    }

    Ok(())
}
