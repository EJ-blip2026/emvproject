.PHONY: migrate run dev build test fmt

# Run embedded sqlx migrations (uses DATABASE_URL or sqlite://data/keys.db)
migrate:
	@echo "Running DB migrations..."
	@DATABASE_URL=${DATABASE_URL:-sqlite://data/keys.db} cargo run -- migrate

# Run the server (development)
run:
	@echo "Starting server (dev)..."
	@cargo run

dev: run

build:
	@cargo build --release

test:
	@cargo test

fmt:
	@cargo fmt
