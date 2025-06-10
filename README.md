
# GuardPlus Gateway

GuardPlus is a high-performance, extensible Rust-based service gateway designed as a modern, secure, and observable replacement for Traefik, NGINX, or Apache.

## 🔥 Key Features

- 🔐 **OIDC/JWT Auth** — Multi-provider (Google, GitHub, Auth0) with JWKS auto-discovery
- 📊 **Prometheus Metrics** — Rich HTTP/gRPC/TCP observability
- ⚡ **gRPC + TCP/UDP Support** — Proxy multiple protocols with metrics
- 🔁 **Consul Integration** — Dynamic backend discovery (planned)
- 🛡️ **Rate Limiting** — Custom middleware with full metric export
- 🔧 **Kubernetes Ready** — Includes Helm chart, Dockerfile, Makefile

---

## 🚀 Quick Start

### 🐳 Docker
```bash
docker build -t guardplus .
docker run -p 8080:8080 \
  -v $(pwd)/config.yaml:/app/config.yaml \
  -v $(pwd)/cert.pem:/app/cert.pem \
  -v $(pwd)/key.pem:/app/key.pem \
  guardplus
```

### ⎈ Helm (Kubernetes)
```bash
helm install guardplus ./chart \
  --set image.repository=guardplus --set image.tag=latest
```

---

## ⚙️ Configuration Example (`config.yaml`)

```yaml
http_port: 8080

auth:
  oidc_providers:
    - name: google
      issuer_url: "https://accounts.google.com"
      audience: "your-client-id"

tls:
  cert_path: "./cert.pem"
  key_path: "./key.pem"

consul:
  enabled: true
  url: "http://localhost:8500"

backends:
  - name: http-api
    protocol: http
    address: "http://localhost:9000"
    routes: ["/api/"]
```

---

## 📈 Metrics
Exposed at `/metrics` in Prometheus format.

| Metric                            | Description                           |
|----------------------------------|---------------------------------------|
| `guardplus_backend_requests`     | Count of routed requests              |
| `guardplus_response_latency_ms`  | Histogram of request durations        |
| `guardplus_grpc_requests`        | gRPC service/method counts            |
| `guardplus_tls_cert_expiry_days`| TLS cert expiration timeline          |
| `guardplus_ratelimit_blocked`    | Count of blocked requests             |

---

## 📊 Grafana Dashboard
Use the included JSON dashboard:
📥 `guardplus_grafana_dashboard.json`

---

## 🛡️ Maturity Overview

| Capability                       | Status          |
|----------------------------------|-----------------|
| OIDC/Authn/Authz                 | ✅ Production    |
| HTTP/gRPC/TCP Proxying           | ✅ Production    |
| Metrics + Observability          | ✅ Production    |
| TLS Termination                  | ✅ Production    |
| Rate Limiting                    | ✅ Stable        |
| Consul Discovery                 | 🛠️ Planned       |
| Hot Config Reload                | 🛠️ Planned       |
| UI/Dashboard                     | ❌ Not yet       |
| Canary / A/B Routing             | 🛠️ Next phase    |

---

## 🙋 Contributing
Want to build Rust-powered edge tooling? PRs welcome!

## 📄 License
Apache-2.0


## Notes
Build
```bash
cd guard_plus
cargo build --release
```
Run local mock backends
You’ll want something listening on the ports we registered (9000/9001 for HTTP, 50052 for gRPC, 9100/9101 for TCP, 9200/9201 for UDP). For example, in separate terminals:

# HTTP backends for “foo”
```bash
python3 -m http.server 9000
python3 -m http.server 9001
```
gRPC mock on 50052 (you can write a quick Tonic echo server that implements the same proto).
Or reuse the `echo` service from grpc_service.rs by spawning a Tonic server on 50052.

# Simple TCP echo servers on 9100/9101:
```
nc -l 9100 -c 'xargs -n1 echo'
nc -l 9101 -c 'xargs -n1 echo'
```
# UDP echo on 9200/9201:
```
socat UDP-LISTEN:9200,reuseaddr,fork UDP:0.0.0.0:9200
socat UDP-LISTEN:9201,reuseaddr,fork UDP:0.0.0.0:9201
```
Run “Guard Plus”
```bash
RUST_LOG=info cargo run --release
```
Test HTTP
```bash
curl http://localhost:8080/foo/
```
# Should return whatever the Python HTTP server at port 9000 or 9001 serves.
Test TCP

```bash
# Connect to the TCP gateway at 91000
nc localhost 91000
hello tcp
```
# Should echo back “hello tcp”
Test UDP
```bash
echo -n "hello udp" | nc -u -w1 localhost 92000
# Should reply with “hello udp”
```
Test gRPC
Write a small gRPC client that does:

```rust
let mut client = EchoClient::connect("http://localhost:50051").await?;
let mut request = Request::new(EchoRequest { message: "hi".into() });
request.metadata_mut().insert("service-name", MetadataValue::from_static("bar"));
let response = client.say_hello(request).await?;
println!("REPLY={}", response.into_inner().message);
That will flow through the gateway to whichever backend (e.g. 127.0.0.1:50052).
```