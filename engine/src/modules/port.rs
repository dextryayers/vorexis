use super::ScanContext;
use crate::core::pool::WorkerPool;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const COMMON_PORTS: &[u16] = &[
    21, 22, 23, 25, 53, 80, 110, 111, 135, 139, 143, 443, 445, 465, 514, 587, 631, 636, 873,
    993, 995, 1025, 1080, 1099, 1433, 1521, 1723, 2049, 2181, 2375, 2376, 3000, 3128, 3306,
    3389, 3690, 4000, 4200, 4369, 4443, 5000, 5001, 5432, 5555, 5672, 5900, 5984, 5985, 5986,
    6379, 6443, 7001, 7002, 7070, 8000, 8001, 8008, 8009, 8080, 8081, 8082, 8083, 8084, 8085,
    8086, 8087, 8088, 8089, 8090, 8161, 8200, 8443, 8500, 8600, 8888, 9000, 9001, 9002, 9042,
    9090, 9092, 9100, 9200, 9300, 9418, 9999, 10000, 10001, 11211, 15672, 16379, 18080, 20000,
    27017, 27018, 28017, 30000, 32768, 44818, 49152, 50000, 50070, 61616, 62078,
];

/// Parse a port spec like "80", "80,443", "1-1000", "1-1000,8080,9000-9100".
pub fn parse_ports(spec: &str) -> Vec<u16> {
    let mut ports = Vec::new();
    for part in spec.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((a, b)) = part.split_once('-') {
            let (Ok(lo), Ok(hi)) = (a.trim().parse::<u16>(), b.trim().parse::<u16>()) else {
                continue;
            };
            if lo > hi || hi - lo > 65_000 {
                continue;
            }
            ports.extend(lo..=hi);
        } else if let Ok(p) = part.parse::<u16>() {
            if p > 0 {
                ports.push(p);
            }
        }
    }
    ports.sort_unstable();
    ports.dedup();
    ports
}

async fn grab_banner(stream: &mut TcpStream, timeout: u64) -> Option<String> {
    // Send a generic probe, wait briefly for a greeting banner.
    let _ = stream.write_all(b"\r\n").await;
    let mut buf = [0u8; 512];
    match tokio::time::timeout(Duration::from_millis(timeout.saturating_mul(150)), stream.read(&mut buf)).await {
        Ok(Ok(n)) if n > 0 => Some(String::from_utf8_lossy(&buf[..n]).trim().to_string()),
        _ => None,
    }
}

pub async fn run(ctx: &ScanContext) {
    let started = std::time::Instant::now();
    let spec = ctx.job.option("ports", "common");
    let ports = if spec == "common" {
        COMMON_PORTS.to_vec()
    } else if spec == "all" {
        (1u16..=65535).collect()
    } else {
        parse_ports(&spec)
    };

    let host = ctx.host();
    let timeout = ctx.timeout().max(1);

    // Resolve hostname once.
    let mut ips: Vec<String> = match tokio::net::lookup_host((host.as_str(), 0)).await {
        Ok(addrs) => addrs
            .map(|a| a.ip().to_string())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect(),
        Err(_) => vec![host.clone()],
    };
    if ips.is_empty() {
        ctx.emitter.event("error", format!("cannot resolve {host}")).await;
        return;
    }
    // Prefer IPv4: v6 SYNs to filtered hosts often hang the full timeout.
    if ips.iter().any(|ip| ip.contains('.')) {
        ips.retain(|ip| ip.contains('.'));
    }
    ips.truncate(3);
    ctx.emitter
        .event("info", format!("scanning {} ports on {} ({})", ports.len(), host, ips.join(",")))
        .await;

    let total = ports.len() as u64;
    let mut open: Vec<Value> = Vec::new();
    let mut checked: u64 = 0;

    // Scan ports concurrently (bounded by job concurrency). Each worker tries
    // every resolved IP and gives up after `timeout` seconds per address.
    let pool = WorkerPool::new(ctx.job.concurrency.max(1));
    let ips = std::sync::Arc::new(ips);
    let scan_results: Vec<Option<(u16, u128, Option<String>)>> = pool
        .run(ports, move |port| {
            let ips = ips.clone();
            async move {
                for ip in ips.iter() {
                    let t = tokio::time::Instant::now();
                    match tokio::time::timeout(
                        Duration::from_secs(timeout),
                        TcpStream::connect((ip.as_str(), port)),
                    )
                    .await
                    {
                        Ok(Ok(mut stream)) => {
                            let latency = t.elapsed().as_millis();
                            let banner = grab_banner(&mut stream, timeout).await;
                            return Some((port, latency, banner));
                        }
                        _ => continue,
                    }
                }
                None
            }
        })
        .await;

    for res in scan_results {
        checked += 1;
        if let Some((port, latency, banner)) = res {
            open.push(json!({
                "port": port,
                "protocol": "tcp",
                "latency_ms": latency,
                "banner": banner,
                "service": guess_service(port),
            }));
            ctx.emitter
                .result(json!({
                    "port": port,
                    "protocol": "tcp",
                    "status": "open",
                    "latency_ms": latency,
                    "banner": banner,
                    "service": guess_service(port),
                }))
                .await;
        }
        if checked % 50 == 0 || checked == total {
            ctx.emitter.progress(checked, total).await;
        }
    }

    ctx.emitter
        .result(json!({
            "target": host,
            "ports_scanned": total,
            "open_ports": open,
        }))
        .await;
    ctx.emitter.complete(started.elapsed().as_millis()).await;
}

pub fn guess_service(port: u16) -> &'static str {
    match port {
        21 => "ftp",
        22 => "ssh",
        23 => "telnet",
        25 => "smtp",
        53 => "dns",
        80 => "http",
        110 => "pop3",
        111 => "rpcbind",
        135 => "msrpc",
        139 | 445 => "smb/netbios",
        143 => "imap",
        443 => "https",
        465 | 587 => "smtp-submission",
        514 => "syslog",
        631 => "ipp",
        636 => "ldaps",
        873 => "rsync",
        993 => "imaps",
        995 => "pop3s",
        1080 => "socks",
        1433 => "mssql",
        1521 => "oracle",
        1723 => "pptp",
        2049 => "nfs",
        2181 => "zookeeper",
        2375 | 2376 => "docker",
        3000 => "grafana/gitea/dev",
        3128 => "squid",
        3306 => "mysql",
        3389 => "rdp",
        4369 => "erlang",
        5000 => "upnp/flask",
        5432 => "postgresql",
        5672 => "amqp",
        5900 => "vnc",
        5984 => "couchdb",
        6379 => "redis",
        6443 => "kubernetes-api",
        7001 => "weblogic",
        8000 => "http-alt",
        8009 => "ajp",
        8080 => "http-proxy",
        8081 => "http-alt",
        8161 => "activemq",
        8443 => "https-alt",
        8888 => "http-alt",
        9000 => "php-fpm/portainer",
        9092 => "kafka",
        9200 | 9300 => "elasticsearch",
        11211 => "memcached",
        15672 => "rabbitmq-mgmt",
        27017 | 27018 | 28017 => "mongodb",
        50070 => "hdfs",
        61616 => "activemq-openwire",
        _ => "unknown",
    }
}
