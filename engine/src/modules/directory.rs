use super::ScanContext;
use crate::core::WorkerPool;
use crate::wordlists::{load_wordlist, DIR_WORDLIST};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// djb2 hash over the (capped) response body — used for soft-404 detection.
fn hash_body(bytes: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in bytes {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Read up to `cap` bytes of the body and return (bytes, total_read, truncated).
async fn read_body_capped(resp: &mut reqwest::Response, cap: usize) -> (Vec<u8>, bool) {
    let mut buf: Vec<u8> = Vec::new();
    let mut truncated = false;
    loop {
        match resp.chunk().await {
            Ok(Some(c)) => {
                buf.extend_from_slice(&c);
                if buf.len() >= cap {
                    truncated = true;
                    break;
                }
            }
            Ok(None) => break,
            Err(_) => break,
        }
    }
    (buf, truncated)
}

/// A body whose size/hash matches a random nonexistent path baseline is a
/// soft-404: the server returns 200/302 for anything.
fn is_soft_404(status: u16, size: u64, hash: u64, baselines: &[(u16, u64, u64)]) -> bool {
    for &(bs, bsize, bhash) in baselines {
        if bs != status {
            continue;
        }
        if hash != 0 && hash == bhash {
            return true;
        }
        if bsize > 0 && size > 0 {
            let diff = (size as i64 - bsize as i64).abs();
            if diff * 100 <= bsize as i64 * 15 {
                return true;
            }
        }
    }
    false
}

pub async fn run(ctx: &ScanContext) {
    let started = std::time::Instant::now();
    let base = match ctx.base_url() {
        Some(b) => b,
        None => {
            ctx.emitter.event("error", "invalid target URL").await;
            return;
        }
    };
    let wordlist_path = ctx.job.wordlists.get("directory").cloned();
    let words = load_wordlist(&wordlist_path, DIR_WORDLIST);
    let exclude_codes: Vec<u16> = ctx
        .job
        .option("exclude_status", "404")
        .split(',')
        .filter_map(|c| c.trim().parse().ok())
        .collect();
    let concurrency = ctx.job.concurrency.max(1);
    let max_hash_bytes = ctx.job.option_usize("hash_bytes", 524_288);
    let baseline_count = ctx.job.option_usize("baseline_probes", 8);

    ctx.emitter
        .event("info", format!("brute-forcing {} paths on {}", words.len(), base))
        .await;

    let pool = WorkerPool::new(concurrency);
    let client = ctx.client.clone();
    let total = words.len() as u64;
    let found: Arc<Mutex<Vec<Value>>> = Arc::new(Mutex::new(Vec::new()));
    let checked = Arc::new(AtomicU64::new(0));
    let cache = ctx.cache.clone();
    let deadline = ctx.deadline;
    let max_hash_bytes_closure = max_hash_bytes;

    // Establish a soft-404 baseline by requesting random nonexistent paths.
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut baselines: Vec<(u16, u64, u64)> = Vec::new();
    for i in 0..baseline_count {
        if ctx.expired() {
            break;
        }
        let mut u = base.clone();
        u.set_path(&format!("/zq{nonce:x}-{i}-{}.html", nonce.wrapping_add(i as u128 + 7)));
        if let Ok(mut resp) = client.get(u).send().await {
            let status = resp.status().as_u16();
            let content_length = resp.content_length().unwrap_or(0);
            let (buf, _) = read_body_capped(&mut resp, max_hash_bytes).await;
            let size = content_length.max(buf.len() as u64);
            if status != 0 {
                baselines.push((status, size, hash_body(&buf)));
            }
        }
    }
    let baselines = baselines.clone();

    let mut urls = Vec::with_capacity(words.len());
    for w in &words {
        let mut u = base.clone();
        let clean = w.trim_start_matches('/');
        u.set_path(&format!("/{}", clean));
        urls.push((u.to_string(), w.clone()));
    }

    let found_for_closure = found.clone();
    let checked_for_closure = checked.clone();
    let soft404_count = baselines.len();

    // Periodic progress updates while the pool drains.
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

    pool.run(urls, move |(url, word)| {
        let client = client.clone();
        let found = found_for_closure.clone();
        let exclude_codes = exclude_codes.clone();
        let checked = checked_for_closure.clone();
        let cache = cache.clone();
        let deadline = deadline;
        let max_hash_bytes = max_hash_bytes_closure;
        let baselines = baselines.clone();
        async move {
            checked.fetch_add(1, Ordering::Relaxed);
            if deadline.is_some_and(|d| std::time::Instant::now() >= d) {
                return;
            }

            let cache_key = format!("http:{url}");
            let cached = cache.get(&cache_key);

            let entry: Option<Value> = if let Some(raw) = cached {
                serde_json::from_str::<Value>(&raw).ok()
            } else {
                match client.get(&url).send().await {
                    Ok(mut resp) => {
                        let status = resp.status().as_u16();
                        let content_length = resp.content_length().unwrap_or(0);
                        let (buf, _) = read_body_capped(&mut resp, max_hash_bytes).await;
                        let size = content_length.max(buf.len() as u64);
                        let redirect = resp
                            .headers()
                            .get("location")
                            .map(|v| v.to_str().unwrap_or_default().to_string());
                        let server = resp
                            .headers()
                            .get("server")
                            .map(|v| v.to_str().unwrap_or_default().to_string());
                        let hash = hash_body(&buf);
                        let soft = is_soft_404(status, size, hash, &baselines);
                        let hit = json!({
                            "url": url,
                            "word": word,
                            "status": status,
                            "size": size,
                            "redirect": redirect.unwrap_or_default(),
                            "server": server.unwrap_or_default(),
                            "hash": hash,
                            "soft_404": soft,
                        });
                        cache.set(&cache_key, hit.to_string());
                        Some(hit)
                    }
                    Err(_) => None,
                }
            };

            if let Some(hit) = entry {
                let status = hit["status"].as_u64().unwrap_or(0) as u16;
                let soft = hit["soft_404"] == json!(true);
                if !exclude_codes.contains(&status) && !soft {
                    found.lock().unwrap().push(hit);
                }
            }
        }
    })
    .await;
    let _ = watcher.await;

    let found_list = found.lock().unwrap().clone();
    ctx.emitter
        .result(json!({
            "target": base.to_string(),
            "paths_tested": total,
            "soft404_baselines": soft404_count,
            "found": found_list,
        }))
        .await;
    ctx.emitter.complete(started.elapsed().as_millis()).await;
}
