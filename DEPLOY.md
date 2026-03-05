# Deployment Guide (Vault API)

Minimal steps to ship the Vault API (Axum + sqlx) to Railway or Render.

## Prerequisites

- GitHub repository access
- PostgreSQL (preferred) or SQLite fallback
- `cargo` available locally if you want to test before deploy

## Railway (recommended)

1) Create project and link GitHub
- Railway → New Project → Deploy from GitHub → choose `emvproject`.

2) Add database
- Add Service → Database → PostgreSQL. Railway will inject `DATABASE_URL`.

3) Environment variables
- `ADMIN_TOKEN` (optional; defaults to `admintoken` if not set)
- `PORT` is provided by Railway; no change needed.

4) Deploy
- Push to `main`; Railway builds with `cargo build --release` and runs the binary.
- Migrations run automatically on startup from `./migrations`.

5) Health check
- Path: `/health`
- Expect `200` with `{ "status": "ok", "service": "vault-api" }`.

## Render (alternative)

1) New → Web Service → connect repo
- Build: `cargo build --release`
- Start: `./target/release/emvproject`

2) Add PostgreSQL
- New → PostgreSQL; Render sets `DATABASE_URL`.

3) Env vars
- Same as Railway (`ADMIN_TOKEN`, optional `PORT`).

4) Migrations
- They run on startup. If schema drift occurs, redeploy or run `cargo sqlx migrate run` inside the service shell.

## Local test before deploy

```bash
cargo test
DATABASE_URL=sqlite:///app/data/vault.db cargo run
curl -s http://localhost:3000/health
```

## Production checklist

- [ ] Strong `ADMIN_TOKEN` (32+ chars)
- [ ] PostgreSQL provisioned (avoid in-memory SQLite in prod)
- [ ] Health check configured to `/health`
- [ ] Logs monitored in Railway/Render dashboard
- [ ] Uptime check (e.g., UptimeRobot) hitting `/health`

## mTLS certificates (optional)

Create a private CA and client certificate for mTLS.

```bash
# 1. Create your own private CA
openssl genrsa -out ca.key 4096
openssl req -x509 -new -nodes -key ca.key -sha256 -days 3650 -out ca.crt

# 2. Create a client certificate (admin tool or laptop)
openssl genrsa -out admin_client.key 2048
openssl req -new -key admin_client.key -out admin_client.csr
openssl x509 -req -in admin_client.csr -CA ca.crt -CAkey ca.key -CAcreateserial \
	-out admin_client.crt -days 365 -sha256
```

Place the server-side certs at:

```
certs/ca.crt
certs/server.crt
certs/server.key
```

Example request using the client cert:

```bash
curl --cert admin_client.crt --key admin_client.key https://your-vault-api.com/admin/stats
```

Example router structure for public vs. mTLS-protected routes:

```rust
let app = Router::new()
	// 1. Public routes (no mTLS required)
	.route("/health", get(health_check))
	.route("/metrics", get(public_metrics))
	// 2. Protected routes (wrap these in your mTLS layer)
	.nest("/admin", admin_routes)
	.nest("/vault", vault_routes);
```

Alternate pattern using a dedicated mTLS layer:

```rust
let app = Router::new()
	// Public: no mTLS needed
	.route("/health", get(|| async { "OK" }))
	// Protected: nested under a layer that checks for the cert
	.nest("/api", protected_routes)
	.layer(mtls_layer);
```

Axum example with middleware for protected routes:

```rust
let app = Router::new()
	// Public: no mTLS required for health/pings
	.route("/health", get(|| async { "OK" }))
	// Protected: apply mTLS layer only to actual data routes
	.nest("/api/v1", protected_routes)
	.layer(from_fn(mtls_verification_middleware));
```

## Rollback

- Railway: Deployments → select previous → Rollback.
- Render: Redeploy an earlier commit or restart the service on a prior build.
