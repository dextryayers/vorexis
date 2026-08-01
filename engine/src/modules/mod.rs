pub mod crawler;
pub mod directory;
pub mod dns;
pub mod fingerprint;
pub mod fuzzer;
pub mod http;
pub mod parser;
pub mod port;
pub mod subdomain;
pub mod tech;
pub mod tls;
pub mod waf;

use crate::core::Cache;
use crate::job::JobSpec;
use crate::output::OutputEvent;
use futures::future::BoxFuture;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct Emitter {
    tx: mpsc::Sender<OutputEvent>,
    module: String,
}

impl Emitter {
    pub fn new(tx: mpsc::Sender<OutputEvent>, module: &str) -> Self {
        Emitter {
            tx,
            module: module.to_string(),
        }
    }

    pub async fn event(&self, level: &str, message: impl Into<String>) {
        let _ = self
            .tx
            .send(OutputEvent::event(&self.module, level, message))
            .await;
    }

    pub async fn progress(&self, current: u64, total: u64) {
        let _ = self
            .tx
            .send(OutputEvent::progress(&self.module, current, total))
            .await;
    }

    pub async fn result(&self, data: serde_json::Value) {
        let _ = self
            .tx
            .send(OutputEvent::result(&self.module, data))
            .await;
    }

    pub async fn complete(&self, duration_ms: u128) {
        let _ = self
            .tx
            .send(OutputEvent::complete(&self.module, duration_ms))
            .await;
    }
}

pub struct ScanContext {
    pub job: JobSpec,
    pub emitter: Emitter,
    pub client: reqwest::Client,
    pub cache: Arc<Cache>,
    pub started: Instant,
    pub deadline: Option<Instant>,
}

impl ScanContext {
    pub fn base_url(&self) -> Option<url::Url> {
        normalize_url(&self.job.target)
    }

    pub fn host(&self) -> String {
        self.base_url()
            .and_then(|u| u.host_str().map(|h| h.to_string()))
            .unwrap_or_else(|| self.job.target.clone())
    }

    pub fn timeout(&self) -> u64 {
        self.job.timeout
    }

    /// True when the global job deadline has been reached. Modules that loop
    /// should stop doing new work and wind down gracefully.
    pub fn expired(&self) -> bool {
        match self.deadline {
            Some(d) => Instant::now() >= d,
            None => false,
        }
    }
}

/// Try to normalize an arbitrary target string into a base URL.
pub fn normalize_url(target: &str) -> Option<url::Url> {
    let t = target.trim();
    if t.is_empty() {
        return None;
    }
    let with_scheme = if t.contains("://") {
        t.to_string()
    } else {
        format!("https://{}", t)
    };
    url::Url::parse(&with_scheme).ok()
}

pub async fn run(job: JobSpec, tx: mpsc::Sender<OutputEvent>) {
    let request_timeout = std::time::Duration::from_secs(job.timeout.max(1));
    // TLS verification is disabled by default (scanner behavior) but can be
    // re-enabled per job via options.verify_tls = true.
    let verify_tls = job.option_bool("verify_tls", false);
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (aipentest-engine)")
        .redirect(reqwest::redirect::Policy::limited(5))
        .connect_timeout(request_timeout.min(std::time::Duration::from_secs(5)))
        .timeout(request_timeout)
        .danger_accept_invalid_certs(!verify_tls)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let deadline_secs = job.option_usize("deadline_secs", 780);
    let deadline = (deadline_secs > 0).then(|| {
        Instant::now() + std::time::Duration::from_secs(deadline_secs as u64)
    });

    let cache = Arc::new(Cache::new(std::time::Duration::from_secs(60)));

    let mut handles = Vec::new();
    for m in &job.modules {
        let name = m.to_lowercase();
        let ctx = ScanContext {
            emitter: Emitter::new(tx.clone(), &name),
            job: job.clone(),
            client: client.clone(),
            cache: cache.clone(),
            started: Instant::now(),
            deadline,
        };
        let f: Option<BoxFuture<'_, ()>> = match name.as_str() {
            "port" | "port-scan" | "portscanner" => Some(Box::pin(async move {
                port::run(&ctx).await;
            })),
            "directory" | "dir" | "dirscan" => Some(Box::pin(async move {
                directory::run(&ctx).await;
            })),
            "subdomain" | "sub" => Some(Box::pin(async move {
                subdomain::run(&ctx).await;
            })),
            "dns" => Some(Box::pin(async move {
                dns::run(&ctx).await;
            })),
            "crawler" | "crawl" | "spider" => Some(Box::pin(async move {
                crawler::run(&ctx).await;
            })),
            "parser" | "parse" | "html" => Some(Box::pin(async move {
                parser::run(&ctx).await;
            })),
            "http" | "https" => Some(Box::pin(async move {
                http::run(&ctx).await;
            })),
            "tls" | "ssl" | "ssl-tls" => Some(Box::pin(async move {
                tls::run(&ctx).await;
            })),
            "fuzzer" | "fuzz" => Some(Box::pin(async move {
                fuzzer::run(&ctx).await;
            })),
            "waf" | "waf-detect" => Some(Box::pin(async move {
                waf::run(&ctx).await;
            })),
            "fingerprint" | "finger" => Some(Box::pin(async move {
                fingerprint::run(&ctx).await;
            })),
            "tech" | "tech-detect" | "technology" => Some(Box::pin(async move {
                tech::run(&ctx).await;
            })),
            _ => None,
        };
        match f {
            Some(task) => {
                handles.push(tokio::spawn(async move { task.await }));
            }
            None => {
                let _ = tx
                    .send(OutputEvent::event(m, "warn", format!("unknown module: {m}")))
                    .await;
            }
        }
    }
    for h in handles {
        let _ = h.await;
    }
}
