// src/backend_registry.rs
use parking_lot::RwLock;
use rand::Rng;
use serde::Deserialize;
use std::collections::HashMap;

/// Service entry for discovery
#[derive(Debug, Deserialize, Clone)]
pub struct ServiceEntry {
    pub name: String,
    pub url: String,
}

/// Backend registry with interior mutability
#[derive(Debug, Default)]
pub struct BackendRegistry {
    services: RwLock<HashMap<String, Vec<String>>>,
    indices:  RwLock<HashMap<String, usize>>,
}

impl BackendRegistry {
    pub fn new() -> Self {
        BackendRegistry {
            services: RwLock::new(HashMap::new()),
            indices:  RwLock::new(HashMap::new()),
        }
    }

    /// Register a backend URL under a service name
    pub fn register(&self, name: &str, url: &str) {
        let mut svc = self.services.write();
        svc.entry(name.to_string()).or_default().push(url.to_string());
    }

    /// Deregister a backend URL
    pub fn deregister(&self, name: &str, url: &str) {
        let mut svc = self.services.write();
        if let Some(vec) = svc.get_mut(name) {
            vec.retain(|u| u != url);
        }
    }

    /// Pick a backend in round-robin fashion
    pub fn pick_one(&self, service_name: &str) -> Option<String> {
        let svc_map = self.services.read();
        svc_map.get(service_name).and_then(|backends| {
            if backends.is_empty() {
                return None;
            }
            let mut idx_map = self.indices.write();
            let idx = idx_map.entry(service_name.to_string()).or_insert(0);
            let choice = backends.get(*idx % backends.len()).cloned();
            *idx = (*idx + 1) % backends.len();
            choice
        })
    }

    /// List all backends
    pub fn list(&self, name: &str) -> Vec<String> {
        self.services.read().get(name).cloned().unwrap_or_default()
    }
}