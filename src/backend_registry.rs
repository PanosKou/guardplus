use parking_lot::RwLock;
use rand::Rng;
use serde::Deserialize;
use std::{collections::HashMap, sync::Arc};
use std::sync::atomic::{AtomicUsize, Ordering};
// Description: Basic service discovery and backend registry for HTTP, gRPC, TCP, and UDP services
/// Simplest “service discovery” mock:
/// You can extend this to watch Consul/etcd/etc.
#[derive(Debug, Deserialize, Clone)]
pub struct ServiceEntry {
    pub name: String,
    pub url: String, // e.g. "http://127.0.0.1:9000"
}

struct BackendRegistry {
    services: HashMap<String, Vec<String>>,
    indices: HashMap<String, usize>,
}


impl BackendRegistry {
    pub fn new() -> Self {
        BackendRegistry {
            services: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new backend under a given service name
    pub fn register(&self, name: &str, url: &str) {
        let mut map = self.services.write();
        map.entry(name.to_string())
            .or_default()
            .push(url.to_string());
    }

    /// Remove a backend (by exact match) from a service
    pub fn deregister(&self, name: &str, url: &str) {
        if let Some(vec) = self.services.write().get_mut(name) {
            vec.retain(|u| u != url);
        }
    }

    /// Get one backend URL in a round-robin or random fashion
    fn pick_one(&mut self, service_name: &str) -> Option<String> {
        if let Some(backends) = self.services.get(service_name) {
            let index = self.indices.entry(service_name.to_string()).or_insert(0);
            let backend = backends.get(*index % backends.len()).cloned();
            *index = (*index + 1) % backends.len();
            backend
        } else {
            None
        }
    }

    /// List all backends registered under a service
    pub fn list(&self, name: &str) -> Vec<String> {
        let map = self.services.read();
        map.get(name).cloned().unwrap_or_default()
    }
}
