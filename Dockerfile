# -------- Stage 1: Build --------
FROM rust:1.74-slim AS builder

# Install build tools
RUN apt-get update && apt-get install -y pkg-config libssl-dev curl git && rm -rf /var/lib/apt/lists/*

# Set up build directory
WORKDIR /app

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY build.rs .  # if you use one
RUN cargo build --release

# -------- Stage 2: Runtime --------
FROM debian:bullseye-slim

# Install minimal runtime dependencies
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

# Set up app directory
WORKDIR /app

# Copy binary from build stage
COPY --from=builder /app/target/release/gamb /app/gamb

# Optional: copy config + TLS certs (if using file-based TLS)
COPY config.yaml /app/config.yaml
COPY cert.pem /app/cert.pem
COPY key.pem /app/key.pem

# Expose ports
EXPOSE 8080 8443 50051 9100 9200

# Set environment variable fallback
ENV GATEWAY_CONFIG=/app/config.yaml

# Run the gateway
ENTRYPOINT ["/app/gamb"]

# How-to
#docker build -t gamb .
#docker run -p 8080:8080 -p 8443:8443 -p 50051:50051 -p 9100:9100 -p 9200:9200 \
#  -v $(pwd)/config.yaml:/app/config.yaml \
#  -v $(pwd)/cert.pem:/app/cert.pem \
#  -v $(pwd)/key.pem:/app/key.pem \
#  gamb