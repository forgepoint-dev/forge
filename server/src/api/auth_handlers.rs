//! Authentication HTTP handlers for ATProto OAuth flow

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    Form,
};
use axum::http::{HeaderMap, header};
use url::Url;
use serde::Deserialize;
use std::sync::Arc;

use crate::auth::{AtProtoAuthClient, SessionManager, SqliteAuthStore};

/// Shared auth state containing OAuth client and session manager
pub struct AuthState {
    pub oauth_client: AtProtoAuthClient,
    pub session_manager: SessionManager,
    pub auth_store: SqliteAuthStore,
}

/// OAuth callback parameters
#[derive(Debug, Deserialize)]
pub struct OAuthCallback {
    pub code: String,
    pub state: String,
    /// OAuth issuer returned by AS per ATProto spec
    pub iss: String,
}

/// Login form parameters
#[derive(Debug, Deserialize)]
pub struct LoginForm {
    pub handle: String,
}

/// Optional login query for return redirect
#[derive(Debug, Deserialize)]
pub struct LoginQuery {
    pub return_to: Option<String>,
}

/// Logout query parameters
#[derive(Debug, Deserialize)]
pub struct LogoutQuery {
    pub session_id: Option<String>,
}

fn parse_cookie<'a>(cookies: &'a str, name: &str) -> Option<String> {
    cookies.split(';')
        .map(|c| c.trim())
        .find_map(|c| c.strip_prefix(&format!("{}=", name)).map(|v| v.to_string()))
}

/// Handler for displaying the login form
///
/// This displays a form where users enter their ATProto handle
pub async fn login_handler(State(_auth_state): State<Arc<AuthState>>, Query(q): Query<LoginQuery>) -> impl IntoResponse {
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
    // If a return_to is provided, store in a short-lived cookie
    if let Some(rt) = q.return_to {
        let mut headers = HeaderMap::new();
        let cookie = format!("forge_return_to={}; Path=/; Max-Age=600; SameSite=Lax", urlencoding::encode(&rt));
        headers.insert(header::SET_COOKIE, header::HeaderValue::from_str(&cookie).unwrap_or(header::HeaderValue::from_static("")));
        return (StatusCode::OK, headers, Html(html.to_string())).into_response();
    }
    Html(html).into_response()
}

/// Serve OAuth Dynamic Client Metadata for public clients
pub async fn client_metadata_handler(State(auth_state): State<Arc<AuthState>>) -> impl IntoResponse {
    // Minimal metadata for a public client (no client_secret)
    let metadata = serde_json::json!({
        "client_id": auth_state.oauth_client.client_id(),
        "client_name": "Forgepoint",
        "application_type": "web",
        "redirect_uris": [ auth_state.oauth_client.redirect_uri() ],
        "grant_types": ["authorization_code", "refresh_token"],
        "response_types": ["code"],
        "token_endpoint_auth_method": "none",
        "dpop_bound_access_tokens": true,
        "scope": auth_state.oauth_client.scope()
    });
    (
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        serde_json::to_string(&metadata).unwrap_or("{}".to_string()),
    )
}

/// Health check for OAuth setup: validates client_id and redirect_uri consistency
pub async fn auth_health_handler(State(auth_state): State<Arc<AuthState>>) -> impl IntoResponse {
    let client_id = auth_state.oauth_client.client_id().to_string();
    let redirect_uri = auth_state.oauth_client.redirect_uri().to_string();
    let public_base = std::env::var("FORGE_PUBLIC_BASE_URL").unwrap_or_else(|_| "http://localhost:8000".to_string());

    let mut issues: Vec<String> = Vec::new();
    let mut ok = true;

    // Parse URLs
    let rid = Url::parse(&redirect_uri);
    if rid.is_err() { ok = false; issues.push("ATPROTO_REDIRECT_URI is not a valid URL".into()); }
    let pbase = Url::parse(&public_base);
    if pbase.is_err() { ok = false; issues.push("FORGE_PUBLIC_BASE_URL is not a valid URL".into()); }

    if let (Ok(rid), Ok(pbase)) = (rid, pbase) {
        // Ensure redirect path
        if rid.path() != "/auth/callback" { ok = false; issues.push("redirect_uri path should be /auth/callback".into()); }
        // Avoid localhost in redirect_uri per RFC 8252; use 127.0.0.1 instead
        if rid.host_str() == Some("localhost") { ok = false; issues.push("redirect_uri must not use hostname 'localhost'; use 127.0.0.1".into()); }
        // Ensure same host when not using localhost client
        let is_local = client_id.starts_with("http://localhost");
        if !is_local && (rid.host_str() != pbase.host_str()) {
            ok = false; issues.push("redirect_uri host must match FORGE_PUBLIC_BASE_URL host".into());
        }
        if !is_local && pbase.scheme() != "https" { ok = false; issues.push("FORGE_PUBLIC_BASE_URL should use https in production".into()); }
        if !is_local {
            if !client_id.ends_with("/client-metadata.json") {
                ok = false; issues.push("client_id should point to /client-metadata.json for non-localhost".into());
            }
        }
    }

    let body = serde_json::json!({
        "ok": ok,
        "client_id": client_id,
        "redirect_uri": redirect_uri,
        "public_base": public_base,
        "issues": issues,
    });
    (StatusCode::OK, axum::Json(body))
}

/// Admin endpoint to vacuum the auth DB.
pub async fn auth_vacuum_handler(State(auth_state): State<Arc<AuthState>>) -> impl IntoResponse {
    match auth_state.auth_store.vacuum().await {
        Ok(()) => (StatusCode::OK, axum::Json(serde_json::json!({"ok": true }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, axum::Json(serde_json::json!({"ok": false, "error": e.to_string()}))).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{AtProtoAuthClient, AuthConfig, SqliteAuthStore};
    use axum::extract::State as AxumState;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_auth_health_handler_ok_localhost() {
        let config = AuthConfig {
            client_id: "http://localhost".into(),
            client_secret: None,
            redirect_uri: "http://localhost:8000/auth/callback".into(),
            scope: "atproto".into(),
        };
        let oauth_client = AtProtoAuthClient::new(config).unwrap();
        let store = SqliteAuthStore::new(tempdir().unwrap().path().join("auth.db").to_str().unwrap()).await.unwrap();
        let state = Arc::new(AuthState { oauth_client, session_manager: SessionManager::new(), auth_store: store });
        let resp = auth_health_handler(AxumState(state)).await.into_response();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}

/// Handler for initiating OAuth authorization
///
/// This resolves the handle and redirects to the authorization server
pub async fn authorize_handler(
    State(auth_state): State<Arc<AuthState>>,
    Form(form): Form<LoginForm>,
) -> impl IntoResponse {
    match auth_state.oauth_client.get_authorization_url(&auth_state.auth_store, form.handle).await {
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
            let body = format!("<h1>Failed to initiate login</h1><pre>{}</pre>", err);
            (StatusCode::INTERNAL_SERVER_ERROR, Html(body)).into_response()
        }
    }
}

/// Handler for OAuth callback
///
/// This exchanges the authorization code for an access token and creates a session
pub async fn callback_handler(
    State(auth_state): State<Arc<AuthState>>,
    Query(params): Query<OAuthCallback>,
    headers_in: HeaderMap,
) -> impl IntoResponse {
    // Exchange authorization code for access token
    let (access_token, refresh_token, _issuer, subject_did) = match auth_state
        .oauth_client
        .exchange_code(&auth_state.auth_store, params.code.clone(), params.state.clone(), params.iss.clone())
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

    // Determine PDS URL used earlier in the flow
    // Load flow record (contains DPoP key); do this before exchange if you want to be strict,
    // but exchange_code no longer deletes it, so loading after works too.
    let flow_rec = auth_state.auth_store.get(&params.state).await.ok().flatten();
    // Prefer PDS from DID doc when available
    let pds_url = if let Some(did) = subject_did.as_deref() {
        match auth_state.oauth_client.discover_pds_from_did(did).await {
            Ok(url) => url,
            Err(e) => {
                tracing::warn!("Failed to resolve PDS from DID ({}), falling back to flow PDS: {}", did, e);
                flow_rec.as_ref().map(|r| r.pds_url.clone()).unwrap_or_else(|| "https://bsky.social".to_string())
            }
        }
    } else {
        flow_rec.as_ref().map(|r| r.pds_url.clone()).unwrap_or_else(|| "https://bsky.social".to_string())
    };

    // Fetch user profile
    let user = if let Some(rec) = &flow_rec {
        let jwk: serde_json::Value = serde_json::from_str(&rec.dpop_jwk).unwrap_or(serde_json::json!({}));
        match auth_state.oauth_client.get_user_profile_with_key(&access_token, &pds_url, &rec.dpop_pkcs8, &jwk).await {
            Ok(u) => Ok(u),
            Err(e) => Err(e),
        }
    } else {
        auth_state.oauth_client.get_user_profile(&access_token, &pds_url, None).await
    };
    let user = match user {
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
        .create_session(
            user.clone(),
            access_token,
            refresh_token,
            flow_rec.as_ref().map(|r| r.dpop_pkcs8.clone()),
            flow_rec.as_ref().map(|r| r.dpop_jwk.clone()),
        )
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

    // Cleanup flow record now that session is established
    let _ = auth_state.auth_store.delete(&params.state).await;

    tracing::info!("User {} authenticated successfully", user.handle);

    // Build session cookie (Secure only when appropriate)
    let cookie_domain = std::env::var("FORGE_COOKIE_DOMAIN").ok();
    let mut after_login = std::env::var("FORGE_WEB_AFTER_LOGIN_URL").unwrap_or_else(|_| "/".to_string());
    // Prefer return_to cookie if present
    let return_to = headers_in
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|c| parse_cookie(c, "forge_return_to"));
    if let Some(rt) = return_to {
        let decoded = urlencoding::decode(&rt).map(|cow| cow.into_owned()).unwrap_or(rt);
        after_login = decoded;
    }
    let mut cookie = format!("forge_session={}; Path=/; HttpOnly; SameSite=Lax", session_id);
    // Decide if cookie should be Secure
    let cookie_secure = std::env::var("FORGE_COOKIE_SECURE")
        .ok()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or_else(|| {
            std::env::var("FORGE_PUBLIC_BASE_URL")
                .map(|u| u.starts_with("https://"))
                .unwrap_or(false)
        });
    if cookie_secure { cookie.push_str("; Secure"); }
    if let Some(ref domain) = cookie_domain { cookie.push_str(&format!("; Domain={}", domain)); }
    // Do NOT set Domain for IP literals like 127.0.0.1. Many browsers ignore or reject
    // cookies with a Domain attribute that is an IP address (RFC 6265). Omitting Domain
    // yields a host-only cookie which works across all ports for the host.
    // Default max-age 7 days
    cookie.push_str("; Max-Age=604800");

    let mut headers = HeaderMap::new();
    tracing::debug!(target: "auth", secure = cookie_secure, domain = %cookie_domain.as_deref().unwrap_or("<host-only>"), "callback_handler: setting forge_session cookie");
    headers.insert(header::SET_COOKIE, header::HeaderValue::from_str(&cookie).unwrap_or(header::HeaderValue::from_static("")));
    // Optional debug cookie (non-HttpOnly) to verify presence in devtools
    if std::env::var("FORGE_DEBUG_COOKIES").ok().and_then(|v| v.parse::<bool>().ok()).unwrap_or(false) {
        let dbg = format!("forge_session_dbg={}; Path=/; SameSite=Lax{}{}",
            session_id,
            if cookie_secure { "; Secure" } else { "" },
            if cookie_domain.is_some() { "" } else if std::env::var("FORGE_PUBLIC_BASE_URL").map(|u| u.contains("127.0.0.1")).unwrap_or(false) { "; Domain=127.0.0.1" } else { "" }
        );
        headers.append(header::SET_COOKIE, header::HeaderValue::from_str(&dbg).unwrap_or(header::HeaderValue::from_static("")));
    }
    // Clear return_to cookie
    headers.append(header::SET_COOKIE, header::HeaderValue::from_static("forge_return_to=; Path=/; Max-Age=0; SameSite=Lax"));
    headers.insert(header::LOCATION, header::HeaderValue::from_str(&after_login).unwrap_or(header::HeaderValue::from_static("/")));
    (StatusCode::FOUND, headers).into_response()
}

/// Handler for logout
/// 
/// Note: In production, session ID should be stored in a secure HTTP-only cookie
/// and retrieved from there. For now, we accept it as a query parameter or default
/// to clearing all sessions (backwards compatible with single-user mode).
pub async fn logout_handler(
    State(auth_state): State<Arc<AuthState>>,
    Query(params): Query<LogoutQuery>,
    headers: HeaderMap,
) -> axum::response::Response {
    // Prefer cookie if present
    let mut session_id_opt = None;
    if let Some(cookie_hdr) = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()) {
        session_id_opt = parse_cookie(cookie_hdr, "forge_session");
    }
    if session_id_opt.is_none() { session_id_opt = params.session_id; }

    if let Some(session_id) = session_id_opt {
        if let Err(err) = auth_state.session_manager.delete_session(&session_id) {
            tracing::error!("Failed to delete session: {}", err);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to logout").into_response();
        }
        let mut headers_out = HeaderMap::new();
        // Clear cookie (respect Secure attribute setting)
        let cookie_secure = std::env::var("FORGE_COOKIE_SECURE")
            .ok()
            .and_then(|v| v.parse::<bool>().ok())
            .unwrap_or_else(|| {
                std::env::var("FORGE_PUBLIC_BASE_URL")
                    .map(|u| u.starts_with("https://"))
                    .unwrap_or(false)
            });
        let clear = if cookie_secure {
            "forge_session=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Lax"
        } else {
            "forge_session=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax"
        };
        headers_out.insert(header::SET_COOKIE, header::HeaderValue::from_static(clear));
        return (StatusCode::OK, headers_out).into_response();
    }

    (StatusCode::BAD_REQUEST, "No session to logout").into_response()
}

/// Return the current authenticated user based on forge_session cookie
pub async fn me_handler(
    State(auth_state): State<Arc<AuthState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let has_cookie_hdr = headers.get(header::COOKIE).is_some();
    tracing::debug!(target: "auth", has_cookie_hdr, "me_handler: received request");
    if let Some(cookie_hdr) = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()) {
        let session_id_opt = parse_cookie(cookie_hdr, "forge_session");
        tracing::debug!(target: "auth", has_cookie_hdr, has_session_cookie = session_id_opt.is_some(), "me_handler: parsed cookie header");
        if let Some(session_id) = session_id_opt {
            match auth_state.session_manager.get_session(&session_id) {
                Ok(Some(session)) => {
                    tracing::debug!(target: "auth", "me_handler: session found for user");
                    let body = serde_json::json!({
                        "authenticated": true,
                        "user": {
                            "did": session.user.did,
                            "handle": session.user.handle,
                            "displayName": session.user.display_name,
                            "avatar": session.user.avatar,
                        }
                    });
                    return (StatusCode::OK, axum::Json(body)).into_response();
                }
                Ok(None) => {
                    tracing::debug!(target: "auth", "me_handler: no matching session for cookie");
                }
                Err(e) => {
                    tracing::error!(target: "auth", error = %e, "me_handler: failed to load session");
                }
            }
        }
    }
    let body = serde_json::json!({ "authenticated": false });
    (StatusCode::OK, axum::Json(body)).into_response()
}
