use super::ScanContext;
use serde_json::{json, Value};
use std::time::Duration;

/// Analyze the HTTP/HTTPS service: headers, security headers, cookies,
/// methods, redirects and response behaviour.
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

    // 1. Check http -> https redirect if target is https and port 80 reachable.
    if base.scheme() == "https" {
        if let Some(host) = base.host_str() {
            let mut http_url = base.clone();
            http_url.set_scheme("http").ok();
            http_url.set_port(Some(80)).ok();
            let http_url = http_url.to_string();
            if let Ok(Ok(resp)) = tokio::time::timeout(timeout, ctx.client.get(&http_url).send()).await {
                let status = resp.status().as_u16();
                let location = resp
                    .headers()
                    .get("location")
                    .map(|v| v.to_str().unwrap_or_default().to_string());
                if let Some(loc) = location {
                    ctx.emitter
                        .result(json!({
                            "check": "http_to_https_redirect",
                            "url": http_url,
                            "status": status,
                            "redirects_to": loc,
                            "secure": loc.starts_with("https://"),
                        }))
                        .await;
                }
            }
        }
    }

    // 2. Full GET analysis.
    let started_at = std::time::Instant::now();
    let resp = match tokio::time::timeout(timeout, ctx.client.get(base.clone()).send()).await {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => {
            ctx.emitter.event("error", format!("request failed: {e}")).await;
            ctx.emitter.complete(started.elapsed().as_millis()).await;
            return;
        }
        Err(_) => {
            ctx.emitter.event("error", "request timed out").await;
            ctx.emitter.complete(started.elapsed().as_millis()).await;
            return;
        }
    };
    let latency_ms = started_at.elapsed().as_millis();
    let status = resp.status().as_u16();
    let final_url = resp.url().to_string();
    let version = format!("{:?}", resp.version());

    let mut headers: Vec<Value> = Vec::new();
    for (k, v) in resp.headers().iter() {
        headers.push(json!({
            "name": k.as_str(),
            "value": v.to_str().unwrap_or_default(),
        }));
    }

    // 3. Security headers check.
    let security_checks = vec![
        ("strict-transport-security", "HSTS missing - client could be downgraded to HTTP", true),
        ("content-security-policy", "CSP missing - no Content Security Policy defined", true),
        ("x-frame-options", "X-Frame-Options missing - page may be clickjackable", true),
        ("x-content-type-options", "X-Content-Type-Options missing - MIME sniffing possible", true),
        ("x-xss-protection", "X-XSS-Protection missing", false),
        ("referrer-policy", "Referrer-Policy missing", false),
        ("permissions-policy", "Permissions-Policy missing", false),
    ];
    let mut security: Vec<Value> = Vec::new();
    for (name, msg, recommended) in security_checks {
        let present = resp.headers().contains_key(name);
        security.push(json!({
            "header": name,
            "present": present,
            "note": if present { "present" } else { msg },
            "recommended": recommended,
        }));
    }

    // 4. Cookies.
    let mut cookies: Vec<Value> = Vec::new();
    for (k, v) in resp.headers().iter() {
        if k.as_str().eq_ignore_ascii_case("set-cookie") {
            let value = v.to_str().unwrap_or_default();
            let name = value.split('=').next().unwrap_or("").to_string();
            cookies.push(json!({
                "raw": value,
                "name": name,
                "secure": value.to_lowercase().contains("secure"),
                "httponly": value.to_lowercase().contains("httponly"),
                "samesite": value.to_lowercase().contains("samesite"),
            }));
        }
    }

    // 5. HTTP methods (OPTIONS + common verbs).
    let mut methods: Vec<Value> = Vec::new();
    if let Ok(Ok(opt)) = tokio::time::timeout(timeout, ctx.client.request(reqwest::Method::OPTIONS, base.clone()).send()).await {
        methods.push(json!({
            "method": "OPTIONS",
            "status": opt.status().as_u16(),
            "allow": opt.headers().get("allow").map(|v| v.to_str().unwrap_or_default()),
        }));
    }
    let mut probe_body = String::new();
    for method in ["PUT", "DELETE", "PATCH", "TRACE"] {
        let m = reqwest::Method::from_bytes(method.as_bytes()).unwrap();
        if let Ok(Ok(r)) = tokio::time::timeout(timeout, ctx.client.request(m.clone(), base.clone()).send()).await {
            let s = r.status().as_u16();
            if s < 400 {
                methods.push(json!({ "method": method, "status": s, "note": "accepted" }));
                if s == 200 || s == 201 || s == 204 {
                    probe_body.push_str(&format!("{method}:allowed;"));
                }
            }
        }
    }

    // 6. Version / server details.
    let server = resp
        .headers()
        .get("server")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let powered = resp
        .headers()
        .get("x-powered-by")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    ctx.emitter
        .result(json!({
            "target": base.to_string(),
            "final_url": final_url,
            "status": status,
            "http_version": version,
            "latency_ms": latency_ms,
            "server": server,
            "x_powered_by": powered,
            "headers": headers,
            "security_headers": security,
            "cookies": cookies,
            "methods": methods,
            "method_probe": if probe_body.is_empty() { None } else { Some(probe_body) },
        }))
        .await;
    ctx.emitter.complete(started.elapsed().as_millis()).await;
}
