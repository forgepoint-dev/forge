use anyhow::{Context as _, Result};
use clap::{Parser, Subcommand};
use sqlx::SqlitePool;

// Import only the minimal modules needed
mod db {
    pub use server::db::{init_pool, normalize_path};
}

mod repository {
    pub use server::repository::{
        mutations::{create_repository_raw, link_remote_repository_raw, CreateRepositoryInput},
        RepositoryStorage,
    };
}

mod validation {
    pub use server::validation::slug::validate_slug;
}

#[derive(Parser)]
#[command(name = "forge")]
#[command(about = "Forgepoint CLI - Manage repositories and groups", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(subcommand)]
    Repo(RepoCommands),
}

#[derive(Subcommand)]
enum RepoCommands {
    /// Create a new repository
    Create {
        /// Repository slug (lowercase, alphanumeric, hyphens only)
        slug: String,
        /// Optional group ID to create the repository in
        #[arg(short, long)]
        group: Option<String>,
    },
    /// Link a remote repository
    Link {
        /// Remote repository URL (e.g., https://github.com/user/repo)
        url: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    // Initialize database connection
    let (pool, db_root_path) = db::init_pool().await?;

    // Handle repository paths
    let repos_root = if std::env::var("FORGE_IN_MEMORY_DB").unwrap_or_default() == "true" {
        std::env::temp_dir().join("forge-repos")
    } else {
        let repos_root_raw = std::env::var("FORGE_REPOS_PATH")
            .with_context(|| "FORGE_REPOS_PATH environment variable must be set".to_string())?;
        db::normalize_path(repos_root_raw)?
    };

    std::fs::create_dir_all(&repos_root).with_context(|| {
        format!(
            "failed to create repository root directory: {}",
            repos_root.display()
        )
    })?;

    let remote_cache_root = db_root_path.join("remote-cache");
    std::fs::create_dir_all(&remote_cache_root).with_context(|| {
        format!(
            "failed to create remote cache directory: {}",
            remote_cache_root.display()
        )
    })?;

    let storage = repository::RepositoryStorage::new(repos_root, remote_cache_root);

    match cli.command {
        Commands::Repo(repo_cmd) => match repo_cmd {
            RepoCommands::Create { slug, group } => {
                create_repository(&pool, &storage, slug, group).await?
            }
            RepoCommands::Link { url } => link_repository(&pool, &storage, url).await?,
        },
    }

    Ok(())
}

async fn create_repository(
    pool: &SqlitePool,
    storage: &repository::RepositoryStorage,
    slug: String,
    group: Option<String>,
) -> Result<()> {
    // Validate slug
    validation::validate_slug(&slug)
        .map_err(|e| anyhow::anyhow!("Invalid slug: {}", e))?;

    let input = repository::CreateRepositoryInput { slug, group };

    let repo = repository::create_repository_raw(pool, input).await?;

    // Create the repository directory
    let working_copy_path = storage.get_repository_path(&repo.id);
    std::fs::create_dir_all(&working_copy_path).with_context(|| {
        format!(
            "failed to create repository directory: {}",
            working_copy_path.display()
        )
    })?;

    println!("✓ Repository created successfully!");
    println!("  ID:   {}", repo.id);
    println!("  Slug: {}", repo.slug);
    if let Some(group_id) = repo.group_id {
        println!("  Group: {}", group_id);
    }
    println!("  Path: {}", working_copy_path.display());

    Ok(())
}

async fn link_repository(
    pool: &SqlitePool,
    storage: &repository::RepositoryStorage,
    url: String,
) -> Result<()> {
    let repo = repository::link_remote_repository_raw(pool, storage, url).await?;

    println!("✓ Remote repository linked successfully!");
    println!("  ID:   {}", repo.id);
    println!("  Slug: {}", repo.slug);
    if let Some(remote_url) = repo.remote_url {
        println!("  URL:  {}", remote_url);
    }

    Ok(())
}

