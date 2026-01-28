# Gamb Gateway

Gamb is a high-performance, extensible Rust-based service gateway designed as a modern, secure, and observable replacement for Traefik, NGINX, or Apache.

## Key Features

- **Multi-Protocol Support** - HTTP, HTTPS, gRPC, TCP, and UDP proxying
- **Bearer Token Auth** - Simple token-based authentication
- **TLS Termination** - Secure connections with rustls (no OpenSSL dependency)
- **Rate Limiting** - Configurable request throttling via Tower middleware
- **Round-Robin Load Balancing** - Distribute traffic across multiple backends
- **Kubernetes Ready** - Includes Helm chart and Dockerfile

---

## Prerequisites

- **Rust** 1.70+ (install via [rustup](https://rustup.rs/))
- **OpenSSL** development libraries (for some dependencies)

```bash
# Ubuntu/Debian
sudo apt-get install pkg-config libssl-dev

# macOS
brew install openssl
```

---

## CLI Commands

### Build

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release
```

### Run

```bash
# Run with debug logging
RUST_LOG=info cargo run

# Run release build
RUST_LOG=info cargo run --release

# Or run the binary directly
RUST_LOG=info ./target/release/gamb
```

### Test

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture
```

### Format

```bash
# Check formatting
cargo fmt --check

# Apply formatting
cargo fmt
```

### Lint

```bash
# Run clippy linter
cargo clippy

# Run clippy with warnings as errors
cargo clippy -- -D warnings
```

### Clean

```bash
# Remove build artifacts
cargo clean
```

---

## Configuration

Create a `config.yaml` file in the project root:

```yaml
# HTTP listening port
http_port: 8080

# Optional ports (defaults shown)
# https_port: 8081      # defaults to http_port + 1
# grpc_port: 50051
# tcp_port: 9100
# udp_port: 9200

# Authentication
auth:
  oidc_providers:
    - name: github
      issuer_url: "https://token.actions.githubusercontent.com"
      audience: "https://github.com/org"

# TLS certificates
tls:
  cert_path: "./cert.pem"
  key_path: "./key.pem"

# Backend services
backends:
  - name: api
    protocol: http
    address: "http://127.0.0.1:9000"
    routes: ["/api"]
  - name: grpc_backend
    protocol: grpc
    address: "http://127.0.0.1:50052"
    routes: []
  - name: tcpservice
    protocol: tcp
    address: "127.0.0.1:9100"
    routes: []

# Service discovery (planned)
consul_url: "http://localhost:8500"
tls_mode: "file"
tls_domain: "example.com"
tls_email: "admin@example.com"

# Bearer token for authentication
bearer_token: "Bearer mysecrettoken"

# Rate limiting
rate_limit_per_sec: 100
rate_limit_burst: 50
```

---

## TLS Certificates

Generate self-signed certificates for development:

```bash
openssl req -x509 -newkey rsa:4096 \
  -keyout key.pem -out cert.pem \
  -days 365 -nodes \
  -subj "/CN=localhost"
```

---

## Quick Start

### 1. Build and run Gamb

```bash
# Generate TLS certificates
openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem -days 365 -nodes -subj "/CN=localhost"

# Build and run
RUST_LOG=info cargo run
```

### 2. Start a test backend

```bash
# Simple Python HTTP server on port 8087
python3 -m http.server 8087
```

### 3. Test the proxy

```bash
# Without auth (should return 401)
curl http://localhost:8080/echo/test
# Returns: 401 Unauthorized

# With auth (proxies to backend)
curl -H "Authorization: Bearer mysecrettoken" http://localhost:8080/echo/hello
# Returns response from backend
```

---

## Docker

### Build

```bash
docker build -t gamb .
```

### Run

```bash
docker run -p 8080:8080 -p 8081:8081 \
  -v $(pwd)/config.yaml:/app/config.yaml \
  -v $(pwd)/cert.pem:/app/cert.pem \
  -v $(pwd)/key.pem:/app/key.pem \
  gamb
```

---

## Kubernetes (Helm)

```bash
helm install gamb ./chart \
  --set image.repository=gamb \
  --set image.tag=latest
```

---

## Architecture

```
                    ┌─────────────────────────────────────┐
                    │            Gamb Gateway             │
                    ├─────────────────────────────────────┤
 HTTP :8080  ──────►│  HTTP Proxy (Axum + Tower)          │──────► Backend Services
HTTPS :8081  ──────►│  ├─ Bearer Auth Middleware          │
 gRPC :50051 ──────►│  ├─ Rate Limiting                   │
  TCP :9100  ──────►│  └─ Round-Robin Load Balancing      │
  UDP :9200  ──────►│                                     │
                    │  Backend Registry (Thread-safe)     │
                    └─────────────────────────────────────┘
```

---

## Testing Each Protocol

### HTTP/HTTPS

```bash
# Test HTTP proxy with auth
curl -H "Authorization: Bearer mysecrettoken" http://localhost:8080/servicename/path

# Test HTTPS (with self-signed cert)
curl -k -H "Authorization: Bearer mysecrettoken" https://localhost:8081/servicename/path
```

### TCP

```bash
# Start a TCP echo backend
nc -l 9100 -c 'cat'

# Connect through the gateway
nc localhost 9100
hello
# Should echo back: hello
```

### UDP

```bash
# Start a UDP echo backend
socat UDP-LISTEN:9200,reuseaddr,fork EXEC:cat

# Send through gateway
echo "hello" | nc -u -w1 localhost 9200
```

### gRPC

```rust
// Example gRPC client
let mut client = EchoClient::connect("http://localhost:50051").await?;
let mut request = Request::new(EchoRequest { message: "hello".into() });
request.metadata_mut().insert("service-name", "grpc_service".parse()?);
let response = client.echo(request).await?;
```

---

## Project Structure

```
gamb/
├── src/
│   ├── main.rs              # Entry point, spawns all gateways
│   ├── config.rs            # YAML configuration parsing
│   ├── backend_registry.rs  # Thread-safe service registry
│   ├── http_proxy.rs        # HTTP/HTTPS proxy implementation
│   ├── grpc_service.rs      # gRPC proxy (Echo service)
│   ├── tcp_udp_proxy.rs     # TCP/UDP proxy implementation
│   ├── tls_config.rs        # TLS/rustls configuration
│   ├── middleware.rs        # Tower middleware (auth, rate-limit)
│   └── consul_integration.rs # Consul discovery (planned)
├── proto/
│   └── echo.proto           # gRPC service definition
├── chart/                   # Helm chart for Kubernetes
├── Dockerfile
├── Cargo.toml
├── config.yaml
└── README.md
```

---

## Status

| Feature | Status |
|---------|--------|
| HTTP/HTTPS Proxying | Working |
| Bearer Token Auth | Working |
| Rate Limiting | Working |
| TLS Termination | Working |
| gRPC Proxying | Working |
| TCP Proxying | Working |
| UDP Proxying | Working |
| Round-Robin LB | Working |
| OIDC/JWT Validation | Planned |
| Prometheus Metrics | Planned |
| Consul Discovery | Planned |
| Hot Config Reload | Planned |

---

## License

Apache-2.0
