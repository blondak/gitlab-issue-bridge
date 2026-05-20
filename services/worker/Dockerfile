FROM rust:1.88 AS builder
WORKDIR /app

COPY Cargo.toml Cargo.toml
COPY Cargo.lock Cargo.lock
COPY crates/bridge-core/Cargo.toml crates/bridge-core/Cargo.toml
COPY services/api/Cargo.toml services/api/Cargo.toml
COPY services/worker/Cargo.toml services/worker/Cargo.toml
RUN mkdir -p crates/bridge-core/src services/api/src services/worker/src && \
    printf 'pub fn placeholder() {}\n' > crates/bridge-core/src/lib.rs && \
    printf 'fn main() {}\n' > services/api/src/main.rs && \
    printf 'fn main() {}\n' > services/worker/src/main.rs
RUN cargo fetch --locked

COPY . .
RUN cargo build --locked -p issuehub-worker --release

FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/issuehub-worker /usr/local/bin/issuehub-worker
CMD ["issuehub-worker"]
