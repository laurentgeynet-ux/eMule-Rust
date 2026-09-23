// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

//! ICMP probing for USS — eMule `LastCommonRouteFinder` equivalent.
//!
//! eMule traces ICMP routes (increasing TTL) toward a sample of remote
//! hosts, then pings the deepest hop shared by all routes — typically the
//! ISP aggregation router. That measures congestion on the *local* uplink
//! specifically, unlike an end-to-end probe to a distant server.
//!
//! Windows uses `IcmpSendEcho` (iphlpapi.dll) — the same unprivileged API
//! as `ping.exe`/`tracert.exe` (raw sockets would need admin rights).
//! Other platforms have no unprivileged ICMP path here: every call
//! returns `None` and USS falls back to the TCP-connect probe.

use std::net::Ipv4Addr;

/// Max traceroute depth (eMule probes up to 30 hops).
const MAX_HOPS: u8 = 30;
/// Per-hop echo timeout.
const HOP_TIMEOUT_MS: u32 = 800;
/// Abort a route after this many consecutive unanswered hops.
const MAX_CONSECUTIVE_LOSS: u8 = 3;
/// ICMP payload size.
const PAYLOAD_LEN: usize = 32;

/// One ICMP echo reply. `reached` distinguishes the destination itself
/// (`IP_SUCCESS`) from a transit router (`IP_TTL_EXPIRED_TRANSIT`).
#[derive(Debug, Clone)]
pub struct EchoReply {
    /// Responding host (transit router for TTL-expired replies).
    pub responder: Ipv4Addr,
    pub rtt_ms: u32,
    /// true when the reply came from the destination itself.
    pub reached: bool,
}

#[cfg(windows)]
mod imp {
    use super::*;
    use windows::Win32::NetworkManagement::IpHelper::{
        IcmpCloseHandle, IcmpCreateFile, IcmpSendEcho, ICMP_ECHO_REPLY, IP_OPTION_INFORMATION,
        IP_SUCCESS,
    };

    /// Blocking ICMP echo — `ttl` = IP TTL (255 for a direct ping,
    /// 1..MAX_HOPS for traceroute). Returns `None` on timeout/failure.
    pub fn echo(dest: Ipv4Addr, ttl: u8, timeout_ms: u32) -> Option<EchoReply> {
        unsafe {
            let Ok(handle) = IcmpCreateFile() else {
                return None;
            };
            let payload = [0u8; PAYLOAD_LEN];
            let mut buf =
                vec![0u8; std::mem::size_of::<ICMP_ECHO_REPLY>() + PAYLOAD_LEN + 8];
            let opts = IP_OPTION_INFORMATION {
                Ttl: ttl,
                ..Default::default()
            };
            // `destinationaddress` is the IPv4 address in network byte
            // order — `from_le_bytes` on the octets yields exactly that.
            let replies = IcmpSendEcho(
                handle,
                u32::from_le_bytes(dest.octets()),
                payload.as_ptr() as *const _,
                payload.len() as u16,
                Some(&opts),
                buf.as_mut_ptr() as *mut _,
                buf.len() as u32,
                timeout_ms,
            );
            let _ = IcmpCloseHandle(handle);
            if replies == 0 {
                return None;
            }
            let reply = &*(buf.as_ptr() as *const ICMP_ECHO_REPLY);
            Some(EchoReply {
                responder: Ipv4Addr::from(reply.Address.to_le_bytes()),
                rtt_ms: reply.RoundTripTime,
                reached: reply.Status == IP_SUCCESS,
            })
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    pub fn echo(_dest: Ipv4Addr, _ttl: u8, _timeout_ms: u32) -> Option<EchoReply> {
        None
    }
}

/// Whether ICMP probing is available on this platform (Windows only).
pub fn icmp_available() -> bool {
    cfg!(windows)
}

/// ICMP ping — returns the round-trip time in ms, `None` on timeout.
pub async fn ping(dest: Ipv4Addr, timeout_ms: u32) -> Option<u32> {
    tokio::task::spawn_blocking(move || imp::echo(dest, 255, timeout_ms))
        .await
        .ok()
        .flatten()
        .filter(|r| r.reached)
        .map(|r| r.rtt_ms)
}

/// ICMP traceroute to `dest` — ordered hop list (closest … destination).
/// Hops that don't answer are skipped; `None` when nothing replies at all.
pub async fn trace_route(dest: Ipv4Addr) -> Option<Vec<Ipv4Addr>> {
    tokio::task::spawn_blocking(move || {
        let mut hops = Vec::new();
        let mut losses = 0u8;
        for ttl in 1..=MAX_HOPS {
            match imp::echo(dest, ttl, HOP_TIMEOUT_MS) {
                Some(r) => {
                    losses = 0;
                    hops.push(r.responder);
                    if r.reached {
                        break;
                    }
                }
                None => {
                    losses += 1;
                    if losses >= MAX_CONSECUTIVE_LOSS {
                        break;
                    }
                }
            }
        }
        if hops.is_empty() {
            None
        } else {
            Some(hops)
        }
    })
    .await
    .ok()
    .flatten()
}

/// eMule `CLastCommonRouteFinder`: the deepest hop present in *every*
/// route — the ISP-side aggregation point. Requires ≥ 2 routes: with a
/// single route the result would degenerate to the destination itself.
pub fn last_common_host(routes: &[Vec<Ipv4Addr>]) -> Option<Ipv4Addr> {
    if routes.len() < 2 {
        return None;
    }
    routes[0]
        .iter()
        .rev()
        .find(|hop| routes.iter().skip(1).all(|r| r.contains(hop)))
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v4(s: &str) -> Ipv4Addr {
        s.parse().unwrap()
    }

    #[test]
    fn last_common_host_picks_deepest_shared_hop() {
        let routes = vec![
            vec![
                v4("192.168.1.1"),
                v4("80.10.1.1"),
                v4("80.10.2.5"),
                v4("9.9.9.9"),
            ],
            vec![
                v4("192.168.1.1"),
                v4("80.10.1.1"),
                v4("80.10.9.9"),
                v4("8.8.8.8"),
            ],
        ];
        // Deepest hop present in every route wins (ISP aggregation point),
        // not the shallow home gateway that is also common.
        assert_eq!(last_common_host(&routes), Some(v4("80.10.1.1")));
    }

    #[test]
    fn last_common_host_requires_two_routes() {
        let routes = vec![vec![v4("192.168.1.1"), v4("80.10.1.1"), v4("8.8.8.8")]];
        assert_eq!(last_common_host(&routes), None);
    }

    #[test]
    fn last_common_host_none_when_routes_diverge() {
        let routes = vec![
            vec![v4("10.0.0.1"), v4("80.10.1.1")],
            vec![v4("10.0.0.1"), v4("90.20.2.2")],
        ];
        // 10.0.0.1 IS common to both (shallow gateway).
        assert_eq!(last_common_host(&routes), Some(v4("10.0.0.1")));

        let fully_diverged = vec![vec![v4("80.10.1.1")], vec![v4("90.20.2.2")]];
        assert_eq!(last_common_host(&fully_diverged), None);
    }
}
