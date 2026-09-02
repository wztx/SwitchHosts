//! Domain-name → IP resolution over DNS-over-HTTPS (JSON API), used by
//! remote hosts entries whose `source` is `"domain"`.
//!
//! The DoH endpoints in the builtin registry are IP-literal so the
//! resolver itself never needs a DNS lookup (no bootstrap problem).
//! Query/response handling follows the Google JSON DoH shape
//! (`Status`, `Answer[].type == 1` for A records).

use std::net::Ipv4Addr;

use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct DohProvider {
    /// config id, e.g. "alidns"
    pub id: String,
    /// short display name for UI lists, e.g. "Ali DoH"
    pub name: String,
    /// label used in generated hosts comments, e.g. "Ali DoH (223.5.5.5)"
    pub label: String,
    /// URL template containing "{domain}"
    pub template: String,
    /// Cloudflare's JSON API requires this Accept header
    pub json_header: bool,
}

#[derive(Debug)]
pub enum DnsError {
    Network(String),
    Parse(String),
    BadStatus(i64),
    NoARecord,
    InvalidProvider(String),
    InvalidTemplate(String),
}

impl std::fmt::Display for DnsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DnsError::Network(m) => {
                write!(
                    f,
                    "DNS query failed: {m}. The DNS service can be changed in Preferences."
                )
            }
            DnsError::Parse(m) => write!(f, "Unexpected DNS response: {m}"),
            DnsError::BadStatus(s) => write!(
                f,
                "DNS server returned status {s}. The DNS service can be changed in Preferences."
            ),
            DnsError::NoARecord => write!(f, "Domain has no IPv4 (A) records."),
            DnsError::InvalidProvider(id) => {
                write!(f, "Unknown DNS provider \"{id}\". Fix it in Preferences.")
            }
            DnsError::InvalidTemplate(t) => write!(
                f,
                "Custom DoH template must contain the {{domain}} placeholder, got: {t}"
            ),
        }
    }
}

pub fn builtin_providers() -> Vec<DohProvider> {
    vec![
        DohProvider {
            id: "alidns".into(),
            name: "Ali DoH".into(),
            label: "Ali DoH (223.5.5.5)".into(),
            template: "https://223.5.5.5/resolve?name={domain}&type=A".into(),
            json_header: false,
        },
        DohProvider {
            id: "dnspod".into(),
            name: "DNSPod".into(),
            label: "DNSPod (120.53.53.53)".into(),
            template: "https://120.53.53.53/dns-query?name={domain}&type=A".into(),
            json_header: false,
        },
        DohProvider {
            id: "cloudflare".into(),
            name: "Cloudflare".into(),
            label: "Cloudflare (1.1.1.1)".into(),
            template: "https://1.1.1.1/dns-query?name={domain}&type=A".into(),
            json_header: true,
        },
        DohProvider {
            id: "google".into(),
            name: "Google".into(),
            label: "Google (8.8.8.8)".into(),
            template: "https://8.8.8.8/resolve?name={domain}&type=A".into(),
            json_header: false,
        },
    ]
}

pub fn is_known_provider_id(id: &str) -> bool {
    id == "custom" || builtin_providers().iter().any(|p| p.id == id)
}

pub fn provider_by_id(id: &str, custom_template: &str) -> Result<DohProvider, DnsError> {
    if let Some(p) = builtin_providers().into_iter().find(|p| p.id == id) {
        return Ok(p);
    }
    if id == "custom" {
        if !custom_template.contains("{domain}") {
            return Err(DnsError::InvalidTemplate(custom_template.to_string()));
        }
        return Ok(DohProvider {
            id: "custom".into(),
            name: "Custom".into(),
            label: "Custom DoH".into(),
            template: custom_template.to_string(),
            json_header: false,
        });
    }
    Err(DnsError::InvalidProvider(id.to_string()))
}

/// Validate a bare domain name (no scheme, no path). Must stay in sync
/// with `isValidDomain` in `src/common/hostsFn.ts`.
pub fn is_valid_domain(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() || s.len() > 253 {
        return false;
    }
    if s.contains("://") {
        return false;
    }
    if s
        .chars()
        .any(|c| matches!(c, '/' | ':' | '@' | ' ' | '\t' | '\r' | '\n'))
    {
        return false;
    }
    if s.ends_with('.') {
        return false; // FQDN trailing dot is rejected as-is
    }
    let labels: Vec<&str> = s.split('.').collect();
    if labels.len() < 2 {
        return false;
    }
    for label in &labels {
        if label.is_empty() || label.len() > 63 {
            return false;
        }
        if label.starts_with('-') || label.ends_with('-') {
            return false;
        }
        if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return false;
        }
    }
    let tld = labels[labels.len() - 1];
    if tld.len() < 2 {
        return false;
    }
    if s.parse::<std::net::IpAddr>().is_ok() {
        return false;
    }
    true
}

pub const MAX_DOH_BYTES: usize = 64 * 1024;

#[derive(Deserialize)]
struct DohAnswer {
    #[serde(rename = "type")]
    rtype: u16,
    data: String,
}

#[derive(Deserialize)]
struct DohResponse {
    #[serde(rename = "Status", default)]
    status: i64,
    #[serde(rename = "Answer", default)]
    answer: Vec<DohAnswer>,
}

/// Parse a Google-style DoH JSON body and return IPv4 A records in
/// answer order. `Status != 0`, no A records, or malformed JSON are
/// all errors (no silent fallback).
pub fn parse_doh_a_records(body: &str) -> Result<Vec<Ipv4Addr>, DnsError> {
    let resp: DohResponse =
        serde_json::from_str(body).map_err(|e| DnsError::Parse(e.to_string()))?;
    if resp.status != 0 {
        return Err(DnsError::BadStatus(resp.status));
    }
    let ips: Vec<Ipv4Addr> = resp
        .answer
        .iter()
        .filter(|a| a.rtype == 1)
        .filter_map(|a| a.data.parse::<Ipv4Addr>().ok())
        .collect();
    if ips.is_empty() {
        return Err(DnsError::NoARecord);
    }
    Ok(ips)
}

/// Build the hosts content for a domain-sourced remote entry. First IP
/// is the active line; the rest are commented alternates. The whole
/// content is rebuilt on every refresh (never appended).
pub fn build_domain_hosts_content(
    domain: &str,
    ips: &[Ipv4Addr],
    provider_label: &str,
    ts: &str,
) -> String {
    let mut lines = Vec::with_capacity(ips.len() + 3);
    lines.push(format!("# Source: domain {domain}"));
    lines.push(format!("# Resolved via {provider_label} at {ts}"));
    lines.push(format!("{} {}", ips[0], domain));
    if ips.len() > 1 {
        lines.push("# Alternate addresses:".to_string());
        for ip in &ips[1..] {
            lines.push(format!("# {ip} {domain}"));
        }
    }
    lines.join("\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_valid_domain_accepts_plain_domains() {
        for s in [
            "github.com",
            "a-b.example.co",
            "raw.githubusercontent.com",
            "xn--fiqs8s.example",
            "1.2.3.4.com",
        ] {
            assert!(is_valid_domain(s), "should accept: {s}");
        }
    }

    #[test]
    fn is_valid_domain_rejects_bad_input() {
        let too_long_label = "a".repeat(64);
        let too_long_name = format!("{}.com", "a".repeat(250));
        for s in [
            "",
            "   ",
            "github",
            "github.com.",
            "https://github.com",
            "github.com/x",
            "github.com:443",
            "a..b",
            ".a.com",
            "-a.com",
            "a-.com",
            "192.168.1.1",
            "a b.com",
            too_long_label.as_str(),
            too_long_name.as_str(),
        ] {
            assert!(!is_valid_domain(s), "should reject: {s:?}");
        }
    }

    #[test]
    fn provider_by_id_resolves_builtins_and_custom() {
        let p = provider_by_id("alidns", "").unwrap();
        assert_eq!(p.id, "alidns");
        assert!(p.template.contains("{domain}"));
        assert!(!p.json_header);

        let cf = provider_by_id("cloudflare", "").unwrap();
        assert!(cf.json_header);

        let tpl = "https://doh.example.com/resolve?name={domain}&type=A";
        let cu = provider_by_id("custom", tpl).unwrap();
        assert_eq!(cu.template, tpl);

        assert!(matches!(
            provider_by_id("custom", "https://no-placeholder.example/resolve"),
            Err(DnsError::InvalidTemplate(_))
        ));
        assert!(matches!(
            provider_by_id("bogus", ""),
            Err(DnsError::InvalidProvider(_))
        ));
    }

    #[test]
    fn known_provider_ids() {
        assert!(is_known_provider_id("alidns"));
        assert!(is_known_provider_id("custom"));
        assert!(!is_known_provider_id("bogus"));
    }

    use std::net::Ipv4Addr;

    fn ip(s: &str) -> Ipv4Addr {
        s.parse().unwrap()
    }

    #[test]
    fn parse_doh_a_records_filters_type_1_only() {
        let body = r#"{
            "Status": 0,
            "Answer": [
                {"name": "example.com.", "type": 1, "TTL": 60, "data": "93.184.216.34"},
                {"name": "example.com.", "type": 5, "TTL": 60, "data": "ns1.example.com"},
                {"name": "example.com.", "type": 28, "TTL": 60, "data": "2606:2800:220:1:1:1:1:1"},
                {"name": "example.com.", "type": 1, "TTL": 60, "data": "93.184.216.35"}
            ]
        }"#;
        let ips = parse_doh_a_records(body).unwrap();
        assert_eq!(ips, vec![ip("93.184.216.34"), ip("93.184.216.35")]);
    }

    #[test]
    fn parse_doh_a_records_error_paths() {
        assert!(matches!(
            parse_doh_a_records(r#"{"Status":3,"Answer":[]}"#),
            Err(DnsError::BadStatus(3))
        ));
        assert!(matches!(
            parse_doh_a_records(r#"{"Status":0,"Answer":[]}"#),
            Err(DnsError::NoARecord)
        ));
        assert!(matches!(
            parse_doh_a_records("not json"),
            Err(DnsError::Parse(_))
        ));
        // Answer 里全是非 A 记录 → NoARecord
        assert!(matches!(
            parse_doh_a_records(r#"{"Status":0,"Answer":[{"name":"x.","type":5,"data":"y"}]}"#),
            Err(DnsError::NoARecord)
        ));
    }

    #[test]
    fn build_content_single_ip_golden() {
        let ips = [ip("140.82.112.3")];
        let out = build_domain_hosts_content(
            "github.com",
            &ips,
            "Ali DoH (223.5.5.5)",
            "2026-09-02 14:30",
        );
        assert_eq!(
            out,
            "# Source: domain github.com\n\
             # Resolved via Ali DoH (223.5.5.5) at 2026-09-02 14:30\n\
             140.82.112.3 github.com\n"
        );
    }

    #[test]
    fn build_content_multi_ip_golden() {
        let ips = [ip("140.82.112.3"), ip("20.205.243.166")];
        let out = build_domain_hosts_content(
            "github.com",
            &ips,
            "Ali DoH (223.5.5.5)",
            "2026-09-02 14:30",
        );
        assert_eq!(
            out,
            "# Source: domain github.com\n\
             # Resolved via Ali DoH (223.5.5.5) at 2026-09-02 14:30\n\
             140.82.112.3 github.com\n\
             # Alternate addresses:\n\
             # 20.205.243.166 github.com\n"
        );
    }
}
