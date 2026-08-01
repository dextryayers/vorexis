use super::ScanContext;
use regex::Regex;
use serde_json::{json, Value};
use std::sync::OnceLock;
use std::time::Duration;

struct TechRule {
    name: &'static str,
    kind: &'static str,
    re: Regex,
}

fn tech_rules() -> &'static Vec<TechRule> {
    static RULES: OnceLock<Vec<TechRule>> = OnceLock::new();
    RULES.get_or_init(|| {
        const PATTERNS: &[(&str, &str, &str)] = &[
            ("wordpress", "wp-content|wp-includes|wp-json", "cms"),
            ("woocommerce", "woocommerce", "ecommerce"),
            ("drupal", "drupal-settings|sites/default|/drupal", "cms"),
            ("joomla", "joomla|/media/system/", "cms"),
            ("ghost", "ghost", "cms"),
            ("squarespace", "squarespace", "cms"),
            ("wix", "wixstatic", "cms"),
            ("webflow", "webflow", "cms"),
            ("laravel", "laravel_session|csrf-token", "framework"),
            ("django", "csrftoken|django", "framework"),
            ("rails", "rails|_rails_env", "framework"),
            ("flask", "flask", "framework"),
            ("fastapi", "fastapi|swagger-ui", "framework"),
            ("express", "x-powered-by: express", "framework"),
            ("spring-boot", "spring|whitelabel error page", "framework"),
            ("symfony", "symfony", "framework"),
            ("codeigniter", "ci_session", "framework"),
            ("cakephp", "cakephp", "framework"),
            ("yii", "yii|_csrf", "framework"),
            ("asp.net", "__viewstate|__eventvalidation|asp.net", "framework"),
            ("struts", "struts", "framework"),
            ("next.js", "__next|next/dist", "framework"),
            ("nuxt", "__nuxt", "framework"),
            ("gatsby", "gatsby", "framework"),
            ("sveltekit", "svelte", "framework"),
            ("vite", "vite", "build-tool"),
            ("webpack", "webpack", "build-tool"),
            ("parcel", "parcel", "build-tool"),
            ("react", "react|__react", "frontend"),
            ("vue", "vue|createapp|__vue", "frontend"),
            ("angular", "ng-version|angular", "frontend"),
            ("jquery", "jquery", "frontend"),
            ("bootstrap", "bootstrap", "frontend"),
            ("tailwind", "tailwind|tw-", "frontend"),
            ("htmx", "htmx", "frontend"),
            ("alpine", "alpinejs|x-data", "frontend"),
            ("turbo", "turbo", "frontend"),
            ("turbolinks", "turbolinks", "frontend"),
            ("analytics-ga", "google-analytics|gtag|googletagmanager", "analytics"),
            ("analytics-hotjar", "hotjar", "analytics"),
            ("cloudflare-turnstile", "turnstile", "security"),
            ("recaptcha", "recaptcha", "security"),
            ("intercom", "intercom", "support"),
            ("stripe", "stripe", "payment"),
            ("paypal", "paypal", "payment"),
            ("shopify-payment", "shopify", "payment"),
            ("mailchimp", "mailchimp", "marketing"),
            ("hubspot", "hs-script|hubspot", "marketing"),
            ("cloudinary", "cloudinary", "cdn"),
            ("cloudflare-cdn", "cloudflare", "cdn"),
            ("akamai-cdn", "akamai", "cdn"),
            ("fastly-cdn", "fastly", "cdn"),
        ];
        PATTERNS
            .iter()
            .map(|(name, pat, kind)| TechRule {
                name,
                kind,
                re: Regex::new(pat).unwrap(),
            })
            .collect()
    })
}

/// Technology detection via header + content pattern matching.
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

    // Fetch the homepage; if it's an SPA, also fetch the main JS bundle hint.
    let (status, body, headers) = match tokio::time::timeout(timeout, ctx.client.get(base.clone()).send()).await {
        Ok(Ok(resp)) => {
            let s = resp.status().as_u16();
            let headers: Vec<(String, String)> = resp
                .headers()
                .iter()
                .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or_default().to_string()))
                .collect();
            match resp.text().await {
                Ok(b) => (s, b, headers),
                Err(_) => (s, String::new(), headers),
            }
        }
        _ => {
            ctx.emitter.event("error", "failed to fetch target").await;
            ctx.emitter.complete(started.elapsed().as_millis()).await;
            return;
        }
    };

    let body_low = body.to_lowercase();
    let mut detected: Vec<Value> = Vec::new();

    // Header-based signals.
    for (k, v) in &headers {
        let (k_low, v_low) = (k.to_lowercase(), v.to_lowercase());
        if k_low == "server" {
            if v_low.contains("nginx") { detected.push(json!({"name":"nginx","type":"web-server","source":"header"})); }
            if v_low.contains("apache") { detected.push(json!({"name":"apache","type":"web-server","source":"header"})); }
            if v_low.contains("iis") { detected.push(json!({"name":"iis","type":"web-server","source":"header"})); }
            if v_low.contains("cloudflare") { detected.push(json!({"name":"cloudflare","type":"cdn","source":"header"})); }
            if v_low.contains("openresty") { detected.push(json!({"name":"openresty","type":"web-server","source":"header"})); }
            if v_low.contains("litespeed") { detected.push(json!({"name":"litespeed","type":"web-server","source":"header"})); }
            if v_low.contains("caddy") { detected.push(json!({"name":"caddy","type":"web-server","source":"header"})); }
        }
        if k_low == "x-powered-by" {
            detected.push(json!({"name": v, "type":"runtime","source":"x-powered-by"}));
        }
        if k_low == "x-aspnet-version" {
            detected.push(json!({"name": format!("ASP.NET {v}"),"type":"framework","source":"header"}));
        }
        if k_low == "x-drupal-cache" {
            detected.push(json!({"name":"drupal","type":"cms","source":"header"}));
        }
        if k_low == "x-shopify-stage" || k_low.contains("shopify") {
            detected.push(json!({"name":"shopify","type":"ecommerce","source":"header"}));
        }
        if k_low == "x-nextjs-cache" || k_low == "x-vercel-id" {
            detected.push(json!({"name":"next.js","type":"framework","source":"header"}));
        }
        if k_low == "x-generator" {
            detected.push(json!({"name": v, "type":"generator","source":"header"}));
        }
    }

    // Content-based signals (name, pattern, type).
    for rule in tech_rules() {
        if rule.re.is_match(&body_low) {
            detected.push(json!({"name": rule.name, "type": rule.kind, "source": "content"}));
        }
    }

    // Deduplicate and merge sources.
    let mut seen: Vec<String> = Vec::new();
    let mut merged: Vec<Value> = Vec::new();
    for d in detected {
        let name = d["name"].as_str().unwrap_or("").to_string();
        if seen.contains(&name) {
            continue;
        }
        seen.push(name);
        merged.push(d);
    }

    ctx.emitter
        .result(json!({
            "target": base.to_string(),
            "status": status,
            "technologies": merged,
        }))
        .await;
    ctx.emitter.complete(started.elapsed().as_millis()).await;
}
