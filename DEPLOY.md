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

## Rollback

- Railway: Deployments → select previous → Rollback.
- Render: Redeploy an earlier commit or restart the service on a prior build.
