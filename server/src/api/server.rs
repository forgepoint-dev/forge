use anyhow::Result;
use axum::extract::State;
use axum::http::{Method, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tower_http::cors::{Any, CorsLayer};

use super::auth_handlers::{self, AuthState};
use super::playground::graphql_playground;
use crate::router::{GraphQLExecutionRequest, RouterState};
use axum::response::IntoResponse;
use std::io;

/// Combined application state
#[derive(Clone)]
pub struct AppState {
    pub router: Arc<RouterState>,
    pub auth: Option<Arc<AuthState>>,
}

/// GraphQL request structure
#[derive(Debug, Deserialize)]
pub struct GraphQLRequest {
    pub query: String,
    #[serde(default)]
    pub operation_name: Option<String>,
    #[serde(default)]
    pub variables: serde_json::Value,
}

pub async fn graphql_handler(
    State(app_state): State<AppState>,
    Json(req): Json<GraphQLRequest>,
) -> Json<JsonValue> {
    let exec_request = match GraphQLExecutionRequest::from_payload(&req) {
        Ok(req) => req,
        Err(err) => return Json(graphql_error_body(err.to_string())),
    };

    match app_state.router.execute(exec_request).await {
        Ok(json) => Json(json),
        Err(err) => Json(graphql_error_body(err.to_string())),
    }
}

pub async fn graphql_options() -> StatusCode {
    StatusCode::NO_CONTENT
}

pub fn build_api_router(app_state: AppState) -> Router {
    let mut router = Router::new()
        .route("/", get(graphql_playground))
        .route("/graphql", post(graphql_handler).options(graphql_options));

    // Add auth routes if auth is configured
    if app_state.auth.is_some() {
        router = router
            .route("/auth/login", get(auth_login_handler))
            .route("/auth/authorize", post(auth_authorize_handler))
            .route("/auth/callback", get(auth_callback_handler))
            .route("/auth/logout", get(auth_logout_handler));
    }

    router
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_headers(Any)
                .allow_methods([Method::POST, Method::OPTIONS, Method::GET]),
        )
        .with_state(app_state)
}

<<<<<<< HEAD
// Wrapper handlers that extract auth state from AppState
async fn auth_login_handler(State(app_state): State<AppState>) -> axum::response::Response {
    if let Some(auth_state) = app_state.auth {
        auth_handlers::login_handler(State(auth_state)).await.into_response()
    } else {
        (StatusCode::NOT_FOUND, "Authentication not configured").into_response()
    }
}

async fn auth_authorize_handler(
    State(app_state): State<AppState>,
    form: axum::Form<auth_handlers::LoginForm>,
) -> axum::response::Response {
    if let Some(auth_state) = app_state.auth {
        auth_handlers::authorize_handler(State(auth_state), form).await.into_response()
    } else {
        (StatusCode::NOT_FOUND, "Authentication not configured").into_response()
    }
}

async fn auth_callback_handler(
    State(app_state): State<AppState>,
    query: axum::extract::Query<auth_handlers::OAuthCallback>,
) -> axum::response::Response {
    if let Some(auth_state) = app_state.auth {
        auth_handlers::callback_handler(State(auth_state), query).await.into_response()
    } else {
        (StatusCode::NOT_FOUND, "Authentication not configured").into_response()
    }
}

async fn auth_logout_handler(
    State(app_state): State<AppState>,
    query: axum::extract::Query<auth_handlers::LogoutQuery>,
) -> axum::response::Response {
    if let Some(auth_state) = app_state.auth {
        auth_handlers::logout_handler(State(auth_state), query).await.into_response()
    } else {
        (StatusCode::NOT_FOUND, "Authentication not configured").into_response()
    }
}

pub async fn run_api(
    router_state: Arc<RouterState>,
    auth_state: Option<Arc<AuthState>>,
    shutdown: CancellationToken,
) -> Result<()> {
    let app_state = AppState {
        router: router_state,
        auth: auth_state,
    };

    let default_addr = "0.0.0.0:8000".to_string();

    let configured_addr = std::env::var("FORGE_API_ADDR")
        .or_else(|_| {
            std::env::var("FORGE_API_PORT").map(|port| format!("0.0.0.0:{}", port))
        })
        .or_else(|_| std::env::var("PORT").map(|port| format!("0.0.0.0:{}", port)))
        .unwrap_or(default_addr);

    let listener = match tokio::net::TcpListener::bind(&configured_addr).await {
        Ok(listener) => listener,
        Err(err) if err.kind() == io::ErrorKind::AddrInUse => {
            tracing::warn!(
                "Address {} already in use, falling back to an ephemeral port",
                configured_addr
            );
            tokio::net::TcpListener::bind("0.0.0.0:0").await?
        }
        Err(err) => return Err(err.into()),
    };

    if let Ok(addr) = listener.local_addr() {
        tracing::info!("Forge API listening on {}", addr);
    }

    axum::serve(listener, build_api_router(app_state))
        .with_graceful_shutdown(shutdown.cancelled_owned())
        .await?;
    Ok(())
}

fn graphql_error_body(message: String) -> JsonValue {
    JsonValue::Object(serde_json::Map::from_iter([(
        "errors".to_string(),
        JsonValue::Array(vec![JsonValue::Object(serde_json::Map::from_iter([(
            "message".to_string(),
            JsonValue::String(message),
        )]))]),
    )]))
}
