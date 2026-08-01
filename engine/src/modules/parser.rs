use super::ScanContext;
use serde_json::{json, Value};
use std::time::Duration;

/// Parse a single HTML page: metadata, links, forms, scripts, comments,
/// and other structures useful for security analysis.
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

    let body = match tokio::time::timeout(timeout, ctx.client.get(base.clone()).send()).await {
        Ok(Ok(resp)) => match resp.text().await {
            Ok(t) => t,
            Err(_) => {
                ctx.emitter.event("error", "failed to read response body").await;
                return;
            }
        },
        _ => {
            ctx.emitter.event("error", format!("request to {base} timed out")).await;
            return;
        }
    };

    // All parsing happens in one sync scope; `doc` (non-Send DOM) is dropped
    // before any further awaits.
    let (title, meta, links, scripts, forms, comments) = {
        let doc = scraper::Html::parse_document(&body);

        // <title>
        let title = doc
            .select(&scraper::Selector::parse("title").unwrap())
            .next()
            .map(|n| n.text().collect::<String>().trim().to_string());

        // meta tags
        let mut meta: Vec<Value> = Vec::new();
        for node in doc.select(&scraper::Selector::parse("meta").unwrap()) {
            let name = node.value().attr("name").or_else(|| node.value().attr("property")).unwrap_or("");
            let content = node.value().attr("content").unwrap_or("");
            if !name.is_empty() {
                meta.push(json!({ "name": name, "content": content }));
            }
        }

        // links (external + internal)
        let mut links: Vec<Value> = Vec::new();
        for node in doc.select(&scraper::Selector::parse("a[href]").unwrap()) {
            let href = node.value().attr("href").unwrap_or("");
            if href.is_empty() {
                continue;
            }
            let text = node.text().collect::<String>().trim().to_string();
            links.push(json!({ "href": href, "text": text }));
        }

        // scripts
        let mut scripts: Vec<String> = Vec::new();
        for node in doc.select(&scraper::Selector::parse("script").unwrap()) {
            let src = node.value().attr("src").unwrap_or("");
            let inline = node.text().collect::<String>();
            if !src.is_empty() {
                scripts.push(src.to_string());
            } else if inline.contains("eval(")
                || inline.contains("document.write(")
                || inline.contains("atob(")
                || inline.contains("innerHTML")
            {
                scripts.push(format!("inline (suspicious): {}", inline.chars().take(200).collect::<String>()));
            }
        }

        // forms
        let mut forms: Vec<Value> = Vec::new();
        for node in doc.select(&scraper::Selector::parse("form").unwrap()) {
            let action = node.value().attr("action").unwrap_or("");
            let method = node.value().attr("method").unwrap_or("GET").to_uppercase();
            let mut inputs: Vec<Value> = Vec::new();
            let mut csrf_field: Option<String> = None;
            for input in node.select(&scraper::Selector::parse("input, textarea, select").unwrap()) {
                let name = input.value().attr("name").unwrap_or("");
                let input_type = input.value().attr("type").unwrap_or("text");
                if !name.is_empty() {
                    inputs.push(json!({ "name": name, "type": input_type }));
                    let name_low = name.to_lowercase();
                    if name_low.contains("csrf")
                        || name_low.contains("token")
                        || name_low.contains("authenticity")
                        || name_low.contains("nonce")
                        || name_low.contains("_wpnonce")
                        || name_low == "_token"
                    {
                        if csrf_field.is_none() {
                            csrf_field = Some(name.to_string());
                        }
                    }
                }
            }
            // Also honor a <meta name="csrf-token"> (common in SPAs/Laravel).
            let meta_csrf = doc
                .select(&scraper::Selector::parse("meta[name$='csrf'], meta[property$='csrf'], meta[name='csrf-token'], meta[name='_csrf']").unwrap())
                .next()
                .map(|m| m.value().attr("content").unwrap_or("").to_string());
            if csrf_field.is_none() {
                csrf_field = meta_csrf;
            }
            forms.push(json!({
                "action": action,
                "method": method,
                "inputs": inputs,
                "insecure_action": action.starts_with("http://") || action.starts_with("//"),
                "has_csrf_token": csrf_field.is_some(),
                "csrf_field": csrf_field,
                "no_csrf_visible": csrf_field.is_none(),
            }));
        }

        // HTML comments (may leak credentials or hints)
        let mut comments: Vec<String> = Vec::new();
        fn walk(node: ego_tree::NodeRef<'_, scraper::node::Node>, out: &mut Vec<String>) {
            if let scraper::node::Node::Comment(c) = node.value() {
                let text = c.comment.to_string();
                if text.chars().count() > 3 {
                    out.push(text);
                }
            }
            let mut child = node.first_child();
            while let Some(c) = child {
                walk(c, out);
                child = c.next_sibling();
            }
        }
        walk(doc.tree.root(), &mut comments);

        (title, meta, links, scripts, forms, comments)
    };

    ctx.emitter
        .result(json!({
            "target": base.to_string(),
            "title": title,
            "meta": meta,
            "links_count": links.len(),
            "links": links,
            "scripts": scripts,
            "forms": forms,
            "comments": comments,
        }))
        .await;
    ctx.emitter.complete(started.elapsed().as_millis()).await;
}
