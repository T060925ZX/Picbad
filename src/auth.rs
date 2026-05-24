use crate::app::AppState;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use async_trait::async_trait;
use axum::{
    extract::{FromRequestParts, Path, State},
    http::{request::Parts, StatusCode},
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use chrono::Utc;
use password_hash::SaltString;
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::{collections::HashSet, sync::Arc};
use tokio::fs;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub role: String,
    pub api_key: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: User,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub role: Option<String>,
}

#[derive(Debug, Serialize)]
struct ApiError {
    error: String,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/login", post(login))
        .route("/api/register", post(register))
        .route("/api/me", get(me))
        .route("/api/users", get(list_users).post(create_user))
        .route("/api/users/:id", delete(delete_user))
        .route("/api/users/:id/key", post(rotate_user_key))
}

async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> impl IntoResponse {
    let row = match sqlx::query(
        "SELECT id, username, role, password_hash, token, api_key FROM users WHERE username = ?",
    )
    .bind(req.username.trim())
    .fetch_optional(&state.pool)
    .await
    {
        Ok(Some(row)) => row,
        Ok(None) => return unauthorized("invalid username or password"),
        Err(_) => return server_error("login failed"),
    };

    let hash: String = row.get("password_hash");
    if !verify_password(&req.password, &hash) {
        return unauthorized("invalid username or password");
    }

    let token: String = row.get("token");
    let _ = sqlx::query("UPDATE users SET last_login_at = ? WHERE id = ?")
        .bind(Utc::now().to_rfc3339())
        .bind(row.get::<String, _>("id"))
        .execute(&state.pool)
        .await;

    (
        StatusCode::OK,
        Json(LoginResponse {
            token,
            user: User {
                id: row.get("id"),
                username: row.get("username"),
                role: row.get("role"),
                api_key: row.get("api_key"),
            },
        }),
    )
        .into_response()
}

async fn register(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateUserRequest>,
) -> impl IntoResponse {
    let username = req.username.trim();
    if username.len() < 3 || req.password.len() < 8 {
        return bad_request("username must be 3+ chars and password 8+ chars");
    }

    let user_count: i64 = match sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&state.pool)
        .await
    {
        Ok(count) => count,
        Err(_) => return server_error("failed to check users"),
    };
    let role = if user_count == 0 { "admin" } else { "user" };
    let id = Uuid::new_v4().to_string();
    let token = Uuid::new_v4().to_string();
    let api_key = new_api_key();
    let hash = match hash_password(&req.password) {
        Ok(hash) => hash,
        Err(_) => return server_error("failed to hash password"),
    };

    match sqlx::query(
        "INSERT INTO users (id, username, password_hash, role, token, api_key, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(username)
    .bind(hash)
    .bind(role)
    .bind(&token)
    .bind(&api_key)
    .bind(Utc::now().to_rfc3339())
    .execute(&state.pool)
    .await
    {
        Ok(_) => (
            StatusCode::CREATED,
            Json(serde_json::json!({
                "token": token,
                "user": {
                    "id": id,
                    "username": username,
                    "role": role,
                    "api_key": api_key
                }
            })),
        )
            .into_response(),
        Err(_) => bad_request("username already exists"),
    }
}

async fn me(AuthUser(user): AuthUser) -> impl IntoResponse {
    Json(user)
}

async fn list_users(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> impl IntoResponse {
    if user.role != "admin" {
        return forbidden("admin required");
    }
    match sqlx::query(
        "SELECT id, username, role, api_key, created_at, last_login_at FROM users ORDER BY created_at DESC",
    )
    .fetch_all(&state.pool)
    .await
    {
        Ok(rows) => {
            let users: Vec<_> = rows
                .into_iter()
                .map(|row| {
                    serde_json::json!({
                        "id": row.get::<String, _>("id"),
                        "username": row.get::<String, _>("username"),
                        "role": row.get::<String, _>("role"),
                        "api_key": row.get::<String, _>("api_key"),
                        "created_at": row.get::<String, _>("created_at"),
                        "last_login_at": row.try_get::<String, _>("last_login_at").ok()
                    })
                })
                .collect();
            Json(users).into_response()
        }
        Err(_) => server_error("failed to list users"),
    }
}

async fn create_user(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Json(req): Json<CreateUserRequest>,
) -> impl IntoResponse {
    if user.role != "admin" {
        return forbidden("admin required");
    }
    let username = req.username.trim();
    if username.len() < 3 || req.password.len() < 8 {
        return bad_request("username must be 3+ chars and password 8+ chars");
    }
    let role = req.role.unwrap_or_else(|| "user".to_string());
    if role != "admin" && role != "user" {
        return bad_request("role must be admin or user");
    }
    let id = Uuid::new_v4().to_string();
    let token = Uuid::new_v4().to_string();
    let api_key = new_api_key();
    let hash = match hash_password(&req.password) {
        Ok(hash) => hash,
        Err(_) => return server_error("failed to hash password"),
    };
    match sqlx::query(
        "INSERT INTO users (id, username, password_hash, role, token, api_key, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(username)
    .bind(hash)
    .bind(role)
    .bind(&token)
    .bind(&api_key)
    .bind(Utc::now().to_rfc3339())
    .execute(&state.pool)
    .await
    {
        Ok(_) => (StatusCode::CREATED, Json(serde_json::json!({"id": id, "token": token, "api_key": api_key}))).into_response(),
        Err(_) => bad_request("username already exists"),
    }
}

async fn rotate_user_key(
    State(state): State<Arc<AppState>>,
    AuthUser(actor): AuthUser,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if actor.role != "admin" && actor.id != id {
        return forbidden("cannot rotate another user's key");
    }
    let api_key = new_api_key();
    match sqlx::query("UPDATE users SET api_key = ? WHERE id = ?")
        .bind(&api_key)
        .bind(&id)
        .execute(&state.pool)
        .await
    {
        Ok(result) if result.rows_affected() == 1 => {
            Json(serde_json::json!({"id": id, "api_key": api_key})).into_response()
        }
        Ok(_) => bad_request("user not found"),
        Err(_) => server_error("failed to rotate api key"),
    }
}

async fn delete_user(
    State(state): State<Arc<AppState>>,
    AuthUser(actor): AuthUser,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if actor.role != "admin" {
        return forbidden("admin required");
    }
    if actor.id == id {
        return bad_request("cannot delete your own account");
    }

    let target = match sqlx::query("SELECT id, username, role FROM users WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.pool)
        .await
    {
        Ok(Some(row)) => row,
        Ok(None) => return bad_request("user not found"),
        Err(_) => return server_error("failed to load user"),
    };
    let role: String = target.get("role");
    if role == "admin" {
        return forbidden("cannot delete another admin account");
    }

    let media_rows = match sqlx::query("SELECT sha256, ext FROM images WHERE owner_id = ?")
        .bind(&id)
        .fetch_all(&state.pool)
        .await
    {
        Ok(rows) => rows,
        Err(_) => return server_error("failed to load user media"),
    };

    let mut media = Vec::new();
    for row in media_rows {
        media.push((row.get::<String, _>("sha256"), row.get::<String, _>("ext")));
    }

    let _ = sqlx::query("DELETE FROM images WHERE owner_id = ?")
        .bind(&id)
        .execute(&state.pool)
        .await;

    let deleted = match sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(&id)
        .execute(&state.pool)
        .await
    {
        Ok(result) => result.rows_affected(),
        Err(_) => return server_error("failed to delete user"),
    };
    if deleted != 1 {
        return bad_request("user not found");
    }

    let mut checked = HashSet::new();
    for (sha256, ext) in media {
        if !checked.insert(sha256.clone()) {
            continue;
        }
        let still_used: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM images WHERE sha256 = ?")
            .bind(&sha256)
            .fetch_one(&state.pool)
            .await
            .unwrap_or(1);
        if still_used == 0 {
            let _ =
                fs::remove_file(state.config.originals_dir().join(format!("{sha256}.{ext}"))).await;
        }
    }
    let _ = state.cache.clear().await;

    Json(serde_json::json!({
        "ok": true,
        "id": id,
        "username": target.get::<String, _>("username")
    }))
    .into_response()
}

#[derive(Clone, Debug)]
pub struct AuthUser(pub User);

#[async_trait]
impl FromRequestParts<Arc<AppState>> for AuthUser {
    type Rejection = axum::response::Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .ok_or_else(|| unauthorized("missing bearer token"))?;

        let row = sqlx::query("SELECT id, username, role, api_key FROM users WHERE token = ?")
            .bind(token)
            .fetch_optional(&state.pool)
            .await
            .map_err(|_| server_error("auth failed"))?
            .ok_or_else(|| unauthorized("invalid token"))?;

        Ok(AuthUser(User {
            id: row.get("id"),
            username: row.get("username"),
            role: row.get("role"),
            api_key: row.get("api_key"),
        }))
    }
}

#[derive(Clone, Debug)]
pub struct ApiKeyUser(pub User);

#[async_trait]
impl FromRequestParts<Arc<AppState>> for ApiKeyUser {
    type Rejection = axum::response::Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let key = parts
            .headers
            .get("x-api-key")
            .or_else(|| parts.headers.get("x-picbad-key"))
            .and_then(|h| h.to_str().ok())
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .ok_or_else(|| unauthorized("missing upload api key"))?;

        let row = sqlx::query("SELECT id, username, role, api_key FROM users WHERE api_key = ?")
            .bind(key)
            .fetch_optional(&state.pool)
            .await
            .map_err(|_| server_error("api key auth failed"))?
            .ok_or_else(|| unauthorized("invalid upload api key"))?;

        Ok(ApiKeyUser(User {
            id: row.get("id"),
            username: row.get("username"),
            role: row.get("role"),
            api_key: row.get("api_key"),
        }))
    }
}

fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|err| anyhow::anyhow!("password hash failed: {err}"))?
        .to_string())
}

fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .ok()
        .and_then(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .ok()
        })
        .is_some()
}

fn new_api_key() -> String {
    format!("pk_{}", Uuid::new_v4().simple())
}

fn api_error(status: StatusCode, msg: &str) -> axum::response::Response {
    (
        status,
        Json(ApiError {
            error: msg.to_string(),
        }),
    )
        .into_response()
}

fn bad_request(msg: &str) -> axum::response::Response {
    api_error(StatusCode::BAD_REQUEST, msg)
}

fn unauthorized(msg: &str) -> axum::response::Response {
    api_error(StatusCode::UNAUTHORIZED, msg)
}

fn forbidden(msg: &str) -> axum::response::Response {
    api_error(StatusCode::FORBIDDEN, msg)
}

fn server_error(msg: &str) -> axum::response::Response {
    api_error(StatusCode::INTERNAL_SERVER_ERROR, msg)
}
