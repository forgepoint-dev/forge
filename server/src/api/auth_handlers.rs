//! Authentication HTTP handlers for ATProto OAuth flow

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    Form,
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

/// Login form parameters
#[derive(Debug, Deserialize)]
pub struct LoginForm {
    pub handle: String,
}

/// Logout query parameters
#[derive(Debug, Deserialize)]
pub struct LogoutQuery {
    pub session_id: Option<String>,
}

/// Handler for displaying the login form
///
/// This displays a form where users enter their ATProto handle
pub async fn login_handler(State(_auth_state): State<Arc<AuthState>>) -> impl IntoResponse {
    let html = r#"<!DOCTYPE html>
<html>
<head>
    <title>Login with ATProto</title>
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
            max-width: 400px;
            width: 100%;
        }
        .form-group {
            margin: 20px 0;
        }
        input[type="text"] {
            width: 100%;
            padding: 12px;
            border: 1px solid #ddd;
            border-radius: 4px;
            font-size: 16px;
            box-sizing: border-box;
        }
        button {
            width: 100%;
            padding: 12px 24px;
            background-color: #0085ff;
            color: white;
            border: none;
            border-radius: 4px;
            font-weight: 500;
            font-size: 16px;
            cursor: pointer;
        }
        button:hover {
            background-color: #0070dd;
        }
        .help-text {
            font-size: 14px;
            color: #666;
            margin-top: 8px;
        }
    </style>
</head>
<body>
    <div class="container">
        <h1>Login to Forge</h1>
        <p>Enter your ATProto handle to authenticate</p>
        <form action="/auth/authorize" method="post">
            <div class="form-group">
                <input 
                    type="text" 
                    name="handle" 
                    placeholder="your-handle.bsky.social"
                    required
                    autofocus
                />
                <p class="help-text">Enter your Bluesky handle (e.g., alice.bsky.social)</p>
            </div>
            <button type="submit">Continue</button>
        </form>
    </div>
</body>
</html>"#;
    Html(html).into_response()
}

/// Handler for initiating OAuth authorization
///
/// This resolves the handle and redirects to the authorization server
pub async fn authorize_handler(
    State(auth_state): State<Arc<AuthState>>,
    Form(form): Form<LoginForm>,
) -> impl IntoResponse {
    match auth_state.oauth_client.get_authorization_url(form.handle).await {
        Ok((auth_url, _state)) => {
            // In production, store state in a secure cookie and verify in callback
            let html = format!(
                r#"<!DOCTYPE html>
<html>
<head>
    <title>Redirecting...</title>
    <meta http-equiv="refresh" content="0;url={}">
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
    </style>
</head>
<body>
    <div class="container">
        <h1>Redirecting to ATProto...</h1>
        <p>If you are not redirected automatically, <a href="{}">click here</a>.</p>
    </div>
</body>
</html>"#,
                auth_url, auth_url
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
    // For now, we'll use bsky.social as the PDS URL
    // In a full implementation, this would be stored in session state
    let pds_url = "https://bsky.social".to_string();

    // Exchange authorization code for access token
    let (access_token, refresh_token) = match auth_state
        .oauth_client
        .exchange_code(params.code, pds_url.clone())
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
    let user = match auth_state.oauth_client.get_user_profile(&access_token, &pds_url).await {
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

/// Handler for logout
/// 
/// Note: In production, session ID should be stored in a secure HTTP-only cookie
/// and retrieved from there. For now, we accept it as a query parameter or default
/// to clearing all sessions (backwards compatible with single-user mode).
pub async fn logout_handler(
    State(auth_state): State<Arc<AuthState>>,
    Query(params): Query<LogoutQuery>,
) -> axum::response::Response {
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

