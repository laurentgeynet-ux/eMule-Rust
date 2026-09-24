// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use md4::{Digest, Md4};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;

use mule_crypto::cryptkey::{self, LegacyKeyLoad};
use mule_crypto::legacy_sui::EMULE_RSA_BITS;

/// Shared SUI counters for anomaly observability — one instance per daemon,
/// incremented by the inbound listener and outbound peer sessions, exposed
/// read-only via `identity.status` RPC. Lock-free atomics, never reset.
#[derive(Debug, Default)]
pub struct SuiStats {
    /// OP_SECIDENTSTATE challenges sent to inbound peers (we verify them).
    pub challenges_issued_in: AtomicU64,
    /// OP_SECIDENTSTATE challenges sent outbound (we verify the peer).
    pub challenges_issued_out: AtomicU64,
    /// OP_SIGNATURE sent outbound answering a peer challenge.
    pub signatures_sent: AtomicU64,
    /// Peer signatures that passed RSA verification (inbound + outbound).
    pub signatures_verified: AtomicU64,
    /// Peer signatures rejected for any reason (bad RSA, wrong challenge, ...).
    pub signatures_rejected: AtomicU64,
    /// Rejections specifically due to the one-signature-per-IP anti-flood.
    pub signatures_rejected_flood: AtomicU64,
    /// Malformed/oversized SUI packets dropped before verification.
    pub malformed_packets: AtomicU64,
}

/// Serializable snapshot of `SuiStats` for RPC.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SuiStatsSnapshot {
    pub challenges_issued_in: u64,
    pub challenges_issued_out: u64,
    pub signatures_sent: u64,
    pub signatures_verified: u64,
    pub signatures_rejected: u64,
    pub signatures_rejected_flood: u64,
    pub malformed_packets: u64,
}

impl SuiStats {
    pub fn snapshot(&self) -> SuiStatsSnapshot {
        use std::sync::atomic::Ordering::Relaxed;
        SuiStatsSnapshot {
            challenges_issued_in: self.challenges_issued_in.load(Relaxed),
            challenges_issued_out: self.challenges_issued_out.load(Relaxed),
            signatures_sent: self.signatures_sent.load(Relaxed),
            signatures_verified: self.signatures_verified.load(Relaxed),
            signatures_rejected: self.signatures_rejected.load(Relaxed),
            signatures_rejected_flood: self.signatures_rejected_flood.load(Relaxed),
            malformed_packets: self.malformed_packets.load(Relaxed),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IdentityStatus {
    pub enabled: bool,
    pub has_key: bool,
    pub pubkey_hash: Option<String>,
    pub key_bits: Option<u32>,
    /// Set when `cryptkey.dat` was unreadable/corrupt and quarantined.
    pub quarantined_key: Option<String>,
}

/// `userhash.dat` — imported eMule `m_abyUserHash` (16 raw bytes), kept
/// separate from `cryptkey.dat` like eMule keeps it in `preferences.dat`.
const USERHASH_FILENAME: &str = "userhash.dat";

/// Outcome of `import_emule_identity` — eMule `cryptkey.dat` +
/// `preferences.dat` migration.
#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityImportReport {
    /// `cryptkey.dat` imported (RSA-384 private key, credit continuity).
    pub key_imported: bool,
    /// `preferences.dat` user hash imported and activated.
    pub userhash_imported: bool,
    /// Active user hash after import (hex).
    pub userhash: Option<String>,
}

/// Service managing the daemon's legacy eMule SUI identity.
///
/// The private key is a real RSA-384 key stored in `cryptkey.dat`
/// (Base64(PKCS#1 DER), eMule format). The public key wire form is the
/// PKCS#1 `RSAPublicKey` DER sequence (<= 80 bytes) sent in `OP_PUBLICKEY`.
///
/// The private key never leaves this service: callers may only request
/// signatures via `sign_sui_payload`.
#[derive(Clone)]
pub struct IdentityService {
    key_path: PathBuf,
    /// RSA-384 private key; `None` if the stored key was corrupt (quarantined).
    legacy_key: Option<rsa::RsaPrivateKey>,
    /// PKCS#1 DER encoding of the public key (wire form).
    public_key: Vec<u8>,
    pubkey_hash: String,
    quarantined_key: Option<String>,
    /// Imported eMule user hash (`preferences.dat`); when present it wins
    /// over the derived `MD4(pubkey)` hash — eMule credits are keyed on it.
    userhash_override: Option<[u8; 16]>,
}

// Debug intentionally omits private key material.
impl std::fmt::Debug for IdentityService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IdentityService")
            .field("key_path", &self.key_path)
            .field("has_key", &self.legacy_key.is_some())
            .field("pubkey_hash", &self.pubkey_hash)
            .finish()
    }
}

impl IdentityService {
    /// Loads an existing `cryptkey.dat` or generates a new RSA-384 key.
    ///
    /// A corrupt key file is quarantined (`cryptkey.dat.corrupt-<ts>`) and the
    /// service runs with SUI disabled rather than silently destroying the
    /// user's historical identity and credits.
    pub fn load_or_generate(data_dir: &Path) -> Self {
        let key_path = data_dir.join(cryptkey::CRYPTKEY_FILENAME);
        let mut quarantined_key = None;

        let legacy_key = match cryptkey::load_cryptkey(data_dir) {
            LegacyKeyLoad::Loaded(key) => {
                tracing::info!("[SUI] Loaded legacy RSA-384 key from {:?}", key_path);
                Some(*key)
            }
            LegacyKeyLoad::Missing => match cryptkey::generate_cryptkey(data_dir) {
                Ok(key) => {
                    tracing::info!("[SUI] Generated new RSA-384 keypair at {:?}", key_path);
                    Some(key)
                }
                Err(e) => {
                    tracing::error!("[SUI] Failed to generate cryptkey.dat: {}", e);
                    None
                }
            },
            LegacyKeyLoad::Invalid {
                quarantine_path,
                reason,
            } => {
                tracing::error!(
                    "[SUI] cryptkey.dat invalid ({}), quarantined to {:?} — SUI disabled, identity NOT regenerated",
                    reason, quarantine_path
                );
                quarantined_key = Some(quarantine_path.display().to_string());
                None
            }
            LegacyKeyLoad::LegacyBlob { quarantine_path } => {
                // Pre-RSA 80-byte random identity blob from an old daemon build:
                // no key material to preserve — safe to migrate to a real key.
                quarantined_key = Some(quarantine_path.display().to_string());
                match cryptkey::generate_cryptkey(data_dir) {
                    Ok(key) => {
                        tracing::warn!(
                            "[SUI] Migrated legacy 80-byte identity blob to {:?} — generated new RSA-384 keypair at {:?}",
                            quarantine_path, key_path
                        );
                        Some(key)
                    }
                    Err(e) => {
                        tracing::error!("[SUI] Failed to generate cryptkey.dat: {}", e);
                        None
                    }
                }
            }
        };

        let public_key = legacy_key.as_ref().map(public_key_wire).unwrap_or_default();
        let pubkey_hash = hex::encode(Md4::digest(&public_key));
        let userhash_override = Self::load_userhash_override(data_dir);

        tracing::info!(
            "[SUI] Identity ready (sui={}, pubkey md4={}...)",
            legacy_key.is_some(),
            &pubkey_hash[..16.min(pubkey_hash.len())]
        );

        Self {
            key_path,
            legacy_key,
            public_key,
            pubkey_hash,
            quarantined_key,
            userhash_override,
        }
    }

    /// Reads `data_dir/userhash.dat` (16 raw bytes). A degenerate or
    /// malformed file is ignored with a warning — never destroyed, the
    /// user may still hold a copy of their eMule `preferences.dat`.
    fn load_userhash_override(data_dir: &Path) -> Option<[u8; 16]> {
        let path = data_dir.join(USERHASH_FILENAME);
        let raw = std::fs::read(&path).ok()?;
        let Ok(bytes) = <[u8; 16]>::try_from(raw.as_slice()) else {
            tracing::warn!(
                "[SUI] {:?} malformed ({} bytes, expected 16) — ignored",
                path,
                raw.len()
            );
            return None;
        };
        if mule_format::met::is_bad_hash(&bytes) {
            tracing::warn!("[SUI] {:?} holds a degenerate user hash — ignored", path);
            return None;
        }
        tracing::info!("[SUI] Imported eMule user hash loaded from {:?}", path);
        Some(bytes)
    }

    /// Generates a fresh `cryptkey.dat`, replacing the current key.
    /// All credits tied to the previous identity become unclaimable —
    /// including an imported eMule user hash, removed with the old key.
    pub fn rotate_key(data_dir: &Path) -> std::io::Result<Self> {
        let key_path = data_dir.join(cryptkey::CRYPTKEY_FILENAME);
        let userhash_path = data_dir.join(USERHASH_FILENAME);
        let key = cryptkey::generate_cryptkey(data_dir).map_err(std::io::Error::other)?;
        // New identity ⇒ the imported `preferences.dat` hash no longer
        // applies (it belongs to the eMule key we just replaced).
        if userhash_path.exists() {
            let _ = std::fs::remove_file(&userhash_path);
        }
        let public_key = public_key_wire(&key);
        let pubkey_hash = hex::encode(Md4::digest(&public_key));
        tracing::warn!(
            "[SUI] Identity rotated at {:?} (new pubkey md4={}...) — previous credits no longer claimable",
            key_path, &pubkey_hash[..16]
        );
        Ok(Self {
            key_path,
            legacy_key: Some(key),
            public_key,
            pubkey_hash,
            quarantined_key: None,
            userhash_override: None,
        })
    }

    /// Imports an eMule identity: `cryptkey.dat` (RSA-384 private key)
    /// and/or the `preferences.dat` user hash. Both files preserve the
    /// credits peers granted to that identity — eMule keys credits on
    /// `m_abyUserHash` and verifies them with the SUI pubkey.
    ///
    /// `replace` is required when the destination files already exist —
    /// overwriting an identity discards the credits tied to it.
    pub fn import_emule_identity(
        data_dir: &Path,
        cryptkey_src: Option<&Path>,
        preferences_src: Option<&Path>,
        replace: bool,
    ) -> Result<IdentityImportReport, crate::error::DaemonError> {
        use crate::error::DaemonError;
        let key_path = data_dir.join(cryptkey::CRYPTKEY_FILENAME);
        let userhash_path = data_dir.join(USERHASH_FILENAME);

        let mut report = IdentityImportReport::default();

        if let Some(src) = cryptkey_src {
            if key_path.exists() && !replace {
                return Err(DaemonError::Business(
                    "cryptkey.dat already exists — pass replace:true to overwrite the current identity"
                        .to_string(),
                ));
            }
            // Validate before touching anything — eMule `cryptkey.dat` is
            // `Base64(PKCS#1 DER)` RSA-384.
            let raw = std::fs::read(src)?;
            let key = cryptkey::parse_cryptkey_bytes(&raw)
                .map_err(|e| DaemonError::Business(format!("cryptkey.dat: {e}")))?;
            cryptkey::save_cryptkey(data_dir, &key)
                .map_err(|e| DaemonError::Business(format!("writing cryptkey.dat: {e}")))?;
            report.key_imported = true;
        }

        if let Some(src) = preferences_src {
            if userhash_path.exists() && !replace {
                return Err(DaemonError::Business(
                    "userhash.dat already exists — pass replace:true to overwrite the imported user hash"
                        .to_string(),
                ));
            }
            let prefs = mule_format::met::PreferencesDat::open(src)
                .map_err(|e| DaemonError::Business(format!("preferences.dat: {e}")))?;
            std::fs::create_dir_all(data_dir)?;
            let tmp = data_dir.join(format!("{USERHASH_FILENAME}.tmp"));
            {
                use std::io::Write;
                let mut f = std::fs::File::create(&tmp)?;
                f.write_all(&prefs.user_hash)?;
                f.sync_all()?;
            }
            std::fs::rename(&tmp, &userhash_path)?;
            report.userhash_imported = true;
            report.userhash = Some(hex::encode(prefs.user_hash));
        }

        Ok(report)
    }

    /// Whether a usable legacy RSA private key is loaded.
    pub fn has_key(&self) -> bool {
        self.legacy_key.is_some()
    }

    /// Returns the path to the stored cryptkey file.
    pub fn key_path(&self) -> &Path {
        &self.key_path
    }

    /// Returns the PKCS#1 DER public key bytes for eD2K `OP_PUBLICKEY` transmission.
    pub fn public_key_bytes(&self) -> &[u8] {
        &self.public_key
    }

    /// Returns the MD4 hash of the public key (32 hex characters).
    pub fn pubkey_hash(&self) -> &str {
        &self.pubkey_hash
    }

    /// Signs a SUI payload (`pubkey || challenge [|| ip || kind]`) with the
    /// legacy RSA-384 key (RSASSA-PKCS1v15-SHA1). Fails if no key is loaded.
    pub fn sign_sui_payload(
        &self,
        payload: &[u8],
    ) -> Result<Vec<u8>, mule_crypto::legacy_sui::LegacySuiError> {
        let key = self.legacy_key.as_ref().ok_or_else(|| {
            mule_crypto::legacy_sui::LegacySuiError::Rsa(rsa::Error::Verification)
        })?;
        mule_crypto::legacy_sui::sign_sui_payload(key, payload)
    }

    /// Returns the 16-byte user hash.
    /// Order (`Preferences.cpp:1484-1492`): the imported eMule
    /// `preferences.dat` hash wins verbatim — it already carries the
    /// marker bytes and it is the key peers store credits under.
    /// Otherwise the derived `MD4(pubkey)` hash marked with eMule magic
    /// bytes (a rotated key detaches credits).
    pub fn user_hash(&self) -> [u8; 16] {
        if let Some(imported) = self.userhash_override {
            return imported;
        }
        let hash = Md4::digest(&self.public_key);
        let mut out = [0u8; 16];
        out.copy_from_slice(&hash[..16]);
        // Official eMule client identification magic bytes (Preferences.cpp:1491-1492)
        out[5] = 14; // 0x0E
        out[14] = 111; // 0x6F
        out
    }

    /// Returns the public identity status object.
    pub fn status(&self) -> IdentityStatus {
        IdentityStatus {
            enabled: self.legacy_key.is_some(),
            has_key: self.legacy_key.is_some(),
            pubkey_hash: Some(self.pubkey_hash.clone()),
            key_bits: self.legacy_key.as_ref().map(|_| EMULE_RSA_BITS as u32),
            quarantined_key: self.quarantined_key.clone(),
        }
    }
}

/// PKCS#1 DER (`SEQUENCE { n INTEGER, e INTEGER }`) — same bytes as CryptoPP
/// `GetMaterial().Save()` used for the eMule wire public key.
fn public_key_wire(key: &rsa::RsaPrivateKey) -> Vec<u8> {
    use rsa::pkcs1::EncodeRsaPublicKey;
    rsa::RsaPublicKey::from(key)
        .to_pkcs1_der()
        .map(|d| d.as_bytes().to_vec())
        .unwrap_or_default()
}
