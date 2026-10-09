//! Reject operations that have no preview before they look up credentials.

use crate::commands::{
    attachments, auth, comments, cycles, documents, export, favorites, git, initiatives, labels,
    notifications, project_updates, projects, roadmaps, sprint, teams, templates, time, triage,
    uploads, webhooks,
};
use crate::{Commands, ConfigCommands};

pub(crate) fn unsupported_command(command: &Commands) -> Option<&'static str> {
    match command {
        Commands::Attachments {
            action:
                attachments::AttachmentCommands::Create { .. }
                | attachments::AttachmentCommands::Update { .. }
                | attachments::AttachmentCommands::Delete { .. }
                | attachments::AttachmentCommands::LinkUrl { .. },
        } => Some("attachments"),
        Commands::Bulk { .. } => Some("bulk"),
        Commands::Cycles { action }
            if !matches!(
                action,
                cycles::CycleCommands::Update { .. }
                    | cycles::CycleCommands::List { .. }
                    | cycles::CycleCommands::Get { .. }
                    | cycles::CycleCommands::Current { .. }
            ) =>
        {
            Some("cycles")
        }
        Commands::Comments {
            action:
                comments::CommentCommands::Create { .. }
                | comments::CommentCommands::Update { .. }
                | comments::CommentCommands::Delete { .. },
        } => Some("comments"),
        Commands::Favorites {
            action:
                favorites::FavoriteCommands::Add { .. } | favorites::FavoriteCommands::Remove { .. },
        } => Some("favorites"),
        Commands::Labels {
            action:
                labels::LabelCommands::Create { .. }
                | labels::LabelCommands::Delete { .. }
                | labels::LabelCommands::Update { .. },
        } => Some("labels"),
        Commands::Notifications {
            action:
                notifications::NotificationCommands::Read { .. }
                | notifications::NotificationCommands::ReadAll
                | notifications::NotificationCommands::Archive { .. }
                | notifications::NotificationCommands::ArchiveAll,
        } => Some("notifications"),
        Commands::ProjectUpdates {
            action:
                project_updates::ProjectUpdateCommands::Create { .. }
                | project_updates::ProjectUpdateCommands::Update { .. }
                | project_updates::ProjectUpdateCommands::Archive { .. }
                | project_updates::ProjectUpdateCommands::Unarchive { .. },
        } => Some("project-updates"),
        Commands::Projects { action }
            if !matches!(
                action,
                projects::ProjectCommands::Update { .. }
                    | projects::ProjectCommands::List { .. }
                    | projects::ProjectCommands::Get { .. }
                    | projects::ProjectCommands::Members { .. }
            ) =>
        {
            Some("projects")
        }
        Commands::Documents {
            action: documents::DocumentCommands::Create { .. },
        } => Some("documents"),
        Commands::Roadmaps {
            action:
                roadmaps::RoadmapCommands::Create { .. } | roadmaps::RoadmapCommands::Delete { .. },
        } => Some("roadmaps"),
        Commands::Initiatives {
            action:
                initiatives::InitiativeCommands::Create { .. }
                | initiatives::InitiativeCommands::Delete { .. },
        } => Some("initiatives"),
        Commands::InitiativeUpdates {
            action:
                crate::commands::initiative_updates::InitiativeUpdateCommands::Create { .. }
                | crate::commands::initiative_updates::InitiativeUpdateCommands::Update { .. }
                | crate::commands::initiative_updates::InitiativeUpdateCommands::Archive { .. }
                | crate::commands::initiative_updates::InitiativeUpdateCommands::Unarchive { .. },
        } => Some("initiative-updates"),
        Commands::Milestones {
            action: crate::commands::milestones::MilestoneCommands::Delete { .. },
        } => Some("milestones"),
        Commands::Issues { action }
            if !matches!(
                action,
                crate::commands::issues::IssueCommands::Create { .. }
                    | crate::commands::issues::IssueCommands::Update { .. }
                    | crate::commands::issues::IssueCommands::List { .. }
                    | crate::commands::issues::IssueCommands::Get { .. }
                    | crate::commands::issues::IssueCommands::Link { .. }
                    | crate::commands::issues::IssueCommands::Describe { .. }
            ) =>
        {
            Some("issues")
        }
        Commands::Relations { action }
            if !matches!(
                action,
                crate::commands::relations::RelationCommands::List { .. }
            ) =>
        {
            Some("relations")
        }
        Commands::Sprint {
            action: sprint::SprintCommands::CarryOver { .. },
        } => Some("sprint"),
        Commands::Teams {
            action:
                teams::TeamCommands::Create { .. }
                | teams::TeamCommands::Update { .. }
                | teams::TeamCommands::Delete { .. },
        } => Some("teams"),
        Commands::Time {
            action:
                time::TimeCommands::Log { .. }
                | time::TimeCommands::Delete { .. }
                | time::TimeCommands::Update { .. },
        } => Some("time"),
        Commands::Triage {
            action: triage::TriageCommands::Claim { .. } | triage::TriageCommands::Snooze { .. },
        } => Some("triage"),
        Commands::Api {
            action:
                crate::commands::api::ApiCommands::Mutate { .. }
                | crate::commands::api::ApiCommands::Query { .. },
        } => Some("api"),
        Commands::Interactive { .. } => Some("interactive"),
        Commands::Git {
            action:
                git::GitCommands::Checkout { .. }
                | git::GitCommands::Create { .. }
                | git::GitCommands::Pr { .. },
        } => Some("git"),
        Commands::Setup => Some("setup"),
        Commands::Auth { action } if !matches!(action, auth::AuthCommands::Status { .. }) => {
            Some("auth")
        }
        Commands::Cache {
            action: crate::commands::cache::CacheCommands::Clear { .. },
        } => Some("cache"),
        Commands::Config {
            action:
                ConfigCommands::SetKey { .. }
                | ConfigCommands::Set { .. }
                | ConfigCommands::WorkspaceAdd { .. }
                | ConfigCommands::WorkspaceSwitch { .. }
                | ConfigCommands::WorkspaceRemove { .. },
        } => Some("config"),
        Commands::Doctor { fix: true, .. } => Some("doctor"),
        Commands::Webhooks {
            action:
                webhooks::WebhookCommands::RotateSecret { .. }
                | webhooks::WebhookCommands::Listen { .. },
        } => Some("webhooks"),
        Commands::Templates {
            action:
                templates::TemplateCommands::Create { .. }
                | templates::TemplateCommands::Delete { .. }
                | templates::TemplateCommands::RemoteCreate { .. }
                | templates::TemplateCommands::RemoteUpdate { .. }
                | templates::TemplateCommands::RemoteDelete { .. },
        } => Some("templates"),
        Commands::Uploads {
            action: uploads::UploadCommands::Fetch { file: Some(_), .. },
        } => Some("uploads"),
        Commands::Export {
            action:
                export::ExportCommands::Csv { file: Some(_), .. }
                | export::ExportCommands::Markdown { file: Some(_), .. }
                | export::ExportCommands::Json { file: Some(_), .. }
                | export::ExportCommands::ProjectsCsv { file: Some(_), .. },
        } => Some("export"),
        Commands::Hygiene {
            action:
                crate::commands::hygiene::HygieneCommands::Rules { init: true, .. }
                | crate::commands::hygiene::HygieneCommands::Snooze { list: false, .. },
            ..
        } => Some("hygiene"),
        Commands::Context {
            action:
                Some(
                    crate::commands::context::ContextCommands::Init(_)
                    | crate::commands::context::ContextCommands::Refresh { .. },
                ),
        } => Some("context"),
        _ => None,
    }
}
