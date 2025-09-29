use anyhow::Result;
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::Router;
use axum::extract::State;
use axum::http::{Method, StatusCode};
use axum::routing::{get, post};
use tokio_util::sync::CancellationToken;
use tower_http::cors::{Any, CorsLayer};

use super::playground::graphql_playground;
use crate::graphql::AppSchema;

pub async fn graphql_handler(
    State(schema): State<AppSchema>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    schema.execute(req.into_inner()).await.into()
}

pub async fn graphql_options() -> StatusCode {
    StatusCode::NO_CONTENT
}

pub fn build_api_router(schema: AppSchema) -> Router {
    Router::new()
        .route("/", get(graphql_playground))
        .route("/graphql", post(graphql_handler).options(graphql_options))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_headers(Any)
                .allow_methods([Method::POST, Method::OPTIONS]),
        )
        .with_state(schema)
}

pub async fn run_api(schema: AppSchema, shutdown: CancellationToken) -> Result<()> {
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await?;
    axum::serve(listener, build_api_router(schema))
        .with_graceful_shutdown(shutdown.cancelled_owned())
        .await?;
    Ok(())
}
