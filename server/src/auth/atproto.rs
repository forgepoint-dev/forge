//! ATProto OAuth client
//!
//! This module implements OAuth authentication with ATProto services (e.g., Bluesky).
//! It follows the ATProto OAuth specification for authenticating users.

use super::User;
use anyhow::{Context, Result};
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, Scope, TokenResponse, TokenUrl,
    basic::BasicClient,
    reqwest::async_http_client,
};
use serde::Deserialize;
use std::sync::{Arc, Mutex};

/// Configuration for ATProto OAuth
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// OAuth client ID
    pub client_id: String,
    /// OAuth client secret
    pub client_secret: String,
    /// Authorization endpoint
    pub auth_url: String,
    /// Token endpoint
    pub token_url: String,
    /// Redirect URI (callback URL)
    pub redirect_uri: String,
}

impl AuthConfig {
    /// Create default configuration for Bluesky
    pub fn bluesky_default(client_id: String, client_secret: String, redirect_uri: String) -> Self {
        Self {
            client_id,
            client_secret,
            // Bluesky OAuth endpoints (these are example values - actual endpoints may vary)
            auth_url: "https://bsky.social/oauth/authorize".to_string(),
            token_url: "https://bsky.social/oauth/token".to_string(),
            redirect_uri,
        }
    }
}

/// ATProto OAuth client
pub struct AtProtoAuthClient {
    oauth_client: BasicClient,
    pkce_verifier: Arc<Mutex<Option<PkceCodeVerifier>>>,
}

impl AtProtoAuthClient {
    /// Create a new ATProto OAuth client
    pub fn new(config: AuthConfig) -> Result<Self> {
        let client_id = ClientId::new(config.client_id);
        let client_secret = ClientSecret::new(config.client_secret);
        let auth_url = AuthUrl::new(config.auth_url)
            .context("Invalid authorization endpoint URL")?;
        let token_url = TokenUrl::new(config.token_url)
            .context("Invalid token endpoint URL")?;
        let redirect_url = RedirectUrl::new(config.redirect_uri)
            .context("Invalid redirect URI")?;

        let oauth_client = BasicClient::new(
            client_id,
            Some(client_secret),
            auth_url,
            Some(token_url),
        )
        .set_redirect_uri(redirect_url);

        Ok(Self {
            oauth_client,
            pkce_verifier: Arc::new(Mutex::new(None)),
        })
    }

    /// Generate authorization URL for user to initiate OAuth flow
    ///
    /// Returns (authorization_url, csrf_token)
    pub fn get_authorization_url(&self) -> Result<(String, String)> {
        // Generate PKCE challenge for enhanced security
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        // Store PKCE verifier for later use
        {
            let mut verifier = self.pkce_verifier.lock()
                .map_err(|e| anyhow::anyhow!("Failed to lock PKCE verifier: {}", e))?;
            *verifier = Some(pkce_verifier);
        }

        // Generate authorization URL
        let (auth_url, csrf_token) = self
            .oauth_client
            .authorize_url(CsrfToken::new_random)
            .add_scope(Scope::new("atproto".to_string()))
            .set_pkce_challenge(pkce_challenge)
            .url();

        Ok((auth_url.to_string(), csrf_token.secret().to_string()))
    }

    /// Exchange authorization code for access token
    pub async fn exchange_code(&self, code: String) -> Result<(String, Option<String>)> {
        // Retrieve PKCE verifier
        let pkce_verifier = {
            let mut verifier = self.pkce_verifier.lock()
                .map_err(|e| anyhow::anyhow!("Failed to lock PKCE verifier: {}", e))?;
            verifier.take()
                .ok_or_else(|| anyhow::anyhow!("No PKCE verifier found"))?
        };

        // Exchange authorization code for access token
        let token_response = self
            .oauth_client
            .exchange_code(AuthorizationCode::new(code))
            .set_pkce_verifier(pkce_verifier)
            .request_async(async_http_client)
            .await
            .context("Failed to exchange authorization code")?;

        let access_token = token_response.access_token().secret().to_string();
        let refresh_token = token_response.refresh_token()
            .map(|t| t.secret().to_string());

        Ok((access_token, refresh_token))
    }

    /// Fetch user profile using access token
    pub async fn get_user_profile(&self, access_token: &str) -> Result<User> {
        // Make request to ATProto API to get user profile
        let client = reqwest::Client::new();
        
        // ATProto session endpoint to get current user info
        let response = client
            .get("https://bsky.social/xrpc/com.atproto.server.getSession")
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await
            .context("Failed to fetch user profile")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Failed to fetch user profile: {}",
                response.status()
            ));
        }

        let profile: AtProtoSession = response.json().await
            .context("Failed to parse user profile")?;

        Ok(User {
            did: profile.did,
            handle: profile.handle,
            display_name: None, // Could be fetched from additional API call
            avatar: None,
        })
    }
}

/// ATProto session response structure
#[derive(Debug, Deserialize)]
struct AtProtoSession {
    did: String,
    handle: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_config_creation() {
        let config = AuthConfig::bluesky_default(
            "test-client-id".to_string(),
            "test-secret".to_string(),
            "http://localhost:8000/callback".to_string(),
        );
        
        assert_eq!(config.client_id, "test-client-id");
        assert!(config.auth_url.contains("bsky.social"));
    }

    #[test]
    fn test_oauth_client_creation() {
        let config = AuthConfig::bluesky_default(
            "test-client-id".to_string(),
            "test-secret".to_string(),
            "http://localhost:8000/callback".to_string(),
        );
        
        let client = AtProtoAuthClient::new(config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_authorization_url_generation() {
        let config = AuthConfig::bluesky_default(
            "test-client-id".to_string(),
            "test-secret".to_string(),
            "http://localhost:8000/callback".to_string(),
        );
        
        let client = AtProtoAuthClient::new(config).unwrap();
        let (auth_url, csrf_token) = client.get_authorization_url().unwrap();
        
        assert!(auth_url.contains("bsky.social"));
        assert!(auth_url.contains("client_id=test-client-id"));
        assert!(!csrf_token.is_empty());
    }
}
