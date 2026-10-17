// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Garde anti-SSRF pour les téléchargements HTTP(S) déclenchés par RPC
//! (`servers.updateFromUrl`, `kad.bootstrapFromUrl`).
//!
//! La validation se fait en deux temps :
//! 1. [`parse_fetch_url`] — syntaxe : schéma http/https uniquement, hôte
//!    présent, pas de userinfo (`http://user@host/` est refusé).
//! 2. [`validate_fetch_ips`] — chaque IP (littérale ou issue du DNS) est
//!    classée puis confrontée au profil réseau, exactement comme
//!    `validate_connect_address` : `LocalLab` n'accepte que le loopback,
//!    les profils publics n'acceptent que `IpCategory::PublicRoutable`.
//!
//! L'appelant doit ensuite **épingler** les adresses validées dans le
//! client HTTP (reqwest `resolve_to`) et **interdire les redirections** :
//! sans cela, une URL validée peut déléguer vers une cible interne.

use std::net::IpAddr;
use thiserror::Error;
use url::Url;

use crate::profile::NetworkProfile;
use crate::validator::{classify_ip, IpCategory};

/// Port HTTP implicite quand l'URL n'en précise pas.
pub const DEFAULT_HTTP_PORT: u16 = 80;
/// Port HTTPS implicite quand l'URL n'en précise pas.
pub const DEFAULT_HTTPS_PORT: u16 = 443;

/// Erreurs de validation d'une URL de fetch externe.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FetchPolicyError {
    /// URL syntaxiquement invalide ou sans hôte.
    #[error("Invalid fetch URL: {0}")]
    InvalidUrl(String),

    /// Schéma autre que http/https (file://, gopher://, …).
    #[error("Forbidden URL scheme '{0}' — only http/https are allowed")]
    ForbiddenScheme(String),

    /// URL contenant un userinfo (`http://user:pass@host/`).
    #[error("Userinfo in fetch URL is prohibited")]
    UserinfoDisallowed,

    /// Aucune IP n'a été fournie pour un hôte non littéral.
    #[error("Host '{0}' resolved to no IP address")]
    UnresolvableHost(String),

    /// Au moins une IP résolue viole la politique du profil courant.
    #[error("Resolved address {ip} for '{host}' is prohibited: {reason}")]
    AddressViolation {
        /// Hôte demandé.
        host: String,
        /// IP résolue refusée.
        ip: IpAddr,
        /// Catégorie d'adresse refusée.
        reason: &'static str,
    },
}

/// URL de fetch normalisée et validée syntaxiquement.
#[derive(Debug, Clone)]
pub struct FetchUrl {
    /// URL d'origine re-parsée (telle qu'envoyée au client HTTP).
    pub url: Url,
    /// Hôte canonique (sans crochets IPv6).
    pub host: String,
    /// Port effectif (explicite ou 80/443 par défaut).
    pub port: u16,
    /// IP littérale si l'hôte en est une — court-circuite le DNS.
    pub literal_ip: Option<IpAddr>,
}

/// Analyse et valide syntaxiquement une URL de fetch externe.
pub fn parse_fetch_url(raw: &str) -> Result<FetchUrl, FetchPolicyError> {
    let url = Url::parse(raw.trim())
        .map_err(|e| FetchPolicyError::InvalidUrl(format!("{raw}: {e}")))?;

    match url.scheme() {
        "http" | "https" => {}
        other => return Err(FetchPolicyError::ForbiddenScheme(other.to_string())),
    }

    if !url.username().is_empty() || url.password().is_some() {
        return Err(FetchPolicyError::UserinfoDisallowed);
    }

    let host = url
        .host_str()
        .ok_or_else(|| FetchPolicyError::InvalidUrl(format!("{raw}: missing host")))?
        .to_string();

    let port = url.port_or_known_default().unwrap_or(match url.scheme() {
        "https" => DEFAULT_HTTPS_PORT,
        _ => DEFAULT_HTTP_PORT,
    });

    let literal_ip = match url.host() {
        Some(url::Host::Ipv4(v4)) => Some(IpAddr::V4(v4)),
        Some(url::Host::Ipv6(v6)) => Some(IpAddr::V6(v6)),
        _ => None,
    };

    Ok(FetchUrl {
        url,
        host,
        port,
        literal_ip,
    })
}

/// Vérifie que chaque IP candidate est autorisée par le profil courant.
///
/// `resolved` = adresses DNS du moment ; ignoré quand `target` porte une
/// IP littérale. Toutes les adresses doivent passer : une seule entrée
/// DNS publique + une entrée privée suffit à rejeter (DNS round-robin
/// pourrait sinon atteindre l'hôte interne).
pub fn validate_fetch_ips(
    target: &FetchUrl,
    resolved: &[IpAddr],
    profile: NetworkProfile,
) -> Result<(), FetchPolicyError> {
    let candidates: Vec<IpAddr> = match target.literal_ip {
        Some(ip) => vec![ip],
        None => {
            if resolved.is_empty() {
                return Err(FetchPolicyError::UnresolvableHost(target.host.clone()));
            }
            resolved.to_vec()
        }
    };

    for ip in &candidates {
        let cat = classify_ip(*ip);
        let allowed = match profile {
            NetworkProfile::LocalLab => cat == IpCategory::Loopback,
            NetworkProfile::PublicEd2kPreview | NetworkProfile::PublicEd2kKadPreview => {
                cat == IpCategory::PublicRoutable
            }
        };
        if !allowed {
            return Err(FetchPolicyError::AddressViolation {
                host: target.host.clone(),
                ip: *ip,
                reason: category_reason(cat),
            });
        }
    }
    Ok(())
}

/// Libellé court de la catégorie d'adresse pour les messages d'erreur.
fn category_reason(cat: IpCategory) -> &'static str {
    match cat {
        IpCategory::Loopback => "loopback",
        IpCategory::PrivateLan => "private LAN",
        IpCategory::LinkLocal => "link-local",
        IpCategory::Multicast => "multicast",
        IpCategory::Broadcast => "broadcast",
        IpCategory::Unspecified => "unspecified",
        IpCategory::CarrierGradeNat => "carrier-grade NAT",
        IpCategory::SpecialPurpose => "special-purpose/reserved",
        IpCategory::PublicRoutable => "public",
    }
}

/// Adresses à épingler dans le client HTTP après validation :
/// la liste validée telle quelle, ou l'IP littérale seule.
pub fn pinned_ips(target: &FetchUrl, resolved: &[IpAddr]) -> Vec<IpAddr> {
    match target.literal_ip {
        Some(ip) => vec![ip],
        None => resolved.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn parse_rejects_non_http_schemes() {
        for raw in [
            "file:///c:/windows/win.ini",
            "gopher://host/x",
            "ftp://host/x",
            "ed2k://|server|1.2.3.4|4661|/",
        ] {
            // Rejet garanti — `InvalidUrl` (syntaxe non-URL comme les
            // liens ed2k) ou `ForbiddenScheme` selon la parsabilité.
            assert!(parse_fetch_url(raw).is_err(), "{raw} must be rejected");
        }
        assert!(matches!(
            parse_fetch_url("file:///c:/windows/win.ini"),
            Err(FetchPolicyError::ForbiddenScheme(_))
        ));
    }

    #[test]
    fn parse_rejects_userinfo() {
        assert!(matches!(
            parse_fetch_url("http://user@1.2.3.4/server.met"),
            Err(FetchPolicyError::UserinfoDisallowed)
        ));
        assert!(matches!(
            parse_fetch_url("http://user:pass@1.2.3.4/x"),
            Err(FetchPolicyError::UserinfoDisallowed)
        ));
    }

    #[test]
    fn parse_defaults_ports() {
        let http = parse_fetch_url("http://upd.emule-security.org/server.met").unwrap();
        assert_eq!(http.port, DEFAULT_HTTP_PORT);
        assert_eq!(http.host, "upd.emule-security.org");
        let https = parse_fetch_url("https://example.com:8443/nodes.dat").unwrap();
        assert_eq!(https.port, 8443);
    }

    #[test]
    fn literal_ip_detected() {
        let t = parse_fetch_url("http://169.254.169.254/latest/meta-data").unwrap();
        assert_eq!(t.literal_ip, Some(IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254))));
        let v6 = parse_fetch_url("http://[::1]/x").unwrap();
        assert_eq!(v6.literal_ip, Some(IpAddr::V6(Ipv6Addr::LOCALHOST)));
    }

    #[test]
    fn locallab_accepts_only_loopback() {
        let loop_url = parse_fetch_url("http://127.0.0.1:4661/server.met").unwrap();
        assert!(validate_fetch_ips(&loop_url, &[], NetworkProfile::LocalLab).is_ok());

        let pub_url = parse_fetch_url("http://8.8.8.8/x").unwrap();
        assert!(matches!(
            validate_fetch_ips(&pub_url, &[], NetworkProfile::LocalLab),
            Err(FetchPolicyError::AddressViolation { .. })
        ));

        let priv_url = parse_fetch_url("http://192.168.1.1/x").unwrap();
        assert!(validate_fetch_ips(&priv_url, &[], NetworkProfile::LocalLab).is_err());
    }

    #[test]
    fn public_profile_rejects_internal_targets() {
        let prof = NetworkProfile::PublicEd2kKadPreview;
        for raw in [
            "http://169.254.169.254/latest/meta-data",
            "http://192.168.1.1/server.met",
            "http://10.0.0.5/x",
            "http://127.0.0.1/x",
            "http://[fd00::1]/x",
            "http://100.64.1.1/x",
        ] {
            let t = parse_fetch_url(raw).unwrap();
            assert!(
                validate_fetch_ips(&t, &[], prof).is_err(),
                "{raw} should be rejected in public profile"
            );
        }
        let ok = parse_fetch_url("http://8.8.8.8/server.met").unwrap();
        assert!(validate_fetch_ips(&ok, &[], prof).is_ok());
    }

    #[test]
    fn hostname_requires_nonempty_resolutions() {
        let t = parse_fetch_url("http://example.org/x").unwrap();
        assert!(matches!(
            validate_fetch_ips(&t, &[], NetworkProfile::PublicEd2kKadPreview),
            Err(FetchPolicyError::UnresolvableHost(h)) if h == "example.org"
        ));
    }

    #[test]
    fn single_bad_resolved_ip_rejects_whole_host() {
        let t = parse_fetch_url("http://evil.example/x").unwrap();
        let mixed = [
            IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)),
            IpAddr::V4(Ipv4Addr::new(192, 168, 0, 1)),
        ];
        assert!(matches!(
            validate_fetch_ips(&t, &mixed, NetworkProfile::PublicEd2kKadPreview),
            Err(FetchPolicyError::AddressViolation { ip, .. }) if ip == IpAddr::V4(Ipv4Addr::new(192, 168, 0, 1))
        ));
    }
}
