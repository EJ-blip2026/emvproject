FROM rust:1.70 as builder

WORKDIR /app

# Copy source code
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations
COPY third_party ./third_party
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
COPY --from=builder /app/third_party /app/third_party

WORKDIR /app

ENV PORT=3000
EXPOSE 3000

CMD ["emvproject"]

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
