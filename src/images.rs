use crate::{
    app::AppState,
    auth::{ApiKeyUser, AuthUser},
    cache,
};
use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{delete, get},
    Json, Router,
};
use bytes::Bytes;
use chrono::Utc;
use image::{codecs::jpeg::JpegEncoder, DynamicImage, ImageFormat};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::{
    cmp,
    io::{Cursor, SeekFrom},
    path::PathBuf,
    sync::Arc,
};
use tokio::{
    fs,
    io::{AsyncReadExt, AsyncSeekExt},
};
use uuid::Uuid;

const IMAGE_FORMATS: &[&str] = &["jpeg", "png", "gif", "webp", "avif", "ico"];
const UPLOAD_FORMATS: &[&str] = &["jpeg", "png", "gif", "webp", "avif", "ico", "mp4"];

#[derive(Debug, Deserialize)]
pub struct TransformQuery {
    pub w: Option<u32>,
    pub h: Option<u32>,
    pub fmt: Option<String>,
    pub q: Option<u8>,
}

#[derive(Debug, Serialize)]
struct UploadResponse {
    id: String,
    duplicate: bool,
    url: String,
    player_url: Option<String>,
    sha256: String,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/images", get(list_images).post(upload))
        .route("/api/images/:id", get(image_info).delete(delete_image))
        .route("/api/cache", delete(clear_cache))
        .route("/api/stats", get(stats))
        .route("/api/status", get(status))
        .route("/i/:id", get(serve_image))
        .route("/p/:id", get(player))
}

async fn upload(
    State(state): State<Arc<AppState>>,
    ApiKeyUser(user): ApiKeyUser,
    mut multipart: Multipart,
) -> Response {
    let mut file_name = "upload".to_string();
    let mut data: Option<Bytes> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("file") {
            file_name = field.file_name().unwrap_or("upload").to_string();
            match field.bytes().await {
                Ok(bytes) => data = Some(bytes),
                Err(_) => return error(StatusCode::BAD_REQUEST, "failed to read multipart file"),
            }
            break;
        }
    }

    let bytes = match data {
        Some(bytes) if !bytes.is_empty() => bytes,
        _ => return error(StatusCode::BAD_REQUEST, "missing file field"),
    };

    let detected = match detect_format(&bytes) {
        Some(format) => format,
        None => {
            return error(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "unsupported media format",
            )
        }
    };
    let sha256 = hex::encode(Sha256::digest(&bytes));

    if let Ok(Some(row)) =
        sqlx::query("SELECT id, ext FROM images WHERE owner_id = ? AND sha256 = ?")
            .bind(&user.id)
            .bind(&sha256)
            .fetch_optional(&state.pool)
            .await
    {
        let id: String = row.get("id");
        return (
            StatusCode::OK,
            Json(UploadResponse {
                id: id.clone(),
                duplicate: true,
                url: format!("/i/{id}"),
                player_url: player_url_for_ext(&id, row.get("ext")),
                sha256,
            }),
        )
            .into_response();
    }

    let dims = if detected.is_image {
        image::load_from_memory(&bytes)
            .ok()
            .map(|img| (img.width(), img.height()))
    } else {
        None
    };
    let id = Uuid::new_v4().to_string();
    let path = original_path(&state, &sha256, detected.ext);
    if fs::metadata(&path).await.is_err() {
        if let Err(err) = fs::write(&path, &bytes).await {
            tracing::error!(?err, "failed to store original");
            return error(StatusCode::INTERNAL_SERVER_ERROR, "failed to store image");
        }
    }

    let result = sqlx::query(
        r#"
        INSERT INTO images (id, owner_id, sha256, file_name, mime, ext, size_bytes, width, height, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(&user.id)
    .bind(&sha256)
    .bind(&file_name)
    .bind(detected.mime)
    .bind(detected.ext)
    .bind(bytes.len() as i64)
    .bind(dims.map(|d| d.0 as i64))
    .bind(dims.map(|d| d.1 as i64))
    .bind(Utc::now().to_rfc3339())
    .execute(&state.pool)
    .await;

    match result {
        Ok(_) => (
            StatusCode::CREATED,
            Json(UploadResponse {
                id: id.clone(),
                duplicate: false,
                url: format!("/i/{id}"),
                player_url: player_url_for_ext(&id, detected.ext.to_string()),
                sha256,
            }),
        )
            .into_response(),
        Err(err) => {
            tracing::error!(?err, "failed to insert image row");
            error(StatusCode::INTERNAL_SERVER_ERROR, "failed to save metadata")
        }
    }
}

async fn serve_image(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Query(query): Query<TransformQuery>,
) -> Response {
    let row = match sqlx::query("SELECT sha256, mime, ext FROM images WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.pool)
        .await
    {
        Ok(Some(row)) => row,
        Ok(None) => return error(StatusCode::NOT_FOUND, "image not found"),
        Err(_) => return error(StatusCode::INTERNAL_SERVER_ERROR, "database error"),
    };

    let sha256: String = row.get("sha256");
    let original_ext: String = row.get("ext");
    let original_path = original_path(&state, &sha256, &original_ext);

    let requested_format = query
        .fmt
        .as_deref()
        .unwrap_or(&original_ext)
        .to_ascii_lowercase();
    if original_ext == "mp4" {
        if query.w.is_some() || query.h.is_some() || query.q.is_some() || query.fmt.is_some() {
            return error(
                StatusCode::BAD_REQUEST,
                "mp4 files are served directly and cannot be image-transformed",
            );
        }
        return file_response(original_path, row.get("mime"), &headers).await;
    }

    if !IMAGE_FORMATS.contains(&requested_format.as_str()) {
        return error(StatusCode::BAD_REQUEST, "unsupported output format");
    }

    let needs_transform = query.w.is_some()
        || query.h.is_some()
        || query.q.is_some()
        || requested_format != original_ext;
    if !needs_transform {
        return file_response(original_path, row.get("mime"), &headers).await;
    }

    if query.w.is_some_and(|v| v == 0 || v > 8192) || query.h.is_some_and(|v| v == 0 || v > 8192) {
        return error(
            StatusCode::BAD_REQUEST,
            "w and h must be between 1 and 8192",
        );
    }
    if query.q.is_some_and(|v| v == 0 || v > 100) {
        return error(StatusCode::BAD_REQUEST, "q must be between 1 and 100");
    }

    let query_key = format!(
        "w={:?}&h={:?}&fmt={}&q={:?}",
        query.w, query.h, requested_format, query.q
    );
    let key = cache::TransformCache::key(&id, &query_key);
    if let Some(path) = state.cache.get(&key).await {
        return file_response(path, mime_for_ext(&requested_format).to_string(), &headers).await;
    }

    let original = match fs::read(&original_path).await {
        Ok(bytes) => bytes,
        Err(_) => return error(StatusCode::NOT_FOUND, "original file missing"),
    };
    let transformed = match transform(&original, &query, &requested_format) {
        Ok(bytes) => bytes,
        Err(err) => {
            tracing::warn!(?err, "image transform failed");
            return error(StatusCode::BAD_REQUEST, "image transform failed");
        }
    };

    let cache_path = state
        .cache
        .path_for(&key, ext_for_format(&requested_format));
    if let Err(err) = fs::write(&cache_path, &transformed).await {
        tracing::error!(?err, "failed to write cache file");
        return error(StatusCode::INTERNAL_SERVER_ERROR, "failed to write cache");
    }
    let size = cache::file_size(&cache_path)
        .await
        .unwrap_or(transformed.len() as u64);
    let _ = state.cache.insert(key, cache_path, size).await;
    let _ = sqlx::query("UPDATE images SET hits = hits + 1 WHERE id = ?")
        .bind(&id)
        .execute(&state.pool)
        .await;
    bytes_response(transformed, mime_for_ext(&requested_format))
}

async fn player(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Response {
    let row = match sqlx::query("SELECT id, file_name, ext FROM images WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.pool)
        .await
    {
        Ok(Some(row)) => row,
        Ok(None) => return error(StatusCode::NOT_FOUND, "media not found"),
        Err(_) => return error(StatusCode::INTERNAL_SERVER_ERROR, "database error"),
    };
    let ext: String = row.get("ext");
    if ext != "mp4" {
        return error(StatusCode::BAD_REQUEST, "player is only available for mp4");
    }
    let title = escape_html(&row.get::<String, _>("file_name"));
    Html(format!(
        r#"<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>{title}</title>
<style>
:root{{--canvas:#faf9f5;--ink:#141413;--muted:#6c6a64;--primary:#cc785c;--dark:#181715}}
*{{box-sizing:border-box}}body{{margin:0;min-height:100vh;background:var(--canvas);color:var(--ink);font-family:Inter,-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,sans-serif;display:grid;place-items:center;padding:28px}}
main{{width:min(1080px,100%);display:grid;gap:16px}}h1{{font-family:"Cormorant Garamond","EB Garamond",Garamond,"Times New Roman",serif;font-weight:500;letter-spacing:-.03em;font-size:34px;line-height:1.15;margin:0}}p{{margin:0;color:var(--muted)}}.frame{{background:var(--dark);border-radius:16px;padding:16px}}video{{display:block;width:100%;max-height:74vh;border-radius:12px;background:#000}}a{{color:var(--primary);text-decoration:none}}
</style>
</head>
<body>
<main>
  <div><h1>{title}</h1><p><a href="/i/{id}">打开原始 MP4 资源</a></p></div>
  <section class="frame"><video controls playsinline preload="metadata" src="/i/{id}"></video></section>
</main>
</body>
</html>"#
    ))
    .into_response()
}

async fn list_images(State(state): State<Arc<AppState>>, AuthUser(user): AuthUser) -> Response {
    let (sql, bind_user) = if user.role == "admin" {
        ("SELECT images.id, images.owner_id, users.username AS owner_username, images.file_name, images.mime, images.ext, images.sha256, images.size_bytes, images.width, images.height, images.created_at, images.hits FROM images LEFT JOIN users ON users.id = images.owner_id ORDER BY images.created_at DESC LIMIT 500", None)
    } else {
        ("SELECT images.id, images.owner_id, users.username AS owner_username, images.file_name, images.mime, images.ext, images.sha256, images.size_bytes, images.width, images.height, images.created_at, images.hits FROM images LEFT JOIN users ON users.id = images.owner_id WHERE images.owner_id = ? ORDER BY images.created_at DESC LIMIT 200", Some(user.id))
    };
    let mut query = sqlx::query(sql);
    if let Some(id) = bind_user {
        query = query.bind(id);
    }
    match query.fetch_all(&state.pool).await {
        Ok(rows) => {
            let images: Vec<_> = rows.into_iter().map(|row| image_json(row)).collect();
            Json(images).into_response()
        }
        Err(_) => error(StatusCode::INTERNAL_SERVER_ERROR, "failed to list images"),
    }
}

async fn image_info(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> Response {
    let query = sqlx::query(
        "SELECT images.id, images.owner_id, users.username AS owner_username, images.file_name, images.mime, images.ext, images.sha256, images.size_bytes, images.width, images.height, images.created_at, images.hits FROM images LEFT JOIN users ON users.id = images.owner_id WHERE images.id = ?",
    )
    .bind(&id);
    let row = match query.fetch_optional(&state.pool).await {
        Ok(Some(row)) => row,
        Ok(None) => return error(StatusCode::NOT_FOUND, "image not found"),
        Err(_) => return error(StatusCode::INTERNAL_SERVER_ERROR, "failed to load image"),
    };
    let owner_id: String = row.get("owner_id");
    if user.role != "admin" && user.id != owner_id {
        return error(StatusCode::FORBIDDEN, "image belongs to another user");
    }
    Json(image_json(row)).into_response()
}

async fn delete_image(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> Response {
    let row = match sqlx::query("SELECT owner_id, sha256, ext FROM images WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.pool)
        .await
    {
        Ok(Some(row)) => row,
        Ok(None) => return error(StatusCode::NOT_FOUND, "image not found"),
        Err(_) => return error(StatusCode::INTERNAL_SERVER_ERROR, "failed to load image"),
    };
    let owner_id: String = row.get("owner_id");
    if user.role != "admin" && user.id != owner_id {
        return error(StatusCode::FORBIDDEN, "image belongs to another user");
    }
    let sha256: String = row.get("sha256");
    let ext: String = row.get("ext");
    match sqlx::query("DELETE FROM images WHERE id = ?")
        .bind(&id)
        .execute(&state.pool)
        .await
    {
        Ok(result) if result.rows_affected() == 1 => {
            let still_used: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM images WHERE sha256 = ?")
                    .bind(&sha256)
                    .fetch_one(&state.pool)
                    .await
                    .unwrap_or(1);
            if still_used == 0 {
                let _ = fs::remove_file(original_path(&state, &sha256, &ext)).await;
            }
            let _ = state.cache.clear().await;
            Json(serde_json::json!({"ok": true, "id": id})).into_response()
        }
        Ok(_) => error(StatusCode::NOT_FOUND, "image not found"),
        Err(_) => error(StatusCode::INTERNAL_SERVER_ERROR, "failed to delete image"),
    }
}

async fn clear_cache(State(state): State<Arc<AppState>>, AuthUser(user): AuthUser) -> Response {
    if user.role != "admin" {
        return error(StatusCode::FORBIDDEN, "admin required");
    }
    match state.cache.clear().await {
        Ok(_) => Json(serde_json::json!({"ok": true})).into_response(),
        Err(_) => error(StatusCode::INTERNAL_SERVER_ERROR, "failed to clear cache"),
    }
}

async fn stats(State(state): State<Arc<AppState>>, AuthUser(user): AuthUser) -> Response {
    let (image_count, bytes) = if user.role == "admin" {
        let image_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM images")
            .fetch_one(&state.pool)
            .await
            .unwrap_or(0);
        let bytes: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(size_bytes), 0) FROM images")
            .fetch_one(&state.pool)
            .await
            .unwrap_or(0);
        (image_count, bytes)
    } else {
        let image_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM images WHERE owner_id = ?")
            .bind(&user.id)
            .fetch_one(&state.pool)
            .await
            .unwrap_or(0);
        let bytes: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(size_bytes), 0) FROM images WHERE owner_id = ?",
        )
        .bind(&user.id)
        .fetch_one(&state.pool)
        .await
        .unwrap_or(0);
        (image_count, bytes)
    };
    let user_count: i64 = if user.role == "admin" {
        sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(&state.pool)
            .await
            .unwrap_or(0)
    } else {
        1
    };
    Json(serde_json::json!({
        "images": image_count,
        "users": user_count,
        "stored_bytes": bytes,
        "cache": state.cache.stats()
    }))
    .into_response()
}

async fn status(State(state): State<Arc<AppState>>) -> Response {
    Json(serde_json::json!({
        "ok": true,
        "service": "picbad",
        "version": env!("CARGO_PKG_VERSION"),
        "max_upload_bytes": state.config.max_upload_bytes,
        "supported_formats": UPLOAD_FORMATS,
        "transform_formats": IMAGE_FORMATS
    }))
    .into_response()
}

fn image_json(row: sqlx::sqlite::SqliteRow) -> serde_json::Value {
    let id: String = row.get("id");
    serde_json::json!({
        "id": id,
        "owner_id": row.get::<String, _>("owner_id"),
        "owner_username": row.try_get::<String, _>("owner_username").ok(),
        "file_name": row.get::<String, _>("file_name"),
        "mime": row.get::<String, _>("mime"),
        "ext": row.get::<String, _>("ext"),
        "sha256": row.get::<String, _>("sha256"),
        "size_bytes": row.get::<i64, _>("size_bytes"),
        "width": row.try_get::<i64, _>("width").ok(),
        "height": row.try_get::<i64, _>("height").ok(),
        "created_at": row.get::<String, _>("created_at"),
        "hits": row.get::<i64, _>("hits"),
        "url": format!("/i/{id}"),
        "player_url": player_url_for_ext(&id, row.get::<String, _>("ext")),
    })
}

fn transform(input: &[u8], query: &TransformQuery, format: &str) -> anyhow::Result<Vec<u8>> {
    let mut image = image::load_from_memory(input)?;
    if query.w.is_some() || query.h.is_some() {
        image = resize(image, query.w, query.h);
    }
    let mut out = Cursor::new(Vec::new());
    if format == "jpeg" || format == "jpg" {
        let quality = query.q.unwrap_or(85).clamp(1, 100);
        let rgb = image.to_rgb8();
        JpegEncoder::new_with_quality(&mut out, quality).encode(
            &rgb,
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )?;
    } else {
        image.write_to(&mut out, image_format(format)?)?;
    }
    Ok(out.into_inner())
}

fn resize(image: DynamicImage, w: Option<u32>, h: Option<u32>) -> DynamicImage {
    match (w, h) {
        (Some(w), Some(h)) => image.resize(w, h, image::imageops::FilterType::Lanczos3),
        (Some(w), None) => {
            let ratio = w as f64 / image.width().max(1) as f64;
            let h = ((image.height().max(1) as f64 * ratio).round() as u32).max(1);
            image.resize(w, h, image::imageops::FilterType::Lanczos3)
        }
        (None, Some(h)) => {
            let ratio = h as f64 / image.height().max(1) as f64;
            let w = ((image.width().max(1) as f64 * ratio).round() as u32).max(1);
            image.resize(w, h, image::imageops::FilterType::Lanczos3)
        }
        (None, None) => image,
    }
}

fn image_format(format: &str) -> anyhow::Result<ImageFormat> {
    Ok(match format {
        "jpeg" | "jpg" => ImageFormat::Jpeg,
        "png" => ImageFormat::Png,
        "gif" => ImageFormat::Gif,
        "webp" => ImageFormat::WebP,
        "avif" => ImageFormat::Avif,
        "ico" => ImageFormat::Ico,
        _ => anyhow::bail!("unsupported format"),
    })
}

struct DetectedFormat {
    ext: &'static str,
    mime: &'static str,
    is_image: bool,
}

fn detect_format(bytes: &[u8]) -> Option<DetectedFormat> {
    if is_mp4(bytes) {
        return Some(DetectedFormat {
            ext: "mp4",
            mime: "video/mp4",
            is_image: false,
        });
    }
    let format = image::guess_format(bytes).ok()?;
    match format {
        ImageFormat::Jpeg => Some(DetectedFormat {
            ext: "jpeg",
            mime: "image/jpeg",
            is_image: true,
        }),
        ImageFormat::Png => Some(DetectedFormat {
            ext: "png",
            mime: "image/png",
            is_image: true,
        }),
        ImageFormat::Gif => Some(DetectedFormat {
            ext: "gif",
            mime: "image/gif",
            is_image: true,
        }),
        ImageFormat::WebP => Some(DetectedFormat {
            ext: "webp",
            mime: "image/webp",
            is_image: true,
        }),
        ImageFormat::Avif => Some(DetectedFormat {
            ext: "avif",
            mime: "image/avif",
            is_image: true,
        }),
        ImageFormat::Ico => Some(DetectedFormat {
            ext: "ico",
            mime: "image/x-icon",
            is_image: true,
        }),
        _ => None,
    }
}

fn is_mp4(bytes: &[u8]) -> bool {
    bytes.len() >= 12 && &bytes[4..8] == b"ftyp"
}

fn original_path(state: &AppState, sha256: &str, ext: &str) -> PathBuf {
    state.config.originals_dir().join(format!("{sha256}.{ext}"))
}

fn ext_for_format(format: &str) -> &str {
    if format == "jpg" {
        "jpeg"
    } else {
        match format {
            "jpeg" | "png" | "gif" | "webp" | "avif" | "ico" | "mp4" => format,
            _ => "bin",
        }
    }
}

fn mime_for_ext(ext: &str) -> &'static str {
    match ext {
        "jpeg" | "jpg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "ico" => "image/x-icon",
        "mp4" => "video/mp4",
        _ => "application/octet-stream",
    }
}

async fn file_response(
    path: PathBuf,
    content_type: String,
    request_headers: &HeaderMap,
) -> Response {
    let size = match fs::metadata(&path).await {
        Ok(metadata) => metadata.len(),
        Err(_) => return error(StatusCode::NOT_FOUND, "file not found"),
    };

    if let Some(range) = request_headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| parse_range(value, size))
    {
        let (start, end) = range;
        let length = end - start + 1;
        let mut file = match fs::File::open(&path).await {
            Ok(file) => file,
            Err(_) => return error(StatusCode::NOT_FOUND, "file not found"),
        };
        if file.seek(SeekFrom::Start(start)).await.is_err() {
            return error(StatusCode::INTERNAL_SERVER_ERROR, "failed to seek file");
        }
        let mut bytes = vec![0; length as usize];
        if file.read_exact(&mut bytes).await.is_err() {
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to read file range",
            );
        }
        let mut headers = media_headers(&content_type, length);
        headers.insert(
            header::CONTENT_RANGE,
            HeaderValue::from_str(&format!("bytes {start}-{end}/{size}"))
                .unwrap_or(HeaderValue::from_static("bytes */*")),
        );
        return (StatusCode::PARTIAL_CONTENT, headers, Body::from(bytes)).into_response();
    }

    match fs::read(path).await {
        Ok(bytes) => {
            let headers = media_headers(&content_type, bytes.len() as u64);
            (headers, Body::from(bytes)).into_response()
        }
        Err(_) => error(StatusCode::NOT_FOUND, "file not found"),
    }
}

fn bytes_response(bytes: impl Into<Vec<u8>>, content_type: &str) -> Response {
    let bytes = bytes.into();
    let headers = media_headers(content_type, bytes.len() as u64);
    (headers, Body::from(bytes)).into_response()
}

fn error(status: StatusCode, message: &str) -> Response {
    (status, Json(serde_json::json!({ "error": message }))).into_response()
}

fn media_headers(content_type: &str, content_length: u64) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(content_type)
            .unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    );
    headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    headers.insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&content_length.to_string()).unwrap_or(HeaderValue::from_static("0")),
    );
    headers
}

fn parse_range(value: &str, size: u64) -> Option<(u64, u64)> {
    if size == 0 {
        return None;
    }
    let range = value.strip_prefix("bytes=")?;
    let (start, end) = range.split_once('-')?;
    if start.is_empty() {
        let suffix = end.parse::<u64>().ok()?.min(size);
        return Some((size - suffix, size - 1));
    }
    let start = start.parse::<u64>().ok()?;
    if start >= size {
        return None;
    }
    let end = if end.is_empty() {
        size - 1
    } else {
        cmp::min(end.parse::<u64>().ok()?, size - 1)
    };
    (start <= end).then_some((start, end))
}

fn player_url_for_ext(id: &str, ext: String) -> Option<String> {
    (ext == "mp4").then(|| format!("/p/{id}"))
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
