# ---- Build Stage ----
FROM rust:1.82-bookworm AS builder

WORKDIR /build

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/

# Build release binary
RUN apt-get update && apt-get install -y libclang-dev cmake && \
    cargo build --release --bin velochain-node && \
    strip target/release/velochain-node

# ---- Runtime Stage ----
FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates libssl3 && \
    rm -rf /var/lib/apt/lists/* && \
    useradd -m -s /bin/bash velochain

WORKDIR /home/velochain

# Copy binary from builder
COPY --from=builder /build/target/release/velochain-node /usr/local/bin/velochain-node

# Copy default configuration
COPY docs/ docs/

# Create data directories
RUN mkdir -p /home/velochain/data /home/velochain/keystore && \
    chown -R velochain:velochain /home/velochain

USER velochain

# Default ports: RPC (8545), P2P (30303), Metrics (9090)
EXPOSE 8545 30303 9090

# Data volume
VOLUME ["/home/velochain/data"]

ENTRYPOINT ["velochain-node"]
CMD ["run", "--data-dir", "/home/velochain/data"]
