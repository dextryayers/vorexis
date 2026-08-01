use super::ScanContext;
use crate::core::WorkerPool;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const EXTENSIONS: &[&str] = &[
    "", ".php", ".asp", ".aspx", ".jsp", ".do", ".action", ".json", ".xml", ".txt", ".bak",
    ".old", ".orig", ".save", ".swp", ".tmp", ".tar", ".zip", ".gz", ".sql", ".log", ".conf",
    ".config", ".inc", ".ini", ".env", ".yml", ".yaml", ".pem", ".key", ".crt", ".p12",
];

const PAYLOADS: &[&str] = &[
    "..%2f", "../", "..%5c", "....//", "%2e%2e%2f", "..;/", "%00", "%20", "~", "*", "a%09",
    "::$DATA", "..%00/", "%0a", "%0d%0a",
];

/// Fuzzer: path variants with extensions + a small set of traversal payloads.
pub async fn run(ctx: &ScanContext) {
    let started = std::time::Instant::now();
    let base = match ctx.base_url() {
        Some(b) => b,
        None => {
            ctx.emitter.event("error", "invalid target URL").await;
            return;
        }
    };
    let base_words: Vec<String> = ctx
        .job
        .option("words", "admin,login,index,backup,config,upload,api")
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let extensions: Vec<&str> = if ctx.job.option_bool("all_extensions", true) {
        EXTENSIONS.to_vec()
    } else {
        vec![""]
    };

    let mut urls: Vec<String> = Vec::new();
    for w in &base_words {
        for ext in &extensions {
            let mut u = base.clone();
            u.set_path(&format!("/{w}{ext}"));
            urls.push(u.to_string());
        }
    }
    // Traversal payloads must be sent raw (no percent-escaping of the URL
    // parser). Build the URL string from the origin so payloads like
    // "..%2f" or "%00" reach the server untouched.
    let origin = format!("{}://{}", base.scheme(), base.authority());
    for p in PAYLOADS {
        let clean = p.trim_start_matches('/');
        if let Ok(u) = url::Url::parse(&format!("{origin}/{clean}")) {
            urls.push(u.to_string());
        }
    }

    let pool = WorkerPool::new(ctx.job.concurrency.max(1));
    let client = ctx.client.clone();
    let cache = ctx.cache.clone();
    let deadline = ctx.deadline;
    let total = urls.len() as u64;
    let results: Arc<Mutex<Vec<Value>>> = Arc::new(Mutex::new(Vec::new()));
    let checked = Arc::new(AtomicU64::new(0));
    let results_for_closure = results.clone();
    let checked_for_closure = checked.clone();

    ctx.emitter
        .event("info", format!("fuzzing {} URL variants", urls.len()))
        .await;

    let watcher = {
        let watch_checked = checked.clone();
        let watch_total = total;
        let watch_emitter = ctx.emitter.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(400)).await;
                let c = watch_checked.load(Ordering::Relaxed);
                watch_emitter.progress(c, watch_total).await;
                if c >= watch_total {
                    break;
                }
            }
        })
    };

    pool.run(urls, move |url| {
        let client = client.clone();
        let results = results_for_closure.clone();
        let checked = checked_for_closure.clone();
        let cache = cache.clone();
        let deadline = deadline;
        async move {
            checked.fetch_add(1, Ordering::Relaxed);
            if deadline.is_some_and(|d| std::time::Instant::now() >= d) {
                return;
            }
            let started_at = std::time::Instant::now();
            let cache_key = format!("http:{url}");

            let status;
            let size;
            let redirect;
            let cached = cache.get(&cache_key);
            if let Some(raw) = cached {
                if let Ok(v) = serde_json::from_str::<Value>(&raw) {
                    status = v["status"].as_u64().unwrap_or(0) as u16;
                    size = v["size"].as_u64().unwrap_or(0);
                    redirect = v["redirect"].as_str().unwrap_or("").to_string();
                } else {
                    return;
                }
            } else if let Ok(resp) = client.get(&url).send().await {
                status = resp.status().as_u16();
                size = resp.content_length().unwrap_or(0);
                redirect = resp
                    .headers()
                    .get("location")
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                cache.set(
                    &cache_key,
                    json!({ "status": status, "size": size, "redirect": redirect }).to_string(),
                );
            } else {
                return;
            }

            // Report anything not a clean 404.
            if status != 404 && status != 0 {
                results.lock().unwrap().push(json!({
                    "url": url,
                    "status": status,
                    "size": size,
                    "redirect": if redirect.is_empty() { None } else { Some(redirect) },
                    "latency_ms": started_at.elapsed().as_millis(),
                    "filtered": is_filtered(status, size),
                }));
            }
        }
    })
    .await;
    let _ = watcher.await;

    let list = results.lock().unwrap().clone();
    ctx.emitter
        .result(json!({
            "target": base.to_string(),
            "variants_tested": total,
            "interesting": list,
        }))
        .await;
    ctx.emitter.complete(started.elapsed().as_millis()).await;
}

fn is_filtered(status: u16, size: u64) -> bool {
    // Rough heuristic: identical 403/200 responses with tiny sizes are often
    // soft-404 / firewall responses.
    status == 403 && size < 512
}
