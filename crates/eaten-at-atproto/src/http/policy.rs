//! What the guarded client is allowed to talk to.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Duration;

/// Limits applied to every outbound request.
#[derive(Debug, Clone)]
pub struct Policy {
    /// Permit `http://` URLs. Production is HTTPS only.
    pub allow_http: bool,
    /// Permit loopback addresses. Only for tests against a local mock server;
    /// every other non-public range stays blocked even when this is set.
    pub allow_loopback: bool,
    /// Redirect hops followed before giving up.
    pub max_redirects: u8,
    /// Default response body cap, in bytes. Callers can pass a smaller one.
    pub max_body_bytes: usize,
    pub connect_timeout: Duration,
    /// Whole-request budget, including reading the body.
    pub timeout: Duration,
    /// Concurrent requests allowed per host.
    pub max_per_host: usize,
}

impl Policy {
    /// The production policy: HTTPS only, public addresses only.
    pub fn production() -> Self {
        Self {
            allow_http: false,
            allow_loopback: false,
            max_redirects: 5,
            max_body_bytes: 2 * 1024 * 1024,
            connect_timeout: Duration::from_secs(5),
            timeout: Duration::from_secs(20),
            max_per_host: 8,
        }
    }

    /// Production limits, but plain HTTP to loopback is allowed so tests can
    /// use a mock server.
    pub fn for_tests() -> Self {
        Self {
            allow_http: true,
            allow_loopback: true,
            ..Self::production()
        }
    }

    /// Whether connecting to `ip` is permitted under this policy.
    pub fn allows_ip(&self, ip: IpAddr) -> bool {
        match classify(ip) {
            IpClass::Public => true,
            IpClass::Loopback => self.allow_loopback,
            IpClass::Blocked => false,
        }
    }
}

/// Coarse classification of an address for SSRF purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpClass {
    /// Globally routable.
    Public,
    /// 127/8 or `::1`.
    Loopback,
    /// Private, link-local, multicast, unspecified, documentation, reserved,
    /// cloud metadata, or anything else that must never be fetched.
    Blocked,
}

/// Classify an IP address. IPv6 addresses that embed an IPv4 address are
/// classified by the embedded address.
pub fn classify(ip: IpAddr) -> IpClass {
    match ip {
        IpAddr::V4(v4) => classify_v4(v4),
        IpAddr::V6(v6) => classify_v6(v6),
    }
}

fn classify_v4(ip: Ipv4Addr) -> IpClass {
    if ip.is_loopback() {
        return IpClass::Loopback;
    }
    let [a, b, ..] = ip.octets();
    let blocked = ip.is_unspecified()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_multicast()
        || ip.is_broadcast()
        || ip.is_documentation()
        || a == 0 // "this" network
        || (a == 100 && (64..=127).contains(&b)) // CGNAT 100.64/10
        || (a == 192 && b == 0) // 192.0.0/24 IETF protocol assignments
        || (a == 198 && (b == 18 || b == 19)) // benchmarking 198.18/15
        || a >= 240; // reserved 240/4 and 255.255.255.255
    if blocked {
        IpClass::Blocked
    } else {
        IpClass::Public
    }
}

fn classify_v6(ip: Ipv6Addr) -> IpClass {
    if ip.is_loopback() {
        return IpClass::Loopback;
    }
    if let Some(v4) = ip.to_ipv4_mapped() {
        return classify_v4(v4);
    }
    let segments = ip.segments();
    // NAT64 (64:ff9b::/96) and 6to4 (2002::/16) embed an IPv4 address.
    if segments[..6] == [0x64, 0xff9b, 0, 0, 0, 0] {
        return classify_v4(embedded_v4(segments[6], segments[7]));
    }
    if segments[0] == 0x2002 {
        return classify_v4(embedded_v4(segments[1], segments[2]));
    }
    let blocked = ip.is_unspecified()
        || ip.is_multicast()
        || (segments[0] & 0xfe00) == 0xfc00 // unique local fc00::/7
        || (segments[0] & 0xffc0) == 0xfe80 // link-local fe80::/10
        || (segments[0] == 0x2001 && segments[1] == 0x0db8) // documentation
        || (segments[0] == 0x2001 && segments[1] == 0) // Teredo: tunnelled, opaque
        || segments[..6] == [0, 0, 0, 0, 0, 0]; // deprecated IPv4-compatible ::/96
    if blocked {
        IpClass::Blocked
    } else {
        IpClass::Public
    }
}

fn embedded_v4(hi: u16, lo: u16) -> Ipv4Addr {
    let [a, b] = hi.to_be_bytes();
    let [c, d] = lo.to_be_bytes();
    Ipv4Addr::new(a, b, c, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn classifies_ipv4() {
        let public = [
            "1.1.1.1",
            "8.8.8.8",
            "93.184.216.34",
            "100.128.0.1",
            "172.32.0.1",
        ];
        for p in public {
            assert_eq!(classify(ip(p)), IpClass::Public, "{p}");
        }
        assert_eq!(classify(ip("127.0.0.1")), IpClass::Loopback);
        assert_eq!(classify(ip("127.255.255.254")), IpClass::Loopback);
        let blocked = [
            "0.0.0.0",
            "0.1.2.3",
            "10.0.0.1",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.1.1",
            "169.254.169.254",
            "169.254.0.1",
            "100.64.0.1",
            "100.127.255.255",
            "192.0.0.1",
            "192.0.2.1",
            "198.18.0.1",
            "198.19.255.255",
            "198.51.100.1",
            "203.0.113.1",
            "224.0.0.1",
            "240.0.0.1",
            "255.255.255.255",
        ];
        for b in blocked {
            assert_eq!(classify(ip(b)), IpClass::Blocked, "{b}");
        }
    }

    #[test]
    fn classifies_ipv6() {
        assert_eq!(classify(ip("::1")), IpClass::Loopback);
        assert_eq!(classify(ip("2606:4700:4700::1111")), IpClass::Public);
        let blocked = [
            "::",
            "fc00::1",
            "fd12:3456::1",
            "fe80::1",
            "ff02::1",
            "2001:db8::1",
            "2001::1",
            "::10.0.0.1",
        ];
        for b in blocked {
            assert_eq!(classify(ip(b)), IpClass::Blocked, "{b}");
        }
    }

    #[test]
    fn embedded_ipv4_is_classified_by_the_inner_address() {
        assert_eq!(classify(ip("::ffff:10.0.0.1")), IpClass::Blocked);
        assert_eq!(classify(ip("::ffff:127.0.0.1")), IpClass::Loopback);
        assert_eq!(classify(ip("::ffff:1.1.1.1")), IpClass::Public);
        assert_eq!(classify(ip("64:ff9b::a9fe:a9fe")), IpClass::Blocked); // 169.254.169.254
        assert_eq!(classify(ip("64:ff9b::101:101")), IpClass::Public);
        assert_eq!(classify(ip("2002:c0a8:101::")), IpClass::Blocked); // 192.168.1.1
        assert_eq!(classify(ip("2002:101:101::")), IpClass::Public);
    }

    #[test]
    fn policy_gates_loopback_only() {
        let prod = Policy::production();
        let test = Policy::for_tests();
        assert!(!prod.allows_ip(ip("127.0.0.1")));
        assert!(test.allows_ip(ip("127.0.0.1")));
        assert!(!test.allows_ip(ip("10.0.0.1")));
        assert!(!test.allows_ip(ip("169.254.169.254")));
        assert!(prod.allows_ip(ip("1.1.1.1")));
    }
}
