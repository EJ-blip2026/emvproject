# emvproject

Minimal Rust project scaffold for `emvproject`.

Quick start

```bash
# build + run (dev)
cargo build
cargo run
```

Makefile targets

- `make migrate` — run embedded sqlx migrations (uses `DATABASE_URL` or `sqlite://data/keys.db`).
- `make run` — start the server (dev).
- `make build` — build release binary.
- `make test` — run tests.
- `make fmt` — format code.

Running migrations

The project embeds `sqlx` migrations under `migrations/` and provides a `migrate` CLI subcommand.

Examples:

```bash
# run migrations (sqlite fallback)
make migrate

# run migrations against Postgres
export DATABASE_URL=postgres://user:pass@host/dbname
cargo run -- migrate
```

Environment variables

- `DATABASE_URL` — DB connection string (Postgres recommended). Defaults to `sqlite://data/keys.db`.
- `REDIS_URL` — optional Redis URL for distributed rate-limiting.
- `ADMIN_TOKEN` — admin token for admin endpoints (default `admintoken`).
- `API_KEYS` — comma-separated initial API keys (used if DB empty).
- `RATE_LIMIT` / `RATE_WINDOW_SECS` — rate limiting configuration.

Dev workflow

1. Run migrations: `make migrate`.
2. Start server: `make run`.
3. Add API keys (admin):

```bash
curl -X POST -H "Content-Type: application/json" -H "x-admin-token: admintoken" \
  -d '{"key":"mykey123"}' http://127.0.0.1:3000/admin/keys
```

4. Call API:

```bash
curl -H "x-api-key: mykey123" http://127.0.0.1:3000/api/haikus
```

Production notes

- Use Postgres (`DATABASE_URL`) and `REDIS_URL` for rate-limiting in production.
- Add TLS (reverse proxy or TLS termination), logging, monitoring, and CI-run migrations before deploy.
- Consider using `sqlx` CLI for local development migrations and `sqlx::migrate!` embedded migrations for runtime.
# emvproject

Minimal Rust project scaffold for `emvproject`.

Build and run:

```bash
cargo build
cargo run --quiet
```

Library API:

- `emvproject::run()` — example entry used by the binary.

This repo was cleaned of stray files and scaffolded with a minimal Cargo layout.
# emvproject