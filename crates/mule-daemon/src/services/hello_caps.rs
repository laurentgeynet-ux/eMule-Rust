//! Construction du mot de capacités `CT_EMULE_MISCOPTIONS2` — source unique
//! pour les hellos eD2K (entrant, sortant, connect-back).
//!
//! Layout eMule `BaseClient.cpp:1011` :
//! `FileIdentifiers<<13 | DirectUDPCallback<<12 | SupportsCaptcha<<11 |
//!  SourceEx2<<10 | RequiresCrypt<<9 | RequestsCrypt<<8 | SupportsCrypt<<7 |
//!  reserved<<6 | ExtMultiPacket<<5 | LargeFiles<<4 | KadVersion<<0`.
//!
//! Bit 11 (chat CAPTCHA) reste à 0 : la réponse CAPTCHA n'est pas implémentée.

/// Construit le mot `CT_EMULE_MISCOPTIONS2` d'un hello eMule.
///
/// - `tcp_low_id` : vrai quand l'identité eD2K courante est un LowID
///   (`client_id < 0x0100_0000`), proxy local de `theApp.IsFirewalled()` /
///   `CKademlia::IsFirewalled()` côté TCP.
/// - `udp_verified_open` : vrai quand le testeur de pare-feu UDP Kad a vérifié
///   que l'UDP entrant est joignable (`verified && status == Open` —
///   équivalent `!IsFirewalledUDP(true) && IsVerified()`, full-cone NAT).
///   Combiné au LowID, arme le bit 12 « DirectUDPCallback » comme
///   `GetMyConnectOptions(true, true)`.
pub(crate) fn misc_options2(
    tcp_low_id: bool,
    udp_verified_open: bool,
    obfuscation_enabled: bool,
    obfuscation_required: bool,
) -> u32 {
    let u_direct_udp_cb = (tcp_low_id && udp_verified_open) as u32;
    (1 << 13) // FileIdentifiers / MultipacketExt2 — codecs implémentés
        | (u_direct_udp_cb << 12)
        | (1 << 10) // SourceExchange2
        | ((obfuscation_required as u32) << 9)
        | ((obfuscation_enabled as u32) << 8)
        | ((obfuscation_enabled as u32) << 7)
        | (1 << 5) // ExtMultiPacket
        | (1 << 4) // LargeFiles / tags 64 bits
        | crate::services::kad::firewall::OUR_KAD_VERSION as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_bits_always_present() {
        let m = misc_options2(false, false, false, false);
        assert_eq!(m & (1 << 13), 1 << 13, "FileIdent");
        assert_eq!(m & (1 << 10), 1 << 10, "SrcExch2");
        assert_eq!(m & (1 << 5), 1 << 5, "ExtMultiPacket");
        assert_eq!(m & (1 << 4), 1 << 4, "LargeFiles");
        assert_eq!(
            m & 0x0F,
            crate::services::kad::firewall::OUR_KAD_VERSION as u32,
            "KadVersion"
        );
        assert_eq!(m & (1 << 11), 0, "Captcha non implémenté");
        assert_eq!(m & (1 << 12), 0, "pas de callback direct en HighID");
    }

    #[test]
    fn direct_callback_requires_lowid_and_verified_udp() {
        // TCP injoignable + UDP vérifié ouvert → bit 12 (eMule GetMyConnectOptions).
        assert_ne!(
            misc_options2(true, true, false, false) & (1 << 12),
            0
        );
        // UDP non vérifié ou fermé → pas de bit 12.
        assert_eq!(misc_options2(true, false, false, false) & (1 << 12), 0);
        // UDP ouvert mais TCP joignable → pas de bit 12.
        assert_eq!(misc_options2(false, true, false, false) & (1 << 12), 0);
    }

    #[test]
    fn crypt_bits_follow_obfuscation_flags() {
        let m = misc_options2(false, false, true, true);
        assert_ne!(m & (1 << 7), 0, "SupportsCryptLayer");
        assert_ne!(m & (1 << 8), 0, "RequestsCryptLayer");
        assert_ne!(m & (1 << 9), 0, "RequiresCryptLayer");
    }
}
