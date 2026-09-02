//! Domain-name → IP resolution over DNS-over-HTTPS (JSON API), used by
//! remote hosts entries whose `source` is `"domain"`.
//!
//! The DoH endpoints in the builtin registry are IP-literal so the
//! resolver itself never needs a DNS lookup (no bootstrap problem).
//! Query/response handling follows the Google JSON DoH shape
//! (`Status`, `Answer[].type == 1` for A records).

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
}
