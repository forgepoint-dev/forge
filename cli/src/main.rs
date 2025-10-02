use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

const DEFAULT_GRAPHQL_ENDPOINT: &str = "http://localhost:8000/graphql";

#[derive(Parser)]
#[command(name = "forge")]
#[command(about = "Forgepoint CLI - Remote repository management via HTTP", long_about = None)]
struct Cli {
    /// GraphQL API endpoint URL
    #[arg(long, default_value = DEFAULT_GRAPHQL_ENDPOINT)]
    api_url: String,

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

#[derive(Serialize)]
struct GraphQLRequest<T> {
    query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    variables: Option<T>,
}

#[derive(Deserialize, Debug)]
struct GraphQLResponse<T> {
    #[serde(default)]
    data: Option<T>,
    #[serde(default)]
    errors: Vec<GraphQLError>,
}

#[derive(Deserialize, Debug)]
struct GraphQLError {
    message: String,
}

#[derive(Serialize)]
struct CreateRepositoryVariables {
    input: CreateRepositoryInput,
}

#[derive(Serialize)]
struct CreateRepositoryInput {
    slug: String,
    group: Option<String>,
}

#[derive(Deserialize, Debug, Default)]
struct CreateRepositoryResponse {
    #[serde(rename = "createRepository")]
    create_repository: RepositoryNode,
}

#[derive(Deserialize, Debug, Default)]
struct RepositoryNode {
    id: String,
    slug: String,
    group: Option<GroupNode>,
}

#[derive(Deserialize, Debug)]
struct GroupNode {
    id: String,
    slug: String,
}

#[derive(Serialize)]
struct LinkRepositoryVariables {
    url: String,
}

#[derive(Deserialize, Debug, Default)]
struct LinkRepositoryResponse {
    #[serde(rename = "linkRemoteRepository")]
    link_remote_repository: RepositoryNode,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Repo(repo_cmd) => match repo_cmd {
            RepoCommands::Create { slug, group } => {
                create_repository(&cli.api_url, slug, group).await?
            }
            RepoCommands::Link { url } => link_repository(&cli.api_url, url).await?,
        },
    }

    Ok(())
}

async fn create_repository(api_url: &str, slug: String, group: Option<String>) -> Result<()> {
    let query = r#"
        mutation CreateRepository($input: CreateRepositoryInput!) {
            createRepository(input: $input) {
                id
                slug
                group {
                    id
                    slug
                }
            }
        }
    "#;

    let variables = CreateRepositoryVariables {
        input: CreateRepositoryInput { slug, group },
    };

    let request = GraphQLRequest {
        query: query.to_string(),
        variables: Some(variables),
    };

    let client = reqwest::Client::new();
    let response = client
        .post(api_url)
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .context("Failed to send request to GraphQL API")?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "GraphQL request failed with status: {}",
            response.status()
        ));
    }

    let graphql_response: GraphQLResponse<CreateRepositoryResponse> = response
        .json()
        .await
        .context("Failed to parse GraphQL response")?;

    if !graphql_response.errors.is_empty() {
        let error_messages: Vec<String> = graphql_response
            .errors
            .iter()
            .map(|e| e.message.clone())
            .collect();
        return Err(anyhow::anyhow!(
            "GraphQL errors: {}",
            error_messages.join(", ")
        ));
    }

    let data = graphql_response
        .data
        .context("No data returned from GraphQL")?;
    let repo = data.create_repository;

    println!("✓ Repository created successfully!");
    println!("  ID:   {}", repo.id);
    println!("  Slug: {}", repo.slug);
    if let Some(group) = repo.group {
        println!("  Group: {} ({})", group.slug, group.id);
    }

    Ok(())
}

async fn link_repository(api_url: &str, url: String) -> Result<()> {
    let query = r#"
        mutation LinkRemoteRepository($url: String!) {
            linkRemoteRepository(url: $url) {
                id
                slug
            }
        }
    "#;

    let variables = LinkRepositoryVariables { url };

    let request = GraphQLRequest {
        query: query.to_string(),
        variables: Some(variables),
    };

    let client = reqwest::Client::new();
    let response = client
        .post(api_url)
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .context("Failed to send request to GraphQL API")?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "GraphQL request failed with status: {}",
            response.status()
        ));
    }

    let graphql_response: GraphQLResponse<LinkRepositoryResponse> = response
        .json()
        .await
        .context("Failed to parse GraphQL response")?;

    if !graphql_response.errors.is_empty() {
        let error_messages: Vec<String> = graphql_response
            .errors
            .iter()
            .map(|e| e.message.clone())
            .collect();
        return Err(anyhow::anyhow!(
            "GraphQL errors: {}",
            error_messages.join(", ")
        ));
    }

    let data = graphql_response
        .data
        .context("No data returned from GraphQL")?;
    let repo = data.link_remote_repository;

    println!("✓ Remote repository linked successfully!");
    println!("  ID:   {}", repo.id);
    println!("  Slug: {}", repo.slug);

    Ok(())
}
