# ── Build stage ──
FROM rust:1.88-bookworm AS builder

WORKDIR /app

# Install OpenSSL dev dependencies
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy workspace manifests first for layer caching
COPY Cargo.toml Cargo.lock ./
COPY crates/ingestion/Cargo.toml crates/ingestion/
COPY crates/decoder/Cargo.toml crates/decoder/
COPY crates/storage/Cargo.toml crates/storage/
COPY crates/api/Cargo.toml crates/api/
COPY crates/filter_dsl/Cargo.toml crates/filter_dsl/
COPY bin/soltrace/Cargo.toml bin/soltrace/

# Create dummy source files so cargo can fetch + compile dependencies
RUN mkdir -p crates/ingestion/src crates/decoder/src crates/storage/src crates/api/src crates/filter_dsl/src bin/soltrace/src \
    && echo "pub fn stub(){}" > crates/ingestion/src/lib.rs \
    && echo "pub fn stub(){}" > crates/decoder/src/lib.rs \
    && echo "pub fn stub(){}" > crates/storage/src/lib.rs \
    && echo "pub fn stub(){}" > crates/api/src/lib.rs \
    && echo "pub fn stub(){}" > crates/filter_dsl/src/lib.rs \
    && echo "fn main(){}" > bin/soltrace/src/main.rs

# Build dependencies only (cached unless Cargo.toml/lock changes)
RUN cargo build --release --bin soltrace 2>/dev/null || true

# Copy real source code
COPY crates/ crates/
COPY bin/ bin/

# Touch source files to invalidate the dummy builds
RUN touch crates/ingestion/src/lib.rs crates/decoder/src/lib.rs \
    crates/storage/src/lib.rs crates/api/src/lib.rs \
    crates/filter_dsl/src/lib.rs bin/soltrace/src/main.rs

# Build the real binary
RUN cargo build --release --bin soltrace

# ── Runtime stage ──
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/soltrace /usr/local/bin/soltrace
COPY config/ /app/config/
COPY migrations/ /app/migrations/

WORKDIR /app

EXPOSE 3000

CMD ["soltrace"]
