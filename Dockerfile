FROM rust:1.83 as builder

WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock* ./

# Copy source
COPY src ./src
COPY migrations ./migrations
COPY third_party ./third_party
COPY public ./public

# Build with explicit edition
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y libssl3 ca-certificates && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/emvproject /usr/local/bin/
COPY --from=builder /app/public /app/public
COPY --from=builder /app/migrations /app/migrations
COPY --from=builder /app/third_party /app/third_party

WORKDIR /app

EXPOSE 3000

CMD ["emvproject"]
