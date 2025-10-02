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
use crate::auth::store::{SqliteAuthStore, AuthFlowRecord};
use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use std::collections::HashMap;
use ring::{signature::{EcdsaKeyPair, ECDSA_P256_SHA256_FIXED_SIGNING, Signature, KeyPair}, rand::SystemRandom};

fn b64url(data: impl AsRef<[u8]>) -> String {
    URL_SAFE_NO_PAD.encode(data.as_ref())
}

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
    /// OAuth client ID (can be a dynamic client metadata URL)
    pub client_id: String,
    /// OAuth client secret (optional; ATProto public clients typically don't need this)
    pub client_secret: Option<String>,
    /// Redirect URI (callback URL)
    pub redirect_uri: String,
    /// Space-delimited scope string
    pub scope: String,
}

/// ATProto OAuth client implementing the full ATProto OAuth specification
pub struct AtProtoAuthClient {
    config: AuthConfig,
    http_client: reqwest::Client,
}

impl AtProtoAuthClient {
    /// Create a new ATProto OAuth client
    pub fn new(config: AuthConfig) -> Result<Self> {
        Ok(Self {
            config,
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .context("Failed to create HTTP client")?,
        })
    }

    /// Get redirect URI
    pub fn redirect_uri(&self) -> &str { &self.config.redirect_uri }

    /// Get client ID if set
    pub fn client_id(&self) -> &str { &self.config.client_id }
    /// Get scope
    pub fn scope(&self) -> &str { &self.config.scope }

    /// Get PDS URL saved for an authorization flow state (moved to store)
    pub fn get_flow_pds_url(&self, _state: &str) -> Option<String> { None }

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
    pub async fn discover_server_metadata(&self, origin: &str) -> Result<ServerMetadata> {
        let origin = origin.trim_end_matches('/');
        let metadata_url = format!("{}/.well-known/oauth-authorization-server", origin);
        
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

    /// Discover the user's PDS base URL from their DID document via plc.directory
    pub async fn discover_pds_from_did(&self, did: &str) -> Result<String> {
        // Only did:plc is supported in this helper
        let url = format!("https://plc.directory/{}", did);
        let resp = self.http_client.get(&url).send().await.context("Failed to fetch DID document")?;
        if !resp.status().is_success() {
            return Err(anyhow!("DID document fetch failed: {}", resp.status()));
        }
        let doc: serde_json::Value = resp.json().await.context("Failed to parse DID document")?;
        if let Some(services) = doc.get("service").and_then(|v| v.as_array()) {
            for svc in services {
                let id = svc.get("id").and_then(|v| v.as_str());
                let ty = svc.get("type").and_then(|v| v.as_str());
                if id == Some("#atproto_pds") || ty == Some("AtprotoPersonalDataServer") {
                    if let Some(endpoint) = svc.get("serviceEndpoint").and_then(|v| v.as_str()) {
                        return Ok(endpoint.trim_end_matches('/').to_string());
                    }
                }
            }
        }
        Err(anyhow!("PDS endpoint not found in DID document"))
    }

    /// Generate a DPoP proof JWT
    fn generate_dpop_proof_es256(&self, pkcs8: &[u8], jwk: &serde_json::Value, htm: &str, htu: &str, nonce: Option<&str>, ath: Option<&str>) -> Result<String> {
        // Prepare JWT header with ES256 and public JWK
        let mut header = serde_json::json!({
            "typ": "dpop+jwt",
            "alg": "ES256",
            "jwk": jwk
        });
        let header_b64 = b64url(serde_json::to_vec(&header)?);

        // Prepare JWT payload
        let iat = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs() as i64;
        let mut payload = serde_json::json!({
            "jti": uuid::Uuid::new_v4().to_string(),
            "htm": htm,
            "htu": htu,
            "iat": iat,
        });
        if let Some(n) = nonce { payload["nonce"] = serde_json::Value::String(n.to_string()); }
        if let Some(token) = ath {
            let digest = Sha256::digest(token.as_bytes());
            payload["ath"] = serde_json::Value::String(b64url(digest));
        }
        let payload_b64 = b64url(serde_json::to_vec(&payload)?);

        let signing_input = format!("{}.{}", header_b64, payload_b64);

        // Sign with P-256 over SHA-256, fixed-length signature
        let rng = SystemRandom::new();
        let key_pair = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, pkcs8, &rng)
            .map_err(|_| anyhow!("Invalid DPoP keypair"))?;
        let sig: Signature = key_pair.sign(&rng, signing_input.as_bytes())
            .map_err(|_| anyhow!("Failed to sign DPoP"))?;
        let sig_b64 = b64url(sig.as_ref());
        Ok(format!("{}.{}", signing_input, sig_b64))
    }

    fn generate_p256_keypair_jwk() -> Result<(Vec<u8>, serde_json::Value)> {
        let rng = SystemRandom::new();
        let pkcs8_bytes = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng)
            .map_err(|_| anyhow!("Failed to generate P-256 keypair"))?;

        // Derive public key coordinates for JWK
        let key_pair = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, pkcs8_bytes.as_ref(), &rng)
            .map_err(|_| anyhow!("Invalid generated keypair"))?;
        let pub_uncompressed = key_pair.public_key().as_ref();
        // SEC1 uncompressed: 0x04 || X(32) || Y(32)
        if pub_uncompressed.len() != 65 || pub_uncompressed[0] != 0x04 { return Err(anyhow!("Unexpected public key format")); }
        let x = &pub_uncompressed[1..33];
        let y = &pub_uncompressed[33..65];
        let jwk = serde_json::json!({
            "kty": "EC",
            "crv": "P-256",
            "x": b64url(x),
            "y": b64url(y),
        });
        Ok((pkcs8_bytes.as_ref().to_vec(), jwk))
    }

    /// Generate authorization URL with PKCE
    /// 
    /// Returns (authorization_url, state) where state is used for CSRF protection
    pub async fn get_authorization_url(&self, store: &SqliteAuthStore, handle: String) -> Result<(String, String)> {
        // 1. Resolve handle to discover PDS
        let pds_url = self.resolve_handle(handle.clone()).await?;
        
        // 2. Discover server metadata
        let metadata = self.discover_server_metadata(&pds_url).await?;

        // 3. Generate PKCE challenge
        let code_verifier = Self::generate_code_verifier();
        let code_challenge = Self::generate_code_challenge(&code_verifier);
        
        // 4. Generate state for CSRF protection
        let state = Self::generate_state();

        // Generate DPoP keypair for this flow
        let (pkcs8, jwk) = Self::generate_p256_keypair_jwk()?;

        // 5. Make Pushed Authorization Request (PAR)
        let par_endpoint = metadata
            .pushed_authorization_request_endpoint
            .ok_or_else(|| anyhow!("Authorization server does not advertise PAR endpoint"))?;

        // Build PAR body
        let mut body: HashMap<&str, String> = HashMap::new();
        body.insert("response_type", "code".to_string());
        if !self.config.client_id.is_empty() {
            body.insert("client_id", self.config.client_id.clone());
        }
        body.insert("redirect_uri", self.config.redirect_uri.clone());
        body.insert("state", state.clone());
        body.insert("code_challenge", code_challenge);
        body.insert("code_challenge_method", "S256".to_string());
        body.insert("scope", self.config.scope.clone());
        // Include login_hint to prefill account
        body.insert("login_hint", handle);

        // Generate DPoP proof for PAR
        let dpop = self.generate_dpop_proof_es256(&pkcs8, &jwk, "POST", &par_endpoint, None, None)?;
        let par_resp = self
            .http_client
            .post(&par_endpoint)
            .header("DPoP", dpop)
            .form(&body)
            .send()
            .await
            .context("Failed to send PAR request")?;

        // If nonce is required, retry once with DPoP-Nonce from the response headers
        let par_resp = if par_resp.status() == reqwest::StatusCode::UNAUTHORIZED
            || par_resp.status() == reqwest::StatusCode::BAD_REQUEST
        {
            let nonce_opt = par_resp.headers()
                .get("DPoP-Nonce")
                .or_else(|| par_resp.headers().get("dpop-nonce"))
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            if let Some(nonce) = nonce_opt {
                let dpop_retry = self.generate_dpop_proof_es256(&pkcs8, &jwk, "POST", &par_endpoint, Some(&nonce), None)?;
                let resp2 = self
                    .http_client
                    .post(&par_endpoint)
                    .header("DPoP", dpop_retry)
                    .form(&body)
                    .send()
                    .await
                    .context("Failed to send PAR retry request")?;
                resp2
            } else {
                par_resp
            }
        } else {
            par_resp
        };

        if !par_resp.status().is_success() {
            let err_text = par_resp.text().await.unwrap_or_default();
            return Err(anyhow!("PAR request failed: {}", err_text));
        }

        let dpop_nonce = par_resp
            .headers()
            .get("DPoP-Nonce")
            .or_else(|| par_resp.headers().get("dpop-nonce"))
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        #[derive(Deserialize)]
        struct ParResponse { request_uri: String }
        let par: ParResponse = par_resp.json().await.context("Failed to parse PAR response")?;

        // Persist flow state in SQLite
        store.insert(AuthFlowRecord {
            state: state.clone(),
            issuer: metadata.issuer.clone(),
            pds_url,
            code_verifier,
            dpop_pkcs8: pkcs8,
            dpop_jwk: serde_json::to_string(&jwk)?,
            dpop_nonce,
        }).await?;

        // Build authorization redirect URL with request_uri + client_id only
        let auth_url = format!(
            "{}?client_id={}&request_uri={}",
            metadata.authorization_endpoint,
            urlencoding::encode(&self.config.client_id),
            urlencoding::encode(&par.request_uri)
        );

        Ok((auth_url, state))
    }

    /// Exchange authorization code for access token with DPoP
    pub async fn exchange_code(&self, store: &SqliteAuthStore, code: String, state: String, issuer_origin: String) -> Result<(String, Option<String>, String, Option<String>)> {
        // 1. Discover Authorization Server metadata from issuer
        let metadata = self.discover_server_metadata(&issuer_origin).await?;

        // 2. Retrieve PKCE and DPoP material by state
        let rec = store.get(&state).await?.ok_or_else(|| anyhow!("No PKCE verifier found for state"))?;
        let code_verifier = rec.code_verifier.clone();

        // 3. Generate DPoP proof
        // Get flow DPoP key and nonce (if any)
        let pkcs8 = rec.dpop_pkcs8.clone();
        let jwk: serde_json::Value = serde_json::from_str(&rec.dpop_jwk)?;
        let mut known_nonce = rec.dpop_nonce.clone();

        let dpop_proof = self.generate_dpop_proof_es256(&pkcs8, &jwk, "POST", &metadata.token_endpoint, known_nonce.as_deref(), None)?;

        // 4. Exchange code for token (with nonce handling)
        let mut params = HashMap::new();
        params.insert("grant_type", "authorization_code");
        params.insert("code", &code);
        params.insert("redirect_uri", &self.config.redirect_uri);
        if !self.config.client_id.is_empty() {
            params.insert("client_id", &self.config.client_id);
        }
        if let Some(secret) = &self.config.client_secret {
            if !secret.is_empty() {
                params.insert("client_secret", secret);
            }
        }
        params.insert("code_verifier", &code_verifier);

        let mut resp = self
            .http_client
            .post(&metadata.token_endpoint)
            .header("DPoP", dpop_proof.clone())
            .form(&params)
            .send()
            .await
            .context("Failed to exchange authorization code")?;

        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::BAD_REQUEST {
            let nonce_opt = resp
                .headers()
                .get("DPoP-Nonce")
                .or_else(|| resp.headers().get("dpop-nonce"))
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            if let Some(nonce) = nonce_opt {
                // Retry with nonce-bound DPoP
                let retry_proof = self.generate_dpop_proof_es256(&pkcs8, &jwk, "POST", &metadata.token_endpoint, Some(&nonce), None)?;
                resp = self
                    .http_client
                    .post(&metadata.token_endpoint)
                    .header("DPoP", retry_proof)
                    .form(&params)
                    .send()
                    .await
                    .context("Failed to retry token exchange")?;
                // Save latest nonce
                let _ = store.update_nonce(&state, Some(&nonce)).await;
            }
        }

        if !resp.status().is_success() {
            let error_text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Token exchange failed: {}", error_text));
        }

        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: String,
            refresh_token: Option<String>,
            #[serde(default)]
            sub: Option<String>,
        }

        let token_response: TokenResponse = resp.json()
            .await
            .context("Failed to parse token response")?;

        Ok((token_response.access_token, token_response.refresh_token, metadata.issuer, token_response.sub))
    }

    /// Fetch user profile using access token with DPoP
    pub async fn get_user_profile(&self, access_token: &str, pds_url: &str, _state: Option<&str>) -> Result<User> {
        let session_url = format!("{}/xrpc/com.atproto.server.getSession", pds_url);
        // Use a fresh ephemeral key; note: may fail on DPoP-bound tokens.
        let (pkcs8, jwk) = Self::generate_p256_keypair_jwk()?;
        let mut dpop_proof = self.generate_dpop_proof_es256(&pkcs8, &jwk, "GET", &session_url, None, Some(access_token))?;

        let mut response = self.http_client
            .get(&session_url)
            .header("Authorization", format!("DPoP {}", access_token))
            .header("Accept", "application/json")
            .header("DPoP", dpop_proof)
            .send()
            .await
            .context("Failed to fetch user profile")?;

        // Retry on 400/401 with DPoP-Nonce, if provided
        if response.status() == reqwest::StatusCode::UNAUTHORIZED || response.status() == reqwest::StatusCode::BAD_REQUEST {
            if let Some(nonce) = response.headers().get("DPoP-Nonce").or_else(|| response.headers().get("dpop-nonce")).and_then(|v| v.to_str().ok()) {
                dpop_proof = self.generate_dpop_proof_es256(&pkcs8, &jwk, "GET", &session_url, Some(nonce), Some(access_token))?;
                response = self.http_client
                    .get(&session_url)
                    .header("Authorization", format!("DPoP {}", access_token))
                    .header("DPoP", dpop_proof)
                    .send()
                    .await
                    .context("Failed to fetch user profile (retry)")?;
            }
        }

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("Failed to fetch user profile: {} - {}", status, body));
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

    /// Fetch user profile using a provided DPoP keypair (must match the token-binding key)
    pub async fn get_user_profile_with_key(&self, access_token: &str, pds_url: &str, pkcs8: &[u8], jwk: &serde_json::Value) -> Result<User> {
        let session_url = format!("{}/xrpc/com.atproto.server.getSession", pds_url);

        // First attempt
        let mut dpop_proof = self.generate_dpop_proof_es256(pkcs8, jwk, "GET", &session_url, None, Some(access_token))?;
        let mut response = self.http_client
            .get(&session_url)
            .header("Authorization", format!("DPoP {}", access_token))
            .header("Accept", "application/json")
            .header("DPoP", dpop_proof)
            .send()
            .await
            .context("Failed to fetch user profile")?;

        // Retry with nonce if required
        if response.status() == reqwest::StatusCode::UNAUTHORIZED || response.status() == reqwest::StatusCode::BAD_REQUEST {
            if let Some(nonce) = response.headers().get("DPoP-Nonce").or_else(|| response.headers().get("dpop-nonce")).and_then(|v| v.to_str().ok()) {
                dpop_proof = self.generate_dpop_proof_es256(pkcs8, jwk, "GET", &session_url, Some(nonce), Some(access_token))?;
                response = self.http_client
                    .get(&session_url)
                    .header("Authorization", format!("DPoP {}", access_token))
                    .header("DPoP", dpop_proof)
                    .send()
                    .await
                    .context("Failed to fetch user profile (retry)")?;
            }
        }

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("Failed to fetch user profile: {} - {}", status, body));
        }

        #[derive(Deserialize)]
        struct AtProtoSession { did: String, handle: String }
        let profile: AtProtoSession = response.json().await.context("Failed to parse user profile")?;
        Ok(User { did: profile.did, handle: profile.handle, display_name: None, avatar: None })
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
            client_secret: Some("test-secret".to_string()),
            redirect_uri: "http://localhost:8000/callback".to_string(),
            scope: "atproto".to_string(),
        };
        
        let client = AtProtoAuthClient::new(config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_dpop_proof_contains_jwk_and_nonce() {
        let (pkcs8, jwk) = AtProtoAuthClient::generate_p256_keypair_jwk().unwrap();
        let client = AtProtoAuthClient::new(AuthConfig { client_id: "".into(), client_secret: None, redirect_uri: "http://localhost".into() }).unwrap();
        let jwt = client.generate_dpop_proof_es256(&pkcs8, &jwk, "POST", "https://example.com/token", Some("NONCE"), Some("token")).unwrap();
        let parts: Vec<&str> = jwt.split('.').collect();
        assert_eq!(parts.len(), 3);
        let header_json = String::from_utf8(base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(parts[0]).unwrap()).unwrap();
        let payload_json = String::from_utf8(base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(parts[1]).unwrap()).unwrap();
        assert!(header_json.contains("\"ES256\""));
        assert!(header_json.contains("\"jwk\""));
        assert!(payload_json.contains("\"htu\""));
        assert!(payload_json.contains("NONCE"));
        assert!(payload_json.contains("\"ath\""));
    }
}
