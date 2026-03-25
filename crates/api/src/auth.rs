//! JWT authentication: token creation, validation, and Axum middleware.

use std::sync::Arc;

use axum::extract::{FromRequestParts, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::handlers::ErrorResponse;
use crate::routes::AppState;

/// Google token info response from Google's tokeninfo endpoint.
#[derive(Debug, Deserialize)]
struct GoogleTokenInfo {
    /// Google user ID
    sub: String,
    /// User's email
    email: Option<String>,
    /// Whether email is verified
    email_verified: Option<String>,
    /// User's display name
    name: Option<String>,
    /// User's profile picture URL
    picture: Option<String>,
    /// The audience (should match our client ID)
    aud: Option<String>,
}

/// JWT claims stored in each token.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    /// User ID
    pub sub: i64,
    /// Username
    pub username: String,
    /// Expiration (Unix timestamp)
    pub exp: usize,
    /// Issued at (Unix timestamp)
    pub iat: usize,
}

/// Shared JWT configuration.
#[derive(Debug, Clone)]
pub struct JwtConfig {
    pub secret: Arc<String>,
    pub expiry_hours: u64,
    /// Google OAuth client ID for verifying ID tokens.
    pub google_client_id: Option<String>,
}

impl JwtConfig {
    pub fn new(secret: String, expiry_hours: u64) -> Self {
        Self {
            secret: Arc::new(secret),
            expiry_hours,
            google_client_id: None,
        }
    }

    /// Create with Google OAuth support.
    pub fn with_google(mut self, client_id: Option<String>) -> Self {
        self.google_client_id = client_id;
        self
    }

    /// Create a signed JWT for the given user.
    pub fn create_token(&self, user_id: i64, username: &str) -> Result<String, jsonwebtoken::errors::Error> {
        let now = Utc::now();
        let exp = now + chrono::Duration::hours(self.expiry_hours as i64);

        let claims = Claims {
            sub: user_id,
            username: username.to_string(),
            exp: exp.timestamp() as usize,
            iat: now.timestamp() as usize,
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.secret.as_bytes()),
        )
    }

    /// Validate a JWT and return its claims.
    pub fn validate_token(&self, token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
        let data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.secret.as_bytes()),
            &Validation::default(),
        )?;
        Ok(data.claims)
    }
}

/// Hash a password using Argon2.
pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    use argon2::password_hash::rand_core::OsRng;
    use argon2::password_hash::SaltString;
    use argon2::{Argon2, PasswordHasher};

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2.hash_password(password.as_bytes(), &salt)?;
    Ok(hash.to_string())
}

/// Verify a password against an Argon2 hash.
pub fn verify_password(password: &str, hash: &str) -> Result<bool, argon2::password_hash::Error> {
    use argon2::{Argon2, PasswordHash, PasswordVerifier};

    let parsed_hash = PasswordHash::new(hash)?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

/// Axum extractor that validates the JWT from the `Authorization: Bearer <token>` header.
/// Use this in handler signatures to require authentication.
pub struct AuthUser(pub Claims);

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = (StatusCode, Json<ErrorResponse>);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let jwt_config = state.jwt_config.as_ref().ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "auth not configured".into(),
                }),
            )
        })?;

        let headers = &parts.headers;
        let token = extract_bearer_token(headers).ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: "missing or invalid Authorization header".into(),
                }),
            )
        })?;

        let claims = jwt_config.validate_token(&token).map_err(|_| {
            (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: "invalid or expired token".into(),
                }),
            )
        })?;

        Ok(AuthUser(claims))
    }
}

/// Optional auth extractor — resolves to `None` if no token is present (instead of 401).
/// Use this for endpoints that are public but behave differently when authenticated.
pub struct OptionalAuthUser(pub Option<Claims>);

impl FromRequestParts<AppState> for OptionalAuthUser {
    type Rejection = (StatusCode, Json<ErrorResponse>);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let Some(jwt_config) = state.jwt_config.as_ref() else {
            return Ok(OptionalAuthUser(None));
        };

        let Some(token) = extract_bearer_token(&parts.headers) else {
            return Ok(OptionalAuthUser(None));
        };

        match jwt_config.validate_token(&token) {
            Ok(claims) => Ok(OptionalAuthUser(Some(claims))),
            Err(_) => Ok(OptionalAuthUser(None)),
        }
    }
}

fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    let value = headers.get("authorization")?.to_str().ok()?;
    let token = value.strip_prefix("Bearer ")?;
    Some(token.to_string())
}

// ── Auth handlers ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserInfo,
    /// True when this is a brand-new Google user who hasn't picked a username yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_new: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub id: i64,
    pub username: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
}

/// `POST /auth/register` — Create a new user account.
pub async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> impl IntoResponse {
    let jwt_config = match &state.jwt_config {
        Some(c) => c,
        None => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "auth not configured".into(),
                }),
            ))
        }
    };

    // Validate input
    let username = body.username.trim();
    if username.len() < 3 || username.len() > 32 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "username must be 3-32 characters".into(),
            }),
        ));
    }
    if body.password.len() < 6 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "password must be at least 6 characters".into(),
            }),
        ));
    }

    // Check if username taken
    if let Ok(Some(_)) = state.pg_store.get_user_by_username(username).await {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: "username already taken".into(),
            }),
        ));
    }

    // Hash password
    let password_hash = hash_password(&body.password).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "failed to hash password".into(),
            }),
        )
    })?;

    // Create user
    let user = state
        .pg_store
        .create_user(username, &password_hash)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    // Generate token
    let token = jwt_config.create_token(user.id, &user.username).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "failed to create token".into(),
            }),
        )
    })?;

    Ok((
        StatusCode::CREATED,
        Json(AuthResponse {
            token,
            user: UserInfo {
                id: user.id,
                username: user.username,
                email: user.email,
                avatar_url: user.avatar_url,
            },
            is_new: None,
        }),
    ))
}

/// `POST /auth/login` — Authenticate and receive a JWT.
pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> impl IntoResponse {
    let jwt_config = match &state.jwt_config {
        Some(c) => c,
        None => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "auth not configured".into(),
                }),
            ))
        }
    };

    // Find user
    let user = state
        .pg_store
        .get_user_by_username(body.username.trim())
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    let user = match user {
        Some(u) => u,
        None => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: "invalid username or password".into(),
                }),
            ))
        }
    };

    // Verify password (Google-only users have no password_hash)
    let password_hash = match &user.password_hash {
        Some(h) => h,
        None => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: "this account uses Google sign-in".into(),
                }),
            ))
        }
    };
    let valid = verify_password(&body.password, password_hash).unwrap_or(false);
    if !valid {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "invalid username or password".into(),
            }),
        ));
    }

    // Generate token
    let token = jwt_config.create_token(user.id, &user.username).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "failed to create token".into(),
            }),
        )
    })?;

    Ok(Json(AuthResponse {
        token,
        user: UserInfo {
            id: user.id,
            username: user.username,
            email: user.email,
            avatar_url: user.avatar_url,
        },
        is_new: None,
    }))
}

/// `GET /auth/me` — Get the current user (requires auth).
pub async fn me(
    State(state): State<AppState>,
    auth: AuthUser,
) -> impl IntoResponse {
    // Fetch full user record to include email/avatar
    let user = state.pg_store.get_user_by_id(auth.0.sub).await;
    match user {
        Ok(Some(u)) => Ok(Json(UserInfo {
            id: u.id,
            username: u.username,
            email: u.email,
            avatar_url: u.avatar_url,
        })),
        _ => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "user not found".into(),
            }),
        )),
    }
}

/// `GET /auth/google-client-id` — Returns the Google client ID (public, no auth).
pub async fn google_client_id(State(state): State<AppState>) -> impl IntoResponse {
    let client_id = state
        .jwt_config
        .as_ref()
        .and_then(|c| c.google_client_id.clone());

    #[derive(Serialize)]
    struct Resp {
        client_id: Option<String>,
    }

    Json(Resp { client_id })
}

/// Google OAuth request body.
#[derive(Debug, Deserialize)]
pub struct GoogleAuthRequest {
    /// The ID token from Google Identity Services
    pub credential: String,
}

/// `POST /auth/google` — Authenticate via Google ID token.
pub async fn google_login(
    State(state): State<AppState>,
    Json(body): Json<GoogleAuthRequest>,
) -> impl IntoResponse {
    let jwt_config = match &state.jwt_config {
        Some(c) => c,
        None => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "auth not configured".into(),
                }),
            ))
        }
    };

    let google_client_id = match &jwt_config.google_client_id {
        Some(id) => id.clone(),
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Google OAuth not configured".into(),
                }),
            ))
        }
    };

    // Verify the Google ID token via Google's tokeninfo endpoint
    let client = reqwest::Client::new();
    let token_info = client
        .get("https://oauth2.googleapis.com/tokeninfo")
        .query(&[("id_token", &body.credential)])
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Google tokeninfo request failed: {e}");
            (
                StatusCode::BAD_GATEWAY,
                Json(ErrorResponse {
                    error: "failed to verify Google token".into(),
                }),
            )
        })?;

    if !token_info.status().is_success() {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "invalid Google token".into(),
            }),
        ));
    }

    let info: GoogleTokenInfo = token_info.json().await.map_err(|_| {
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: "failed to parse Google token response".into(),
            }),
        )
    })?;

    // Verify the audience matches our client ID
    if info.aud.as_deref() != Some(&google_client_id) {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Google token audience mismatch".into(),
            }),
        ));
    }

    // Verify email is verified
    if info.email_verified.as_deref() != Some("true") {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Google email not verified".into(),
            }),
        ));
    }

    let email = info.email.unwrap_or_default();
    let username = info.name.unwrap_or_else(|| email.split('@').next().unwrap_or("user").to_string());

    // Upsert user by Google ID
    let (user, is_new) = state
        .pg_store
        .upsert_google_user(&info.sub, &email, &username, info.picture.as_deref())
        .await
        .map_err(|e| {
            tracing::error!("Google user upsert failed: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "failed to create user".into(),
                }),
            )
        })?;

    // Generate JWT
    let token = jwt_config.create_token(user.id, &user.username).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "failed to create token".into(),
            }),
        )
    })?;

    Ok(Json(AuthResponse {
        token,
        user: UserInfo {
            id: user.id,
            username: user.username,
            email: user.email,
            avatar_url: user.avatar_url,
        },
        is_new: Some(is_new),
    }))
}

/// Request body for updating username.
#[derive(Debug, Deserialize)]
pub struct UpdateUsernameRequest {
    pub username: String,
}

/// `PUT /auth/username` — Set/update username (requires auth).
pub async fn update_username(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<UpdateUsernameRequest>,
) -> impl IntoResponse {
    let username = body.username.trim();
    if username.len() < 3 || username.len() > 32 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "username must be 3-32 characters".into(),
            }),
        ));
    }

    // Check if username is taken by someone else
    if let Ok(Some(existing)) = state.pg_store.get_user_by_username(username).await {
        if existing.id != auth.0.sub {
            return Err((
                StatusCode::CONFLICT,
                Json(ErrorResponse {
                    error: "username already taken".into(),
                }),
            ));
        }
    }

    let user = state
        .pg_store
        .update_username(auth.0.sub, username)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    Ok(Json(UserInfo {
        id: user.id,
        username: user.username,
        email: user.email,
        avatar_url: user.avatar_url,
    }))
}
