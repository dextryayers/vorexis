use super::ScanContext;
use crate::core::WorkerPool;
use crate::wordlists::{load_wordlist, SUBDOMAIN_WORDLIST};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub async fn run(ctx: &ScanContext) {
    let started = std::time::Instant::now();
    let host = ctx.host();
    let words = load_wordlist(
        &ctx.job.wordlists.get("subdomain").cloned(),
        SUBDOMAIN_WORDLIST,
    );

    // Only meaningful when the target is a bare domain.
    if host.split('.').count() < 2 || host.starts_with("www.") {
        ctx.emitter
            .event("info", format!("target {} is not a bare domain; using root anyway", host))
            .await;
    }

    let resolver = match hickory_resolver::TokioResolver::builder_tokio() {
        Ok(builder) => match builder.build() {
            Ok(r) => r,
            Err(e) => {
                ctx.emitter.event("error", format!("failed to init resolver: {e}")).await;
                return;
            }
        },
        Err(e) => {
            ctx.emitter.event("error", format!("failed to init resolver: {e}")).await;
            return;
        }
    };
    let resolver = Arc::new(resolver);
    let timeout = Duration::from_secs(ctx.timeout().max(1));
    let concurrency = ctx.job.concurrency.max(1);

    let pool = WorkerPool::new(concurrency);
    let found: Arc<Mutex<Vec<Value>>> = Arc::new(Mutex::new(Vec::new()));
    let checked = Arc::new(AtomicU64::new(0));
    let total = words.len() as u64;
    let host_for_closure = host.clone();
    let found_for_closure = found.clone();
    let deadline = ctx.deadline;

    // Wildcard DNS detection: resolve a random nonexistent subdomain. If it
    // resolves, every name resolves and brute force would be meaningless.
    let wildcard_ips: Vec<String> = {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let probe = format!("zz{nonce:x}.{host}");
        match tokio::time::timeout(timeout, resolver.lookup_ip(probe)).await {
            Ok(Ok(ips)) => ips.iter().map(|ip| ip.to_string()).collect(),
            _ => Vec::new(),
        }
    };
    let wildcard = !wildcard_ips.is_empty();
    if wildcard {
        ctx.emitter
            .event("warn", format!("wildcard DNS detected ({host}): {wildcard_ips:?}; results may be meaningless"))
            .await;
    }

    ctx.emitter
        .event("info", format!("enumerating {} subdomains of {host}", words.len()))
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

    pool.run(words, move |word| {
        let resolver = resolver.clone();
        let found = found_for_closure.clone();
        let checked = checked.clone();
        let host = host_for_closure.clone();
        let deadline = deadline;
        async move {
            checked.fetch_add(1, Ordering::Relaxed);
            if deadline.is_some_and(|d| std::time::Instant::now() >= d) {
                return;
            }
            let fqdn = if word == "@" {
                host.clone()
            } else {
                format!("{}.{}", word, host)
            };
            match tokio::time::timeout(timeout, resolver.lookup_ip(fqdn.clone())).await {
                Ok(Ok(ips)) => {
                    let mut addresses = Vec::new();
                    for ip in ips.iter() {
                        addresses.push(ip.to_string());
                    }
                    found.lock().unwrap().push(json!({
                        "subdomain": fqdn,
                        "ips": addresses,
                        "alive": true,
                    }));
                }
                _ => {}
            }
        }
    })
    .await;
    let _ = watcher.await;

    let list = found.lock().unwrap().clone();
    ctx.emitter
        .result(json!({
            "domain": host,
            "subdomains_tested": total,
            "wildcard_dns": wildcard,
            "wildcard_ips": wildcard_ips,
            "resolved": list,
        }))
        .await;
    ctx.emitter.complete(started.elapsed().as_millis()).await;
}
