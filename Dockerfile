# Stage 1 — Build the React web UI
FROM node:20-alpine AS web-builder
WORKDIR /build
COPY web-ui/package.json web-ui/package-lock.json ./
RUN npm ci
COPY web-ui/ ./
RUN npm run build

# Stage 2 — Compile the Rust binary (with embedded web UI assets)
FROM rust:1-slim-bookworm AS rust-builder
# sqlx with runtime-tokio-rustls uses pure Rust TLS — no system PG or SSL libraries needed.
RUN apt-get update && apt-get install -y pkg-config && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY . .
# Drop in the freshly-built web UI so the binary embeds it at compile time
COPY --from=web-builder /build/dist web-ui/dist
RUN cargo build --release --bin kiro-gateway

# Stage 3 — Minimal runtime image
FROM debian:bookworm-slim
# ca-certificates: required for TLS outbound calls to the Kiro API
# curl: used by the Docker HEALTHCHECK
RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=rust-builder /build/target/release/kiro-gateway /app/kiro-gateway
# /certs — mount point for TLS cert + key (bind mount, operator-managed)
RUN mkdir -p /certs
EXPOSE 8000
CMD ["/app/kiro-gateway"]
