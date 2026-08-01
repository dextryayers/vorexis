use super::ScanContext;
use hickory_resolver::proto::rr::RData;
use hickory_resolver::proto::rr::RecordType;
use hickory_resolver::TokioResolver;
use serde_json::{json, Value};
use std::sync::Arc;

pub async fn run(ctx: &ScanContext) {
    let started = std::time::Instant::now();
    let host = ctx.host();
    let resolver = match TokioResolver::builder_tokio() {
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
    let timeout = std::time::Duration::from_secs(ctx.timeout().max(1));

    ctx.emitter.event("info", format!("querying DNS records for {host}")).await;

    let mut records: Vec<Value> = Vec::new();

    // A / AAAA
    match tokio::time::timeout(timeout, resolver.lookup_ip(host.clone())).await {
        Ok(Ok(ips)) => {
            for ip in ips.iter() {
                records.push(json!({
                    "type": if ip.is_ipv4() { "A" } else { "AAAA" },
                    "name": host,
                    "value": ip.to_string(),
                }));
            }
        }
        _ => records.push(json!({ "type": "A/AAAA", "name": host, "value": "no records" })),
    }

    // CNAME
    if let Ok(Ok(lookup)) = tokio::time::timeout(timeout, resolver.lookup(host.clone(), RecordType::CNAME)).await {
        for rec in lookup.answers() {
            if let RData::CNAME(cname) = &rec.data {
                records.push(json!({ "type": "CNAME", "name": host, "value": cname.to_string() }));
            }
        }
    }

    // MX
    if let Ok(Ok(lookup)) = tokio::time::timeout(timeout, resolver.mx_lookup(host.clone())).await {
        for rec in lookup.answers() {
            if let RData::MX(mx) = &rec.data {
                records.push(json!({
                    "type": "MX",
                    "name": host,
                    "value": mx.exchange.to_string(),
                    "priority": mx.preference,
                }));
            }
        }
    }

    // NS
    if let Ok(Ok(lookup)) = tokio::time::timeout(timeout, resolver.ns_lookup(host.clone())).await {
        for rec in lookup.answers() {
            if let RData::NS(ns) = &rec.data {
                records.push(json!({ "type": "NS", "name": host, "value": ns.to_string() }));
            }
        }
    }

    // TXT
    if let Ok(Ok(lookup)) = tokio::time::timeout(timeout, resolver.txt_lookup(host.clone())).await {
        for rec in lookup.answers() {
            if let RData::TXT(txt) = &rec.data {
                for part in txt.txt_data.iter() {
                    records.push(json!({
                        "type": "TXT",
                        "name": host,
                        "value": String::from_utf8_lossy(part),
                    }));
                }
            }
        }
    }

    // SOA
    if let Ok(Ok(lookup)) = tokio::time::timeout(timeout, resolver.soa_lookup(host.clone())).await {
        for rec in lookup.answers() {
            if let RData::SOA(soa) = &rec.data {
                records.push(json!({
                    "type": "SOA",
                    "name": host,
                    "value": format!("primary: {} | admin: {}", soa.mname, soa.rname),
                }));
            }
        }
    }

    ctx.emitter
        .result(json!({ "domain": host, "records": records }))
        .await;
    ctx.emitter.complete(started.elapsed().as_millis()).await;
}
