// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! LowID callback reachability — eMule `CemuleApp::CanDoCallback`
//! (`Emule.cpp:1067`) and `CServerConnect::IsLocalServer`
//! (`ServerConnect.cpp:519`).
//!
//! LowIDs are only unique per server: `OP_CALLBACKREQUEST` reaches a
//! `lowid:<id>` source only when it lives on our connected server. When we
//! cannot receive a connect-back at all, the source stays parked
//! (eMule `DS_LOWTOLOWIP`) until our ID state changes; when we can but no
//! callback option applies, eMule drops it
//! (`Disconnected("LowID: No Callback Option available")`).

use mule_core::model::{PeerSource, PeerSourceOrigin};

/// Callback reachability context for one scheduling tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CallbackCtx {
    /// eD2K server session is connected (`client_id != 0` assigned).
    pub server_connected: bool,
    /// The server assigned us a HighID — inbound TCP reaches us
    /// (eMule `IsLowID() == false`).
    pub server_high_id: bool,
    /// Connected server IPv4 (LE u32) — `cur_server->GetIP()`.
    pub our_server_ip: u32,
    /// Connected server TCP port — `cur_server->GetPort()`.
    pub our_server_port: u16,
    /// Kad UDP is running *and* verified reachable
    /// (`CKademlia::IsConnected() && !IsFirewalled()`): the peer can then
    /// connect back to our TCP port even when our eD2K ID is LowID.
    pub kad_open: bool,
}

/// eMule `CServerConnect::IsLocalServer` — the source lives on our
/// connected server, so `OP_CALLBACKREQUEST` targets the right LowID
/// namespace.
///
/// `server_ip == 0` on the source means the server context is unknown:
/// only server-origin sources count as local then (they were returned by
/// our own server's `OP_FOUNDSOURCES`). `server_port == 0` = port unknown
/// (UDP `OP_GLOBFOUNDSOURCES` responders) → IP-only match.
pub(crate) fn is_local_server(src: &PeerSource, ctx: &CallbackCtx) -> bool {
    if !ctx.server_connected {
        return false;
    }
    if src.server_ip == 0 {
        return src.origin == PeerSourceOrigin::Server;
    }
    src.server_ip == ctx.our_server_ip
        && (src.server_port == 0 || src.server_port == ctx.our_server_port)
}

/// eMule `CemuleApp::CanDoCallback` — can the LowID source be asked to
/// connect back to us?
///
/// - No usable Kad → only a HighID eD2K connection makes us reachable.
/// - Kad connected and not firewalled → callbacks are possible; but when
///   *we* are LowID on the same server as the source, the callback would
///   relay onto ourselves — eMule refuses (protocol violation, ban risk).
pub(crate) fn can_do_callback(src: &PeerSource, ctx: &CallbackCtx) -> bool {
    if !ctx.kad_open {
        return ctx.server_high_id;
    }
    let we_lowid = ctx.server_connected && !ctx.server_high_id;
    if we_lowid {
        !is_local_server(src, ctx)
    } else {
        true
    }
}

/// Whether a `lowid:` source has a viable server-callback path — eMule
/// `TryToConnect`'s `IsLocalServer` option (`BaseClient.cpp:1429`). The
/// remaining eMule options (direct UDP callback, Kad buddy) are not
/// carried by `lowid:` sources (`kadcb:` sources carry the buddy instead).
pub(crate) fn has_server_callback_option(src: &PeerSource, ctx: &CallbackCtx) -> bool {
    is_local_server(src, ctx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mule_core::model::PeerSourceStatus;

    fn ctx(server_connected: bool, high: bool, kad_open: bool) -> CallbackCtx {
        CallbackCtx {
            server_connected,
            server_high_id: high,
            our_server_ip: u32::from_le_bytes([1, 2, 3, 4]),
            our_server_port: 4661,
            kad_open,
        }
    }

    fn lowid_src(server_ip: u32, server_port: u16, origin: PeerSourceOrigin) -> PeerSource {
        PeerSource {
            file_hash: "00".repeat(16),
            address: "lowid:42".to_string(),
            ip: 42,
            port: 0,
            origin,
            discovered_at_unix_ms: 0,
            last_seen_unix_ms: 0,
            status: PeerSourceStatus::Candidate,
            fail_count: 0,
            last_error: None,
            cooldown_until_unix_ms: None,
            user_hash: None,
            crypt_options: 0,
            buddy_id: None,
            software: None,
            availability: None,
            server_ip,
            server_port,
            udp_port: 0,
        }
    }

    const OUR_IP: u32 = u32::from_le_bytes([1, 2, 3, 4]);

    #[test]
    fn local_server_matching() {
        let c = ctx(true, true, false);
        // Server-origin sources without explicit server fields are local.
        assert!(is_local_server(
            &lowid_src(0, 0, PeerSourceOrigin::Server),
            &c
        ));
        // SX sources: exact endpoint match or IP-only (port unknown).
        let sx = PeerSourceOrigin::SourceExchange;
        assert!(is_local_server(&lowid_src(OUR_IP, 4661, sx), &c));
        assert!(is_local_server(&lowid_src(OUR_IP, 0, sx), &c));
        assert!(!is_local_server(&lowid_src(OUR_IP, 4662, sx), &c));
        assert!(!is_local_server(&lowid_src(0x0A0B0C0D, 0, sx), &c));
        // Not connected → nothing is local.
        assert!(!is_local_server(
            &lowid_src(0, 0, PeerSourceOrigin::Server),
            &ctx(false, false, false)
        ));
        // SX without server fields is NOT local.
        assert!(!is_local_server(&lowid_src(0, 0, sx), &c));
    }

    #[test]
    fn can_do_callback_matrix() {
        let sx = PeerSourceOrigin::SourceExchange;
        let local = lowid_src(OUR_IP, 4661, sx);
        let foreign = lowid_src(0x0A0B0C0D, 4661, sx);

        // HighID → callback possible for any LowID.
        let high = ctx(true, true, false);
        assert!(can_do_callback(&local, &high));
        assert!(can_do_callback(&foreign, &high));

        // LowID + no Kad → nobody can reach us.
        let low_nokad = ctx(true, false, false);
        assert!(!can_do_callback(&local, &low_nokad));
        assert!(!can_do_callback(&foreign, &low_nokad));

        // LowID + Kad open → foreign lowids only (same-server is banned).
        let low_kad = ctx(true, false, true);
        assert!(!can_do_callback(&local, &low_kad));
        assert!(can_do_callback(&foreign, &low_kad));

        // No server + Kad open → everything is foreign → allowed.
        let kad_only = ctx(false, false, true);
        assert!(can_do_callback(&local, &kad_only));
        assert!(can_do_callback(&foreign, &kad_only));
    }
}
