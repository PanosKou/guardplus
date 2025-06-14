use parking_lot::RwLock;
use serde::Deserialize;
use std::{
    collections::HashMap,
    sync::Arc,
};

/// Single backend entry
#[derive(Debug, Deserialize, Clone)]
pub struct ServiceEntry {
    pub name: String,
    pub url: String,
}

/// Thread-safe registry mapping service names → backend URL list, with round-robin
#[derive(Clone)]
pub struct BackendRegistry {
    services: Arc<RwLock<HashMap<String, Vec<String>>>>,
    indices: Arc<RwLock<HashMap<String, usize>>>,
}

impl BackendRegistry {
    pub fn new() -> Self {
        Self {
            services: Arc::new(RwLock::new(HashMap::new())),
            indices: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a new backend URL under `name`
    pub fn register(&self, name: &str, url: &str) {
        let mut map = self.services.write();
        map.entry(name.to_string())
            .or_default()
            .push(url.to_string());
    }

    /// Remove (exact match) a backend URL
    pub fn deregister(&self, name: &str, url: &str) {
        if let Some(vec) = self.services.write().get_mut(name) {
            vec.retain(|u| u != url);
        }
    }

    /// Pick one backend URL (round-robin)
    pub fn pick_one(&self, name: &str) -> Option<String> {
        let services = self.services.write();
        let backends = services.get(name)?;
        let mut idx_map = self.indices.write();
        let ctr = idx_map.entry(name.to_string()).or_insert(0);
        let url = backends.get(*ctr % backends.len()).cloned();
        *ctr = (*ctr + 1) % backends.len();
        url
    }

    /// List all URLs under `name`
    pub fn list(&self, name: &str) -> Vec<String> {
        self.services.read().get(name).cloned().unwrap_or_default()
    }
}
