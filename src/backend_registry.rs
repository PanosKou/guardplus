// src/backend_registry.rs

use parking_lot::RwLock;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;

/// Service entry for discovery
#[derive(Debug, Deserialize, Clone)]
pub struct ServiceEntry {
    #[warn(dead_code)]
    pub name: String,
    pub url: String,
}

/// Thread-safe registry mapping service names → backend entries
#[derive(Debug, Default, Clone)]
pub struct BackendRegistry {
    services: Arc<RwLock<HashMap<String, Vec<ServiceEntry>>>>,
    indices: Arc<RwLock<HashMap<String, usize>>>,
}

impl BackendRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a new backend under `name`
    pub fn register(&self, name: &str, url: &str) {
        let mut map = self.services.write();
        let entry = ServiceEntry {
            name: name.to_string(),
            url: url.to_string(),
        };
        map.entry(name.to_string()).or_default().push(entry);
    }

    /// Remove (exact‐match on URL) a backend under `name`
    #[warn(dead_code)]
    pub fn deregister(&self, name: &str, url: &str) {
        if let Some(vec) = self.services.write().get_mut(name) {
            vec.retain(|e| e.url != url);
        }
    }

    /// Pick one backend URL (round-robin) under `name`
    pub fn pick_one(&self, name: &str) -> Option<String> {
        let services = self.services.write();
        let backends = services.get(name)?;
        if backends.is_empty() {
            return None;
        }
        // bump & wrap the index
        let mut idx_map = self.indices.write();
        let ctr = idx_map.entry(name.to_string()).or_insert(0);
        let url = backends[*ctr % backends.len()].url.clone();
        *ctr = (*ctr + 1) % backends.len();
        Some(url)
    }

    /// List all backend URLs under `name`
    #[warn(dead_code)]
    pub fn list(&self, name: &str) -> Vec<String> {
        self.services
            .read()
            .get(name)
            .map(|vec| vec.iter().map(|e| e.url.clone()).collect())
            .unwrap_or_default()
    }

    /// **New**: get the full entries (including `name`) under `name`
    pub fn list_entries(&self, name: &str) -> Vec<ServiceEntry> {
        self.services.read().get(name).cloned().unwrap_or_default()
    }
}
