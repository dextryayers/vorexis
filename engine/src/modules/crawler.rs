use super::ScanContext;
use crate::core::WorkerPool;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct CrawlItem {
    url: url::Url,
    depth: usize,
}

struct CrawlOutcome {
    page: Option<Value>,
    links: Vec<(url::Url, usize)>,
}

/// Read the response body, capping at `cap` bytes to avoid memory blowups.
async fn read_body_capped(resp: &mut reqwest::Response, cap: usize) -> (String, bool) {
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
    (String::from_utf8_lossy(&buf).into_owned(), truncated)
}

/// Parallel BFS web crawler: fetches pages, extracts links, stays on the same
/// host. Pages at each depth level are fetched concurrently.
pub async fn run(ctx: &ScanContext) {
    let started = std::time::Instant::now();
    let base = match ctx.base_url() {
        Some(b) => b,
        None => {
            ctx.emitter.event("error", "invalid target URL").await;
            return;
        }
    };
    let host = base.host_str().unwrap_or_default().to_string();
    let max_depth = ctx.job.option_usize("depth", 3);
    let max_pages = ctx.job.option_usize("max_pages", 200);
    let max_body = ctx.job.option_usize("max_body", 2_000_000);
    let concurrency = ctx.job.option_usize("crawl_concurrency", ctx.job.concurrency.max(1));
    let timeout = Duration::from_secs(ctx.timeout().max(1));

    ctx.emitter
        .event("info", format!("crawling {base} (depth={max_depth}, max_pages={max_pages}, concurrency={concurrency})"))
        .await;

    let visited = Arc::new(Mutex::new(HashSet::<String>::new()));
    let count = Arc::new(AtomicU64::new(0));
    let deadline = ctx.deadline;
    let client = ctx.client.clone();
    let host_for_closure = host.clone();

    visited.lock().unwrap().insert(base.to_string());
    let pool = WorkerPool::new(concurrency);
    let mut frontier: Vec<CrawlItem> = vec![CrawlItem { url: base.clone(), depth: 0 }];
    let mut pages: Vec<Value> = Vec::new();

    while !frontier.is_empty()
        && (count.load(Ordering::Relaxed) as usize) < max_pages
        && !ctx.expired()
    {
        // Bound the size of a single level so work stays proportional.
        frontier.truncate(max_pages);
        let level_items = std::mem::take(&mut frontier);
        let next = ctx.emitter.clone();

        // Clone per iteration: the pool closure needs a 'static capture, but
        // the loop runs multiple times.
        let client = client.clone();
        let visited = visited.clone();
        let host = host_for_closure.clone();

        let outcomes: Vec<CrawlOutcome> = pool
            .run(level_items, move |item| {
                let client = client.clone();
                let visited = visited.clone();
                let host = host.clone();
                async move {
                    if deadline.is_some_and(|d| std::time::Instant::now() >= d) {
                        return CrawlOutcome { page: None, links: Vec::new() };
                    }
                    let mut resp = match tokio::time::timeout(timeout, client.get(item.url.clone()).send()).await {
                        Ok(Ok(r)) => r,
                        _ => {
                            return CrawlOutcome { page: None, links: Vec::new() };
                        }
                    };
                    let status = resp.status().as_u16();
                    let headers: Vec<(String, String)> = resp
                        .headers()
                        .iter()
                        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or_default().to_string()))
                        .collect();
                    let content_type = resp
                        .headers()
                        .get("content-type")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("")
                        .to_string();
                    let (text, truncated) = read_body_capped(&mut resp, max_body).await;
                    let page_title = title_of(&text);

                    let page = json!({
                        "url": item.url.to_string(),
                        "depth": item.depth,
                        "status": status,
                        "title": page_title,
                        "content_type": content_type,
                        "size": text.len(),
                        "truncated": truncated,
                        "headers": headers,
                    });

                    // Extract links for the next level (scoped so the parsed
                    // DOM is dropped before any await — keeps the future Send).
                    let mut links: Vec<(url::Url, usize)> = Vec::new();
                    if content_type.starts_with("text/html") {
                        let doc = scraper::Html::parse_document(&text);
                        let selector =
                            scraper::Selector::parse("a[href], link[href], script[src], img[src]").unwrap();
                        for node in doc.select(&selector) {
                            let attr = if node.value().attr("href").is_some() {
                                node.value().attr("href")
                            } else {
                                node.value().attr("src")
                            };
                            let Some(href) = attr else { continue };
                            let candidate = if let Ok(parsed) = url::Url::parse(href) {
                                Some(parsed)
                            } else {
                                item.url.join(href).ok()
                            };
                            let Some(mut parsed) = candidate else { continue };
                            if parsed.host_str() != Some(&host) {
                                continue;
                            }
                            parsed.set_fragment(None);
                            let key = parsed.to_string();
                            if visited.lock().unwrap().insert(key) {
                                links.push((parsed, item.depth + 1));
                            }
                        }
                    }

                    CrawlOutcome { page: Some(page), links }
                }
            })
            .await;

        for o in outcomes {
            if let Some(page) = o.page {
                count.fetch_add(1, Ordering::Relaxed);
                next
                    .result(json!({
                        "url": page["url"],
                        "depth": page["depth"],
                        "status": page["status"],
                        "title": page["title"],
                        "content_type": page["content_type"],
                        "size": page["size"],
                    }))
                    .await;
                pages.push(page);
            }
            for (url, depth) in o.links {
                if depth <= max_depth
                    && (count.load(Ordering::Relaxed) as usize) < max_pages
                {
                    frontier.push(CrawlItem { url, depth });
                }
            }
        }
        ctx.emitter
            .progress(count.load(Ordering::Relaxed), max_pages as u64)
            .await;
    }

    ctx.emitter
        .result(json!({
            "target": base.to_string(),
            "pages_crawled": pages.len(),
            "pages": pages,
        }))
        .await;
    ctx.emitter.complete(started.elapsed().as_millis()).await;
}

fn title_of(html: &str) -> Option<String> {
    let doc = scraper::Html::parse_document(html);
    let sel = scraper::Selector::parse("title").ok()?;
    doc.select(&sel).next().map(|n| n.text().collect::<String>().trim().to_string())
}
