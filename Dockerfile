FROM rust:1.83 as builder

WORKDIR /app

# Copy manifests first for better caching
COPY Cargo.toml ./

# Create dummy main to cache dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Now copy real source
COPY src ./src
COPY migrations ./migrations
COPY third_party ./third_party
COPY public ./public

# Build for release
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

ENV PORT=3000
EXPOSE 3000

CMD ["emvproject"]
