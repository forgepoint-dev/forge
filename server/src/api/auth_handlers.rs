//! Authentication HTTP handlers for OAuth flow

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
};
use serde::Deserialize;
use std::sync::Arc;

use crate::auth::{AtProtoAuthClient, SessionManager};

/// Shared auth state containing OAuth client and session manager
pub struct AuthState {
    pub oauth_client: AtProtoAuthClient,
    pub session_manager: SessionManager,
}

/// OAuth callback parameters
#[derive(Debug, Deserialize)]
pub struct OAuthCallback {
    pub code: String,
    pub state: String,
}

/// Handler for initiating OAuth login
///
/// This generates an authorization URL and redirects the user to the OAuth provider
pub async fn login_handler(State(auth_state): State<Arc<AuthState>>) -> impl IntoResponse {
    match auth_state.oauth_client.get_authorization_url() {
        Ok((auth_url, _csrf_token)) => {
            // In production, you'd store csrf_token in a secure cookie and verify it in callback
            let html = format!(
                r#"<!DOCTYPE html>
<html>
<head>
    <title>Login with ATProto</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
            display: flex;
            justify-content: center;
            align-items: center;
            height: 100vh;
            margin: 0;
            background-color: #f5f5f5;
        }}
        .container {{
            text-align: center;
            background: white;
            padding: 40px;
            border-radius: 8px;
            box-shadow: 0 2px 10px rgba(0,0,0,0.1);
        }}
        .button {{
            display: inline-block;
            padding: 12px 24px;
            background-color: #0085ff;
            color: white;
            text-decoration: none;
            border-radius: 4px;
            font-weight: 500;
        }}
        .button:hover {{
            background-color: #0070dd;
        }}
    </style>
</head>
<body>
    <div class="container">
        <h1>Login to Forge</h1>
        <p>Click the button below to authenticate with your ATProto account.</p>
        <a href="{}" class="button">Login with ATProto</a>
    </div>
</body>
</html>"#,
                auth_url
            );
            Html(html).into_response()
        }
        Err(err) => {
            tracing::error!("Failed to generate authorization URL: {}", err);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to initiate login").into_response()
        }
    }
}

/// Handler for OAuth callback
///
/// This exchanges the authorization code for an access token and creates a session
pub async fn callback_handler(
    State(auth_state): State<Arc<AuthState>>,
    Query(params): Query<OAuthCallback>,
) -> impl IntoResponse {
    // Exchange authorization code for access token
    let (access_token, refresh_token) = match auth_state
        .oauth_client
        .exchange_code(params.code)
        .await
    {
        Ok(tokens) => tokens,
        Err(err) => {
            tracing::error!("Failed to exchange authorization code: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html("<h1>Authentication Failed</h1><p>Failed to exchange authorization code.</p>".to_string())
            ).into_response();
        }
    };

    // Fetch user profile
    let user = match auth_state.oauth_client.get_user_profile(&access_token).await {
        Ok(user) => user,
        Err(err) => {
            tracing::error!("Failed to fetch user profile: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html("<h1>Authentication Failed</h1><p>Failed to fetch user profile.</p>".to_string())
            ).into_response();
        }
    };

    // Create session
    let session_id = match auth_state
        .session_manager
        .create_session(user.clone(), access_token, refresh_token)
    {
        Ok(id) => id,
        Err(err) => {
            tracing::error!("Failed to create session: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html("<h1>Authentication Failed</h1><p>Failed to create session.</p>".to_string())
            ).into_response();
        }
    };

    tracing::info!("User {} authenticated successfully", user.handle);

    // Return success page with session info
    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>Login Successful</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
            display: flex;
            justify-content: center;
            align-items: center;
            height: 100vh;
            margin: 0;
            background-color: #f5f5f5;
        }}
        .container {{
            text-align: center;
            background: white;
            padding: 40px;
            border-radius: 8px;
            box-shadow: 0 2px 10px rgba(0,0,0,0.1);
        }}
        .success {{
            color: #10b981;
        }}
        .code {{
            background: #f5f5f5;
            padding: 8px 12px;
            border-radius: 4px;
            font-family: monospace;
            margin: 16px 0;
        }}
    </style>
</head>
<body>
    <div class="container">
        <h1 class="success">✓ Login Successful</h1>
        <p>Welcome, <strong>{}</strong>!</p>
        <p>Session ID: <code class="code">{}</code></p>
        <p>You can now close this window and use the GraphQL API.</p>
    </div>
</body>
</html>"#,
        user.handle, session_id
    );

    Html(html).into_response()
}

/// Logout query parameters
#[derive(Debug, Deserialize)]
pub struct LogoutQuery {
    pub session_id: Option<String>,
}

/// Handler for logout
/// 
/// Note: In production, session ID should be stored in a secure HTTP-only cookie
/// and retrieved from there. For now, we accept it as a query parameter or default
/// to clearing all sessions (backwards compatible with single-user mode).
pub async fn logout_handler(
    State(auth_state): State<Arc<AuthState>>,
    Query(params): Query<LogoutQuery>,
) -> impl IntoResponse {
    let result = if let Some(session_id) = params.session_id {
        // Delete specific session
        auth_state.session_manager.delete_session(&session_id)
    } else {
        // For backwards compatibility, show message that session ID is required
        return Html(r#"<!DOCTYPE html>
<html>
<head>
    <title>Logout</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
            display: flex;
            justify-content: center;
            align-items: center;
            height: 100vh;
            margin: 0;
            background-color: #f5f5f5;
        }
        .container {
            text-align: center;
            background: white;
            padding: 40px;
            border-radius: 8px;
            box-shadow: 0 2px 10px rgba(0,0,0,0.1);
        }
    </style>
</head>
<body>
    <div class="container">
        <h1>Logout</h1>
        <p>Please provide your session ID as a query parameter: <code>?session_id=YOUR_SESSION_ID</code></p>
        <p>In production, sessions would be managed via secure HTTP-only cookies.</p>
    </div>
</body>
</html>"#).into_response();
    };

    match result {
        Ok(_) => {
            let html = r#"<!DOCTYPE html>
<html>
<head>
    <title>Logged Out</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
            display: flex;
            justify-content: center;
            align-items: center;
            height: 100vh;
            margin: 0;
            background-color: #f5f5f5;
        }
        .container {
            text-align: center;
            background: white;
            padding: 40px;
            border-radius: 8px;
            box-shadow: 0 2px 10px rgba(0,0,0,0.1);
        }
    </style>
</head>
<body>
    <div class="container">
        <h1>Logged Out</h1>
        <p>You have been successfully logged out.</p>
    </div>
</body>
</html>"#;
            Html(html).into_response()
        }
        Err(err) => {
            tracing::error!("Failed to delete session: {}", err);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to logout").into_response()
        }
    }
}
