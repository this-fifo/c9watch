use std::net::IpAddr;
use std::process::Command;

/// Get the local network IP address (for display in connection info)
pub fn get_local_ip() -> String {
    std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|socket| {
            socket.connect("8.8.8.8:80")?;
            Ok(socket.local_addr()?.ip().to_string())
        })
        .unwrap_or_else(|_| "127.0.0.1".to_string())
}

/// Get the Tailscale MagicDNS hostname if Tailscale is running.
/// Returns e.g. "macbook.tail1234.ts.net" or None if unavailable.
pub fn get_tailscale_hostname() -> Option<String> {
    let output = Command::new("tailscale")
        .arg("status")
        .arg("--json")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let dns_name = json.get("Self")?.get("DNSName")?.as_str()?;
    // DNSName has a trailing dot — strip it
    let name = dns_name.trim_end_matches('.');
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

/// Generate or refresh Tailscale HTTPS certificates (Let's Encrypt via `tailscale cert`).
/// Returns (cert_path, key_path) on success, or None if certs can't be generated.
pub fn get_tailscale_certs(hostname: &str) -> Option<(std::path::PathBuf, std::path::PathBuf)> {
    let cache_dir = dirs::cache_dir()?.join("c9watch");
    std::fs::create_dir_all(&cache_dir).ok()?;

    let cert_path = cache_dir.join("tailscale.crt");
    let key_path = cache_dir.join("tailscale.key");

    let output = Command::new("tailscale")
        .args([
            "cert",
            "--cert-file",
            cert_path.to_str()?,
            "--key-file",
            key_path.to_str()?,
            hostname,
        ])
        .output()
        .ok()?;

    if output.status.success() {
        Some((cert_path, key_path))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        crate::debug_log::log_warn(&format!(
            "[auth] tailscale cert failed: {}",
            stderr.trim()
        ));
        None
    }
}

/// Check if an IP address belongs to the Tailscale network.
///   IPv4: 100.64.0.0/10 (CGNAT range)
///   IPv6: fd7a:115c:a1e0::/48
///   IPv4-mapped IPv6 (::ffff:100.x.x.x): produced by macOS dual-stack sockets
pub fn is_tailscale_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let octets = v4.octets();
            octets[0] == 100 && (octets[1] & 0xC0) == 64
        }
        IpAddr::V6(v6) => {
            // On macOS, binding to [::] creates a dual-stack socket. IPv4 clients
            // (including Tailscale 100.x.x.x peers) arrive as IPv4-mapped IPv6
            // addresses (::ffff:100.x.x.x). Unwrap those and check the IPv4 range.
            if let Some(v4) = v6.to_ipv4_mapped() {
                let octets = v4.octets();
                return octets[0] == 100 && (octets[1] & 0xC0) == 64;
            }
            // Native Tailscale IPv6 range (fd7a:115c:a1e0::/48)
            let segments = v6.segments();
            segments[0] == 0xfd7a && segments[1] == 0x115c && segments[2] == 0xa1e0
        }
    }
}

/// Check if a connecting address should be allowed.
/// Permits localhost and Tailscale IPs only.
pub fn is_allowed_ip(addr: &std::net::SocketAddr) -> bool {
    let ip = addr.ip();
    ip.is_loopback() || is_tailscale_ip(&ip)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn test_tailscale_ipv4_range() {
        // 100.64.0.0 - 100.127.255.255
        assert!(is_tailscale_ip(&IpAddr::V4(Ipv4Addr::new(100, 64, 0, 1))));
        assert!(is_tailscale_ip(&IpAddr::V4(Ipv4Addr::new(100, 100, 50, 1))));
        assert!(is_tailscale_ip(&IpAddr::V4(Ipv4Addr::new(100, 127, 255, 255))));
        // Outside range
        assert!(!is_tailscale_ip(&IpAddr::V4(Ipv4Addr::new(100, 128, 0, 1))));
        assert!(!is_tailscale_ip(&IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
        assert!(!is_tailscale_ip(&IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))));
    }

    #[test]
    fn test_tailscale_ipv6_range() {
        assert!(is_tailscale_ip(&IpAddr::V6(Ipv6Addr::new(
            0xfd7a, 0x115c, 0xa1e0, 0, 0, 0, 0, 1
        ))));
        // Wrong prefix
        assert!(!is_tailscale_ip(&IpAddr::V6(Ipv6Addr::new(
            0xfd7a, 0x115c, 0xb1e0, 0, 0, 0, 0, 1
        ))));
        assert!(!is_tailscale_ip(&IpAddr::V6(Ipv6Addr::LOCALHOST)));
    }

    #[test]
    fn test_localhost_allowed() {
        use std::net::SocketAddr;
        let v4_local: SocketAddr = "127.0.0.1:9210".parse().unwrap();
        let v6_local: SocketAddr = "[::1]:9210".parse().unwrap();
        assert!(is_allowed_ip(&v4_local));
        assert!(is_allowed_ip(&v6_local));
    }

    #[test]
    fn test_lan_ip_denied() {
        use std::net::SocketAddr;
        let lan: SocketAddr = "192.168.1.50:12345".parse().unwrap();
        assert!(!is_allowed_ip(&lan));
    }

    #[test]
    fn test_tailscale_ip_allowed() {
        use std::net::SocketAddr;
        let ts: SocketAddr = "100.100.50.1:12345".parse().unwrap();
        assert!(is_allowed_ip(&ts));
    }

    #[test]
    fn test_ipv4_mapped_tailscale_allowed() {
        // macOS dual-stack socket presents IPv4 clients as ::ffff:a.b.c.d
        use std::net::{Ipv6Addr, SocketAddr};
        // ::ffff:100.100.50.1
        let mapped = Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0x6464, 0x3201);
        assert!(is_tailscale_ip(&IpAddr::V6(mapped)));

        let sa = SocketAddr::new(IpAddr::V6(mapped), 9210);
        assert!(is_allowed_ip(&sa));
    }

    #[test]
    fn test_ipv4_mapped_non_tailscale_denied() {
        use std::net::Ipv6Addr;
        // ::ffff:192.168.1.50
        let mapped = Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc0a8, 0x0132);
        assert!(!is_tailscale_ip(&IpAddr::V6(mapped)));
    }
}
