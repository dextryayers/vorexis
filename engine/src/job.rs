use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct JobSpec {
    pub target: String,
    #[serde(default)]
    pub modules: Vec<String>,
    #[serde(default = "default_concurrency")]
    pub concurrency: usize,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    #[serde(default)]
    pub options: HashMap<String, String>,
    #[serde(default)]
    pub wordlists: HashMap<String, String>,
}

impl JobSpec {
    pub fn option(&self, key: &str, default: &str) -> String {
        self.options
            .get(key)
            .cloned()
            .unwrap_or_else(|| default.to_string())
    }

    pub fn option_bool(&self, key: &str, default: bool) -> bool {
        self.options
            .get(key)
            .map(|v| v == "true" || v == "1" || v == "yes")
            .unwrap_or(default)
    }

    pub fn option_usize(&self, key: &str, default: usize) -> usize {
        self.options
            .get(key)
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    }
}

fn default_concurrency() -> usize {
    50
}

fn default_timeout() -> u64 {
    8
}

pub const MODULES: &[&str] = &[
    "port",
    "directory",
    "subdomain",
    "dns",
    "crawler",
    "parser",
    "http",
    "tls",
    "fuzzer",
    "waf",
    "fingerprint",
    "tech",
];
