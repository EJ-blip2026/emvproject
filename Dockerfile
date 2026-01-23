FROM rust:latest as builder

WORKDIR /app

# Copy source code
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations
COPY assets ./assets
COPY public ./public

# Build release - use committed Cargo.lock for reproducible builds
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y libssl3 ca-certificates && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/emvproject /usr/local/bin/
COPY --from=builder /app/public /app/public
COPY --from=builder /app/migrations /app/migrations

WORKDIR /app

ENV PORT=8080
EXPOSE 8080

CMD ["emvproject"]
