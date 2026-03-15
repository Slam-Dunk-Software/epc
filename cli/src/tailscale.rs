use anyhow::Result;
use serde::Deserialize;

#[derive(Deserialize)]
struct TailscaleStatus {
    #[serde(rename = "Self")]
    self_node: Option<SelfNode>,
}

#[derive(Deserialize)]
struct SelfNode {
    /// All Tailscale IPs for this node (typically one IPv4 in 100.x.x.x, one IPv6).
    #[serde(rename = "TailscaleIPs")]
    tailscale_ips: Option<Vec<String>>,
}

/// Returns the Tailscale IPv4 address for this machine (e.g. `100.78.103.79`).
///
/// We deliberately use the IP rather than the DNS name (`machine.tail.net`) because:
/// - The DNS name is publicly resolvable; the IP is only meaningful inside the tailnet.
/// - Surfacing the DNS name in logs/URLs leaks network topology to anyone who reads them.
///
/// Falls back to `"localhost"` if Tailscale is unavailable or not connected.
pub async fn ip() -> Result<String> {
    let output = tokio::process::Command::new("tailscale")
        .args(["status", "--json"])
        .output()
        .await;

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return Ok("localhost".to_string()),
    };

    let status: TailscaleStatus = match serde_json::from_slice(&output.stdout) {
        Ok(s) => s,
        Err(_) => return Ok("localhost".to_string()),
    };

    let addr = status
        .self_node
        .and_then(|n| n.tailscale_ips)
        .and_then(|ips| {
            // Prefer the first IPv4 address (100.x.x.x CGNAT range)
            ips.into_iter().find(|ip| ip.starts_with("100."))
        })
        .unwrap_or_else(|| "localhost".to_string());

    Ok(addr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn ip_returns_non_empty_string() {
        let addr = ip().await.unwrap();
        assert!(!addr.is_empty());
    }

    #[tokio::test]
    async fn ip_is_tailscale_or_localhost() {
        let addr = ip().await.unwrap();
        assert!(
            addr == "localhost" || addr.starts_with("100."),
            "unexpected address: {addr}"
        );
    }

    #[tokio::test]
    async fn ip_has_no_trailing_dot() {
        let addr = ip().await.unwrap();
        assert!(!addr.ends_with('.'));
    }

    #[test]
    fn ipv4_preferred_over_ipv6() {
        let ips = vec!["fd7a:115c::1".to_string(), "100.78.103.79".to_string()];
        let addr = ips.into_iter().find(|ip| ip.starts_with("100.")).unwrap();
        assert_eq!(addr, "100.78.103.79");
    }
}
