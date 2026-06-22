# Mononium node — multi-stage Docker build
# ==========================================
# Stage 1: Build
FROM rust:1.85-slim-bookworm AS builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
COPY configs/ configs/

RUN cargo build --release -p mononium-cli && \
    cp target/release/mononium-cli /mononium-cli && \
    strip /mononium-cli

# Stage 2: Runtime
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /mononium-cli /usr/local/bin/mononium-cli
COPY configs/ /etc/mononium/configs/

VOLUME ["/data"]
EXPOSE 30333 9933 9944

ENTRYPOINT ["mononium-cli", "node"]
