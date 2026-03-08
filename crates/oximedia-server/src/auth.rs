//! Authentication and authorization.

pub mod webhook;

pub use webhook::{WebhookConfig, WebhookEvent, WebhookEventType, WebhookNotifier};

use crate::{
    error::{ServerError, ServerResult},
    models::user::{Claims, UserRole},
};
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts},
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rand::RngCore;

/// Authentication manager.
#[derive(Clone)]
pub struct AuthManager {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
}

impl AuthManager {
    /// Creates a new authentication manager.
    #[must_use]
    pub fn new(secret: &str) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
        }
    }

    /// Hashes a password using Argon2.
    ///
    /// # Errors
    ///
    /// Returns an error if password hashing fails.
    pub fn hash_password(&self, password: &str) -> ServerResult<String> {
        let salt = SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| ServerError::PasswordHash(e.to_string()))?;
        Ok(password_hash.to_string())
    }

    /// Verifies a password against a hash.
    ///
    /// # Errors
    ///
    /// Returns an error if verification fails or password is invalid.
    pub fn verify_password(&self, password: &str, hash: &str) -> ServerResult<bool> {
        let parsed_hash =
            PasswordHash::new(hash).map_err(|e| ServerError::PasswordHash(e.to_string()))?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok())
    }

    /// Generates a JWT token for a user.
    ///
    /// # Errors
    ///
    /// Returns an error if token generation fails.
    pub fn generate_token(
        &self,
        user_id: String,
        username: String,
        role: UserRole,
        expiration: i64,
    ) -> ServerResult<String> {
        let claims = Claims::new(user_id, username, role, expiration);
        let token = encode(&Header::default(), &claims, &self.encoding_key)?;
        Ok(token)
    }

    /// Validates a JWT token and returns the claims.
    ///
    /// # Errors
    ///
    /// Returns an error if the token is invalid or expired.
    pub fn validate_token(&self, token: &str) -> ServerResult<Claims> {
        let token_data = decode::<Claims>(token, &self.decoding_key, &Validation::default())?;
        let claims = token_data.claims;

        if claims.is_expired() {
            return Err(ServerError::Unauthorized("Token expired".to_string()));
        }

        Ok(claims)
    }

    /// Generates a random API key.
    #[must_use]
    pub fn generate_api_key() -> String {
        let mut key = [0u8; 32];
        rand::rng().fill_bytes(&mut key);
        format!("oxm_{}", hex::encode(key))
    }

    /// Hashes an API key for storage.
    #[must_use]
    pub fn hash_api_key(key: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        hex::encode(hasher.finalize())
    }
}

/// Authenticated user extracted from request.
pub struct AuthUser {
    /// User ID
    pub user_id: String,
    /// Username
    pub username: String,
    /// User role
    pub role: UserRole,
}

impl AuthUser {
    /// Creates a new authenticated user.
    #[must_use]
    pub const fn new(user_id: String, username: String, role: UserRole) -> Self {
        Self {
            user_id,
            username,
            role,
        }
    }

    /// Checks if the user is an admin.
    #[must_use]
    pub const fn is_admin(&self) -> bool {
        self.role.is_admin()
    }

    /// Checks if the user can write data.
    #[must_use]
    pub const fn can_write(&self) -> bool {
        self.role.can_write()
    }
}

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = ServerError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Try to extract the Authorization header
        let _auth_header = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| ServerError::Unauthorized("Missing authorization header".to_string()))?;

        // The token validation should be done by middleware
        // For now, we'll just parse the claims from the extensions
        // In a real implementation, you would validate the token here
        let claims = parts
            .extensions
            .get::<Claims>()
            .ok_or_else(|| ServerError::Unauthorized("Invalid token".to_string()))?;

        Ok(Self {
            user_id: claims.sub.clone(),
            username: claims.username.clone(),
            role: claims.role,
        })
    }
}

/// Middleware for JWT authentication.
pub mod middleware {
    use super::*;
    use axum::{
        body::Body,
        extract::State,
        http::{Request, StatusCode},
        middleware::Next,
        response::Response,
    };
    use std::sync::Arc;

    /// JWT authentication middleware.
    pub async fn auth(
        State(auth): State<Arc<AuthManager>>,
        mut request: Request<Body>,
        next: Next,
    ) -> Result<Response, StatusCode> {
        // Extract the Authorization header
        let auth_header = request
            .headers()
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|header| header.to_str().ok())
            .ok_or(StatusCode::UNAUTHORIZED)?;

        // Extract the token
        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or(StatusCode::UNAUTHORIZED)?;

        // Validate the token
        let claims = auth
            .validate_token(token)
            .map_err(|_| StatusCode::UNAUTHORIZED)?;

        // Insert claims into request extensions
        request.extensions_mut().insert(claims);

        Ok(next.run(request).await)
    }

    /// Admin-only middleware.
    pub async fn require_admin(
        auth_user: AuthUser,
        request: Request<Body>,
        next: Next,
    ) -> Result<Response, StatusCode> {
        if !auth_user.is_admin() {
            return Err(StatusCode::FORBIDDEN);
        }

        Ok(next.run(request).await)
    }
}
