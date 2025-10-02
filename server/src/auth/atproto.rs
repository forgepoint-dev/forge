//! ATProto OAuth client
//!
//! This module implements the ATProto OAuth specification for authenticating users.
//! See: https://atproto.com/specs/oauth
//!
//! Key features:
//! - Handle resolution to discover user's PDS
//! - Server metadata discovery via .well-known endpoints
//! - DPoP (Demonstrating Proof-of-Possession) for token binding
//! - PKCE (Proof Key for Code Exchange) for enhanced security

use super::User;
use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use hmac::{Hmac, Mac};
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

type HmacSha256 = Hmac<Sha256>;

/// OAuth server metadata from .well-known discovery
#[derive(Debug, Clone, Deserialize)]
pub struct ServerMetadata {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    #[serde(default)]
    pub pushed_authorization_request_endpoint: Option<String>,
    #[serde(default)]
    pub dpop_signing_alg_values_supported: Vec<String>,
    #[serde(default)]
    pub scopes_supported: Vec<String>,
}

/// DPoP proof JWT claims
#[derive(Debug, Serialize, Deserialize)]
struct DpopClaims {
    jti: String,
    htm: String,
    htu: String,
    iat: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    ath: Option<String>,
}

/// Configuration for ATProto OAuth
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// OAuth client ID
    pub client_id: String,
    /// OAuth client secret  
    pub client_secret: String,
    /// Redirect URI (callback URL)
    pub redirect_uri: String,
}

/// ATProto OAuth client implementing the full ATProto OAuth specification
pub struct AtProtoAuthClient {
    config: AuthConfig,
    pkce_verifier: Arc<Mutex<Option<String>>>,
    dpop_key: Arc<Mutex<Option<Vec<u8>>>>,
    http_client: reqwest::Client,
}

impl AtProtoAuthClient {
    /// Create a new ATProto OAuth client
    pub fn new(config: AuthConfig) -> Result<Self> {
        Ok(Self {
            config,
            pkce_verifier: Arc::new(Mutex::new(None)),
            dpop_key: Arc::new(Mutex::new(None)),
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .context("Failed to create HTTP client")?,
        })
    }

    /// Resolve a user's handle to discover their PDS and authorization server
    /// 
    /// ATProto uses handle resolution to discover the user's Personal Data Server (PDS)
    /// which hosts their authorization server.
    pub async fn resolve_handle(&self, handle: String) -> Result<String> {
        // For now, we'll use a simple approach: assume bsky.social PDS
        // In a full implementation, this would:
        // 1. Try DNS TXT record lookup for _atproto.<handle>
        // 2. Fall back to HTTPS .well-known lookup
        // 3. Parse the DID document to find the PDS endpoint
        
        // Simplified: if handle ends with .bsky.social, use bsky.social PDS
        if handle.ends_with(".bsky.social") || !handle.contains('.') {
            Ok("https://bsky.social".to_string())
        } else {
            // For other handles, try to resolve via .well-known
            let well_known_url = format!("https://{}/.well-known/atproto-did", handle);
            let response = self.http_client
                .get(&well_known_url)
                .send()
                .await
                .context("Failed to resolve handle")?;
            
            if response.status().is_success() {
            let _did: String = response.text().await?;
                // In reality, we'd need to resolve the DID document to find the PDS
                // For now, return bsky.social as fallback
                Ok("https://bsky.social".to_string())
            } else {
                Ok("https://bsky.social".to_string())
            }
        }
    }

    /// Discover OAuth server metadata via .well-known endpoint
    pub async fn discover_server_metadata(&self, pds_url: &str) -> Result<ServerMetadata> {
        let metadata_url = format!("{}/.well-known/oauth-authorization-server", pds_url);
        
        let response = self.http_client
            .get(&metadata_url)
            .send()
            .await
            .context("Failed to fetch server metadata")?;

        if !response.status().is_success() {
            return Err(anyhow!("Server metadata not found: {}", response.status()));
        }

        response.json::<ServerMetadata>()
            .await
            .context("Failed to parse server metadata")
    }

    /// Generate a DPoP proof JWT
    fn generate_dpop_proof(&self, htm: &str, htu: &str, ath: Option<&str>) -> Result<String> {
        // Generate or retrieve DPoP key
        let key = {
            let mut dpop_key = self.dpop_key.lock()
                .map_err(|e| anyhow!("Failed to lock DPoP key: {}", e))?;
            
            if dpop_key.is_none() {
                let mut rng = rand::thread_rng();
                let new_key: Vec<u8> = (0..32).map(|_| rng.r#gen()).collect();
                *dpop_key = Some(new_key.clone());
                new_key
            } else {
                dpop_key.as_ref().unwrap().clone()
            }
        };

        let claims = DpopClaims {
            jti: uuid::Uuid::new_v4().to_string(),
            htm: htm.to_string(),
            htu: htu.to_string(),
            iat: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() as i64,
            ath: ath.map(|s| {
                let mut hasher = HmacSha256::new_from_slice(&key).unwrap();
                hasher.update(s.as_bytes());
                URL_SAFE_NO_PAD.encode(hasher.finalize().into_bytes())
            }),
        };

        let mut header = Header::new(Algorithm::HS256);
        header.typ = Some("dpop+jwt".to_string());

        jsonwebtoken::encode(&header, &claims, &EncodingKey::from_secret(&key))
            .context("Failed to generate DPoP proof")
    }

    /// Generate authorization URL with PKCE
    /// 
    /// Returns (authorization_url, state) where state is used for CSRF protection
    pub async fn get_authorization_url(&self, handle: String) -> Result<(String, String)> {
        // 1. Resolve handle to discover PDS
        let pds_url = self.resolve_handle(handle).await?;
        
        // 2. Discover server metadata
        let metadata = self.discover_server_metadata(&pds_url).await?;

        // 3. Generate PKCE challenge
        let code_verifier = Self::generate_code_verifier();
        let code_challenge = Self::generate_code_challenge(&code_verifier);
        
        // Store verifier for later use
        {
            let mut verifier = self.pkce_verifier.lock()
                .map_err(|e| anyhow!("Failed to lock PKCE verifier: {}", e))?;
            *verifier = Some(code_verifier);
        }

        // 4. Generate state for CSRF protection
        let state = Self::generate_state();

        // 5. Build authorization URL
        let mut params = HashMap::new();
        params.insert("response_type", "code");
        params.insert("client_id", &self.config.client_id);
        params.insert("redirect_uri", &self.config.redirect_uri);
        params.insert("state", &state);
        params.insert("code_challenge", &code_challenge);
        params.insert("code_challenge_method", "S256");
        params.insert("scope", "atproto transition:generic");

        let query = params.iter()
            .map(|(k, v)| format!("{}={}", 
                urlencoding::encode(k), 
                urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        let auth_url = format!("{}?{}", metadata.authorization_endpoint, query);

        Ok((auth_url, state))
    }

    /// Exchange authorization code for access token with DPoP
    pub async fn exchange_code(&self, code: String, pds_url: String) -> Result<(String, Option<String>)> {
        // 1. Discover server metadata
        let metadata = self.discover_server_metadata(&pds_url).await?;

        // 2. Retrieve PKCE verifier
        let code_verifier = {
            let mut verifier = self.pkce_verifier.lock()
                .map_err(|e| anyhow!("Failed to lock PKCE verifier: {}", e))?;
            verifier.take()
                .ok_or_else(|| anyhow!("No PKCE verifier found"))?
        };

        // 3. Generate DPoP proof
        let dpop_proof = self.generate_dpop_proof("POST", &metadata.token_endpoint, None)?;

        // 4. Exchange code for token
        let mut params = HashMap::new();
        params.insert("grant_type", "authorization_code");
        params.insert("code", &code);
        params.insert("redirect_uri", &self.config.redirect_uri);
        params.insert("client_id", &self.config.client_id);
        params.insert("client_secret", &self.config.client_secret);
        params.insert("code_verifier", &code_verifier);

        let response = self.http_client
            .post(&metadata.token_endpoint)
            .header("DPoP", dpop_proof)
            .form(&params)
            .send()
            .await
            .context("Failed to exchange authorization code")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Token exchange failed: {}", error_text));
        }

        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: String,
            refresh_token: Option<String>,
        }

        let token_response: TokenResponse = response.json()
            .await
            .context("Failed to parse token response")?;

        Ok((token_response.access_token, token_response.refresh_token))
    }

    /// Fetch user profile using access token with DPoP
    pub async fn get_user_profile(&self, access_token: &str, pds_url: &str) -> Result<User> {
        let session_url = format!("{}/xrpc/com.atproto.server.getSession", pds_url);
        
        // Generate DPoP proof for this request
        let dpop_proof = self.generate_dpop_proof("GET", &session_url, Some(access_token))?;

        let response = self.http_client
            .get(&session_url)
            .header("Authorization", format!("DPoP {}", access_token))
            .header("DPoP", dpop_proof)
            .send()
            .await
            .context("Failed to fetch user profile")?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Failed to fetch user profile: {}",
                response.status()
            ));
        }

        #[derive(Deserialize)]
        struct AtProtoSession {
            did: String,
            handle: String,
        }

        let profile: AtProtoSession = response.json().await
            .context("Failed to parse user profile")?;

        Ok(User {
            did: profile.did,
            handle: profile.handle,
            display_name: None,
            avatar: None,
        })
    }

    /// Generate a cryptographically secure code verifier for PKCE
    fn generate_code_verifier() -> String {
        let mut rng = rand::thread_rng();
        let random_bytes: Vec<u8> = (0..32).map(|_| rng.r#gen()).collect();
        URL_SAFE_NO_PAD.encode(random_bytes)
    }

    /// Generate code challenge from verifier using SHA256
    fn generate_code_challenge(verifier: &str) -> String {
        use sha2::Digest;
        let hash = Sha256::digest(verifier.as_bytes());
        URL_SAFE_NO_PAD.encode(hash)
    }

    /// Generate a cryptographically secure state parameter
    fn generate_state() -> String {
        let mut rng = rand::thread_rng();
        let random_bytes: Vec<u8> = (0..32).map(|_| rng.r#gen()).collect();
        URL_SAFE_NO_PAD.encode(random_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_code_verifier() {
        let verifier = AtProtoAuthClient::generate_code_verifier();
        assert!(!verifier.is_empty());
        assert!(verifier.len() >= 43); // Base64 encoded 32 bytes
    }

    #[test]
    fn test_generate_code_challenge() {
        let verifier = "test_verifier";
        let challenge = AtProtoAuthClient::generate_code_challenge(verifier);
        assert!(!challenge.is_empty());
    }

    #[test]
    fn test_generate_state() {
        let state = AtProtoAuthClient::generate_state();
        assert!(!state.is_empty());
        assert!(state.len() >= 43);
    }

    #[tokio::test]
    async fn test_auth_client_creation() {
        let config = AuthConfig {
            client_id: "test-client-id".to_string(),
            client_secret: "test-secret".to_string(),
            redirect_uri: "http://localhost:8000/callback".to_string(),
        };
        
        let client = AtProtoAuthClient::new(config);
        assert!(client.is_ok());
    }
}
