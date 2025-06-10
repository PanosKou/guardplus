# 📦 Release: `v1.0.0` — Production-Ready Secure Gateway

We’re proud to launch **GuardPlus**, a Rust-native, secure, and observable reverse proxy gateway designed for modern service meshes and cloud-native architectures.

---

## 🚀 Features

- 🔐 **OpenID Connect (OIDC) Authentication**
  - Supports Google, GitHub, Auth0, and custom issuers
  - JWKS auto-fetch and cache
- ⚡ **Multi-Protocol Proxying**
  - HTTP, gRPC, TCP/UDP with round-robin balancing
- 📈 **Prometheus-Ready Metrics**
  - Backend request counters
  - gRPC method tracking
  - TLS certificate expiry tracking
  - Rate limit insights
- 📊 **Grafana Dashboard**
  - Preconfigured for protocol, route, and backend views
- 🛡️ **Rate Limiting Middleware**
  - Route- and method-aware request control
- 🧠 **Rust-Based Performance**
  - Safety + speed, extensible codebase
- 📦 **Helm Chart & Docker Support**
  - Easy Kubernetes deployment
  - CI/CD-ready with `Makefile`

---

## 📚 Documentation

- `README.md`
- 📄 `guardplus_full_documentation.pdf`
- 📊 `guardplus_grafana_dashboard.json`

---

## 📥 Assets

- 🔧 `guardplus_gateway_repo.zip` — Full Git repo (code, Helm, Docker)
- 📦 `guardplus_complete_package.zip` — Includes **all** documentation, dashboards, and diagrams

---

## 🗺️ Roadmap

- 🔄 Hot config reload via Consul and file watchers
- 🧩 Plugin system for route logic
- 🌐 Web UI with admin interface
- 🧪 Canary + A/B deploy support
- 🔗 Zero-trust service mesh integration

---

## 💬 Feedback & Contributions

- Issues and PRs welcome
- Designed for secure, compliant, observable environments