# Picbad

Picbad is a self-hosted image and media hosting service built with Rust. It provides per-user upload keys, SHA256 deduplicated storage, real-time image transforms, MP4 playback, an admin dashboard, and cache management.

## Features

- **Rust backend**: Axum + Tokio, memory-safe and async.
- **Multi-user isolation**: each user has a private upload key and their own media records.
- **First-user admin**: the first registered account becomes the administrator.
- **Deduplicated storage**: original files are stored once by SHA256, while each user gets their own media ID.
- **Image formats**: JPEG, PNG, GIF, WebP, AVIF, ICO.
- **Video support**: MP4 upload, inline preview, player page, and HTTP Range playback.
- **URL transforms**: resize and convert images with query parameters.
- **LRU transform cache**: generated image variants are cached and can be cleared from the dashboard.
- **Admin gallery**: administrators can view all users' media and delete normal user accounts.
- **Web dashboard**: upload, drag-and-drop, user management, API keys, media gallery, and system stats.

## Quick Start

```bash
cargo run --release
```

Open:

```text
http://localhost:8080/admin
```

On a fresh database, register the first account from the login page. That first account automatically becomes `admin`. Later registrations become normal users by default.

## Docker

```bash
docker compose up --build
```

The service listens on port `8080` by default.

## API

### Register

The first registered user becomes admin.

```bash
curl -X POST http://localhost:8080/api/register \
  -H "content-type: application/json" \
  -d '{"username":"alice","password":"StrongPass123!"}'
```

### Login

```bash
curl -X POST http://localhost:8080/api/login \
  -H "content-type: application/json" \
  -d '{"username":"alice","password":"StrongPass123!"}'
```

The response contains a management `token` and the user's `api_key`.

### Upload

Uploads require the per-user upload key:

```bash
curl -X POST http://localhost:8080/api/images \
  -H "x-api-key: <user_api_key>" \
  -F "file=@photo.png"
```

MP4 is also supported:

```bash
curl -X POST http://localhost:8080/api/images \
  -H "x-api-key: <user_api_key>" \
  -F "file=@video.mp4"
```

### List Media

```bash
curl http://localhost:8080/api/images \
  -H "authorization: Bearer <token>"
```

Normal users only see their own media. Admin users see all media, including the owner username.

### Delete Media

```bash
curl -X DELETE http://localhost:8080/api/images/<media_id> \
  -H "authorization: Bearer <token>"
```

Users can delete their own media. Admins can delete any media.

### Manage Users

Create a user as admin:

```bash
curl -X POST http://localhost:8080/api/users \
  -H "authorization: Bearer <admin_token>" \
  -H "content-type: application/json" \
  -d '{"username":"bob","password":"StrongPass123!","role":"user"}'
```

Rotate a user's upload key:

```bash
curl -X POST http://localhost:8080/api/users/<user_id>/key \
  -H "authorization: Bearer <admin_token>"
```

Delete a normal user:

```bash
curl -X DELETE http://localhost:8080/api/users/<user_id> \
  -H "authorization: Bearer <admin_token>"
```

Admins cannot delete themselves or other admin accounts.

### Status

```bash
curl http://localhost:8080/api/status
```

## Direct Links

Original media:

```text
/i/<media_id>
```

MP4 player page:

```text
/p/<media_id>
```

MP4 direct links support `Range` requests for browser playback and seeking.

## Image Transforms

Images can be transformed through URL parameters:

```text
/i/<image_id>?w=1200&fmt=webp&q=85
```

Supported parameters:

- `w`: width, 1-8192.
- `h`: height, 1-8192.
- `fmt`: `jpeg`, `png`, `gif`, `webp`, `avif`, `ico`.
- `q`: quality, 1-100. JPEG uses this quality value.

MP4 files are served directly and cannot be transformed with image parameters.

## Configuration

Environment variables:

| Variable | Default | Description |
|---|---:|---|
| `PICBAD_BIND` | `0.0.0.0:8080` | HTTP bind address |
| `PICBAD_DATA_DIR` | `./data` | Data directory |
| `PICBAD_DATABASE_URL` | `sqlite://data/picbad.sqlite` | SQLite database URL |
| `PICBAD_CACHE_MAX_BYTES` | `2147483648` | Transform cache size |
| `PICBAD_MAX_UPLOAD_BYTES` | `52428800` | Max upload size, 50 MB by default |

Example:

```bash
PICBAD_BIND=127.0.0.1:8080 \
PICBAD_MAX_UPLOAD_BYTES=104857600 \
cargo run --release
```

## Storage Layout

```text
data/
  originals/       # SHA256-addressed original files
  cache/           # transformed image variants
  picbad.sqlite    # metadata, users, API keys
```

## Security Notes

- Keep `data/` private and backed up.
- Upload API keys are secrets. Rotate them from the dashboard if exposed.
- Put Picbad behind HTTPS in production.
- Normal users cannot list or manage other users' media.
- Admins can view all media and delete normal users.

## License

Choose a license before publishing if this repository will be public.
