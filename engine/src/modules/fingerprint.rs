use super::ScanContext;
use crate::core::WorkerPool;
use serde_json::{json, Value};
use std::time::Duration;

struct ProbeOutcome {
    path: String,
    status: u16,
    hints: Vec<String>,
}

/// Server & software fingerprinting based on response headers, cookies and
/// default page patterns. All path probes run in parallel.
pub async fn run(ctx: &ScanContext) {
    let started = std::time::Instant::now();
    let base = match ctx.base_url() {
        Some(b) => b,
        None => {
            ctx.emitter.event("error", "invalid target URL").await;
            return;
        }
    };
    let timeout = Duration::from_secs(ctx.timeout().max(1));

    // Probe common fingerprints paths.
    let probes = [
        "/", "/favicon.ico", "/robots.txt", "/wp-login.php", "/wp-json/", "/.git/HEAD",
        "/server-status", "/actuator", "/graphql", "/.env", "/adminer.php",
    ];

    let pool = WorkerPool::new(ctx.job.concurrency.max(1));
    let client = ctx.client.clone();
    let deadline = ctx.deadline;
    let target = base.to_string();

    let outcomes: Vec<(u16, String, Vec<(String, String)>)> = pool
        .run(probes.to_vec(), move |path| {
            let client = client.clone();
            let base = base.clone();
            async move {
                if deadline.is_some_and(|d| std::time::Instant::now() >= d) {
                    return (0, String::new(), Vec::new());
                }
                let mut u = base.clone();
                u.set_path(&path);
                if let Ok(Ok(resp)) = tokio::time::timeout(timeout, client.get(u.clone()).send()).await {
                    let status = resp.status().as_u16();
                    let headers: Vec<(String, String)> = resp
                        .headers()
                        .iter()
                        .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or_default().to_string()))
                        .collect();
                    let body = match resp.text().await {
                        Ok(b) => b,
                        Err(_) => String::new(),
                    };
                    (status, body, headers)
                } else {
                    (0, String::new(), Vec::new())
                }
            }
        })
        .await;

    let mut signals: Vec<Value> = Vec::new();
    let mut first_response: Option<(u16, String, Vec<(String, String)>)> = None;

    for (path, (status, body, headers)) in probes.iter().zip(outcomes.into_iter()) {
        if status == 0 {
            continue;
        }
        if first_response.is_none() && *path == "/" {
            first_response = Some((status, body.clone(), headers.clone()));
        }

        let mut hints: Vec<String> = Vec::new();
        let server = headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("server"))
            .map(|(_, v)| v.clone())
            .unwrap_or_default();
        if !server.is_empty() {
            hints.push(format!("server header: {server}"));
        }
        let powered = headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("x-powered-by"))
            .map(|(_, v)| v.clone())
            .unwrap_or_default();
        if !powered.is_empty() {
            hints.push(format!("x-powered-by: {powered}"));
        }

        // Cookie-based hints.
        for (k, v) in &headers {
            if k.eq_ignore_ascii_case("set-cookie") {
                let v_low = v.to_lowercase();
                if v_low.contains("phpsessid") { hints.push("PHP (PHPSESSID cookie)".into()); }
                if v_low.contains("jsessionid") { hints.push("Java (JSESSIONID cookie)".into()); }
                if v_low.contains("asp.net_sessionid") || v_low.contains("aspnetsessionid") { hints.push("ASP.NET".into()); }
                if v_low.contains("cfid") || v_low.contains("cftoken") { hints.push("ColdFusion".into()); }
                if v_low.contains("csrftoken") || v_low.contains("_ga") { hints.push("Django/Google Analytics".into()); }
            }
        }

        // Body patterns.
        let body_low = body.to_lowercase();
        let patterns: &[(&str, &str)] = &[
            ("wordpress", "/wp-content/"),
            ("wordpress", "/wp-includes/"),
            ("wordpress", "wordpress.org"),
            ("drupal", "drupal"),
            ("joomla", "joomla"),
            ("phpmyadmin", "phpmyadmin"),
            ("tomcat", "apache tomcat"),
            ("tomcat", "/manager/html"),
            ("jboss", "jboss"),
            ("weblogic", "weblogic"),
            ("jenkins", "jenkins"),
            ("grafana", "grafana"),
            ("kibana", "kibana"),
            ("harbor", "harbor"),
            ("gitlab", "gitlab"),
            ("swagger-ui", "swagger-ui"),
            ("graphiql", "graphiql"),
            ("netlify", "netlify"),
            ("vercel", "__vercel"),
            ("github-pages", "github.com/"),
            ("cloudflare-pages", "pages.dev"),
        ];
        for (name, pat) in patterns {
            if body_low.contains(pat) {
                hints.push(name.to_string());
            }
        }

        if !hints.is_empty() {
            signals.push(json!({ "path": path, "status": status, "hints": hints }));
        }
    }

    // Distilled fingerprint from the main page.
    let mut fingerprint: Vec<String> = Vec::new();
    if let Some((status, body, headers)) = &first_response {
        let server = headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("server"))
            .map(|(_, v)| v.clone())
            .unwrap_or_default();
        let powered = headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("x-powered-by"))
            .map(|(_, v)| v.clone())
            .unwrap_or_default();
        if !server.is_empty() {
            fingerprint.push(server);
        }
        if !powered.is_empty() {
            fingerprint.push(powered);
        }
        let body_low = body.to_lowercase();
        if body_low.contains("nginx") { fingerprint.push("nginx".into()); }
        if body_low.contains("apache") && !body_low.contains("nginx") { fingerprint.push("apache".into()); }
        if body_low.contains("iis") || body_low.contains("microsoft-iis") { fingerprint.push("iis".into()); }
        if body_low.contains("wordpress") || body_low.contains("wp-content") { fingerprint.push("wordpress".into()); }
        if body_low.contains("laravel") { fingerprint.push("laravel".into()); }
        let _ = status;
    }

    ctx.emitter
        .result(json!({
            "target": target,
            "fingerprint": fingerprint,
            "signals": signals,
        }))
        .await;
    ctx.emitter.complete(started.elapsed().as_millis()).await;
}
