use super::ScanContext;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::aws_lc_rs;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, Error, SignatureScheme};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Accepts any certificate — we only want to *inspect* TLS, not validate it.
#[derive(Debug)]
struct AcceptAllVerifier;

impl ServerCertVerifier for AcceptAllVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        aws_lc_rs::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// SSL/TLS analysis: certificate details, protocol version, cipher suite.
/// Uses a manual TLS handshake via rustls to capture peer certificates.
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
    let port = base.port_or_known_default().unwrap_or(443);
    let timeout = Duration::from_secs(ctx.timeout().max(1));

    ctx.emitter
        .event("info", format!("analyzing TLS on {host}:{port}"))
        .await;

    let mut results: Vec<Value> = Vec::new();

    let provider = Arc::new(aws_lc_rs::default_provider());
    let versions = vec![&rustls::version::TLS13, &rustls::version::TLS12];

    let tls_config = rustls::ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(&versions)
        .unwrap()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AcceptAllVerifier))
        .with_no_client_auth();
    // Advertise HTTP/2 via ALPN so we can report real protocol support.
    let mut tls_config = tls_config;
    tls_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    let server_name = rustls::pki_types::ServerName::try_from(host.clone())
        .unwrap_or_else(|_| rustls::pki_types::ServerName::try_from("localhost").unwrap());
    let connector = tokio_rustls::TlsConnector::from(Arc::new(tls_config));

    let connect_result = async {
        let tcp = TcpStream::connect((host.as_str(), port)).await?;
        let mut tls = connector.connect(server_name.clone(), tcp).await?;
        let certs = tls.get_ref().1.peer_certificates().map(|c| c.to_vec()).unwrap_or_default();
        let protocol = tls.get_ref().1.protocol_version();
        let cipher = tls.get_ref().1.negotiated_cipher_suite();
        let alpn = tls.get_ref().1.alpn_protocol().map(|p| p.to_vec());
        // Trigger handshake completion by reading (server may close without data).
        let _ = tokio::time::timeout(Duration::from_secs(3), async {
            let mut buf = [0u8; 4096];
            let _ = tls.read(&mut buf).await;
        })
        .await;
        let _ = tls.flush().await;
        Ok::<_, Box<dyn std::error::Error>>((certs, protocol, cipher, alpn))
    };

    match tokio::time::timeout(timeout, connect_result).await {
        Ok(Ok((certs, protocol, cipher, alpn))) => {
            let tls_version = match protocol {
                Some(p) => match u16::from(p) {
                    0x0304 => "TLSv1.3".to_string(),
                    0x0303 => "TLSv1.2".to_string(),
                    0x0302 => "TLSv1.1".to_string(),
                    0x0301 => "TLSv1.0".to_string(),
                    v => format!("0x{v:04x}"),
                },
                None => "unknown".to_string(),
            };
            let tls_version_ok = protocol.is_some();
            let cipher_name = cipher
                .map(|c| c.suite().as_str().unwrap_or("unknown").to_string())
                .unwrap_or_else(|| "unknown".to_string());
            let supports_http2 = alpn.as_deref() == Some(b"h2");

            // Parse certificates with x509-parser.
            let mut parsed: Vec<Value> = Vec::new();
            for (idx, cert) in certs.iter().enumerate() {
                if let Ok((_, parsed_cert)) = x509_parser::parse_x509_certificate(cert) {
                    let subject = parsed_cert.subject().to_string();
                    let issuer = parsed_cert.issuer().to_string();
                    let not_before = parsed_cert.validity().not_before.to_rfc2822();
                    let not_after = parsed_cert.validity().not_after.to_rfc2822();
                    let days_left = {
                        use std::time::{SystemTime, UNIX_EPOCH};
                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .map(|d| d.as_secs() as i64)
                            .unwrap_or(0);
                        let seconds = parsed_cert.validity().not_after.timestamp() - now;
                        seconds / 86400
                    };
                    let san: Vec<String> = parsed_cert
                        .subject_alternative_name()
                        .ok()
                        .flatten()
                        .map(|ext| {
                            ext.value
                                .general_names
                                .iter()
                                .filter_map(|gn| {
                                    use x509_parser::extensions::GeneralName;
                                    match gn {
                                        GeneralName::DNSName(d) => Some(d.to_string()),
                                        GeneralName::URI(u) => Some(u.to_string()),
                                        GeneralName::IPAddress(ip) => Some(match ip.len() {
                                            4 => std::net::Ipv4Addr::from(
                                                <[u8; 4]>::try_from(&ip[..]).unwrap_or([0; 4]),
                                            )
                                            .to_string(),
                                            16 => std::net::Ipv6Addr::from(
                                                <[u8; 16]>::try_from(&ip[..]).unwrap_or([0; 16]),
                                            )
                                            .to_string(),
                                            _ => ip
                                                .iter()
                                                .map(|b| b.to_string())
                                                .collect::<Vec<_>>()
                                                .join("."),
                                        }),
                                        GeneralName::RFC822Name(e) => Some(e.to_string()),
                                        _ => None,
                                    }
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    let host_matches = {
                        let host_low = host.to_lowercase();
                        san.iter().any(|name| {
                            let n = name.to_lowercase();
                            n == host_low || (n.starts_with("*.") && host_low.ends_with(&n[1..]))
                        })
                    };
                    let signature_alg = parsed_cert.signature_algorithm.algorithm.to_string();
                    let key = parsed_cert.public_key().algorithm.algorithm.to_string();
                    let serial = parsed_cert.raw_serial_as_string();
                    let version = parsed_cert.version();

                    parsed.push(json!({
                        "chain_index": idx,
                        "subject": subject,
                        "issuer": issuer,
                        "not_before": not_before,
                        "not_after": not_after,
                        "days_until_expiry": days_left,
                        "expired": days_left < 0,
                        "expiring_soon": (0..=30).contains(&days_left),
                        "san": san,
                        "hostname_matches": host_matches,
                        "signature_algorithm": signature_alg,
                        "public_key_algorithm": key,
                        "serial": serial,
                        "x509_version": format!("{version:?}"),
                        "self_signed": false,
                    }));
                }
            }

            // Determine self-signed: leaf issuer == leaf subject.
            if let Some(leaf) = parsed.first_mut() {
                let subject = leaf["subject"].as_str().unwrap_or("").to_string();
                let issuer = leaf["issuer"].as_str().unwrap_or("").to_string();
                let is_self = subject.trim() == issuer.trim();
                leaf["self_signed"] = json!(is_self);
            }

            let weak_cipher = cipher_name.contains("CBC") || cipher_name.contains("RC4") || cipher_name.contains("3DES");

            results.push(json!({
                "host": host,
                "port": port,
                "tls_version": tls_version,
                "tls_version_ok": tls_version_ok,
                "cipher_suite": cipher_name,
                "weak_cipher": weak_cipher,
                "certificates": parsed,
                "certificate_chain_len": certs.len(),
                "supports_http2": supports_http2,
            }));
        }
        Ok(Err(e)) => {
            results.push(json!({
                "host": host,
                "port": port,
                "error": e.to_string(),
            }));
        }
        Err(_) => {
            results.push(json!({
                "host": host,
                "port": port,
                "error": "TLS handshake timed out (port may be closed or non-TLS)",
            }));
        }
    }

    ctx.emitter.result(json!({ "target": host, "results": results })).await;
    ctx.emitter.complete(started.elapsed().as_millis()).await;
}
