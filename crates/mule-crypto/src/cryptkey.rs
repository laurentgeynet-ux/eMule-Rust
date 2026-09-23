// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! `cryptkey.dat` persistence — eMule-compatible RSA private key file.
//!
//! Confirmed against eMule `ClientCredits.cpp:474` / aMule `ClientCreditsList.cpp:CreateKeyPair`:
//! the file contains the Crypto++ `InvertibleRSAFunction` private key, DER-encoded
//! (i.e. PKCS#1 `RSAPrivateKey` = `SEQUENCE { v, n, e, d, p, q, dp, dq, qInv }`)
//! and wrapped in Base64 text. Line breaks are tolerated on read.
//!
//! Safety rules:
//! - A corrupt file is never overwritten: it is quarantined with a timestamped
//!   suffix before any regeneration, preserving historical credits recovery.
//! - Writes are atomic (temp file + `sync_all` + rename).

use std::path::{Path, PathBuf};

use rsa::pkcs1::{DecodeRsaPrivateKey, EncodeRsaPrivateKey};
use rsa::RsaPrivateKey;

/// Name used by eMule/aMule for the private key file.
pub const CRYPTKEY_FILENAME: &str = "cryptkey.dat";

#[derive(Debug)]
pub enum LegacyKeyLoad {
    /// Existing key file parsed successfully.
    Loaded(Box<RsaPrivateKey>),
    /// No key file present.
    Missing,
    /// File exists but could not be parsed; it was moved to quarantine.
    Invalid {
        quarantine_path: PathBuf,
        reason: CryptkeyError,
    },
    /// File contained the pre-RSA 80-byte random blob written by old daemon
    /// versions. It holds no recoverable key material; it was quarantined and
    /// the caller should generate a fresh key (this is a migration, not
    /// corruption of a real key).
    LegacyBlob { quarantine_path: PathBuf },
}

#[derive(Debug, thiserror::Error)]
pub enum CryptkeyError {
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("base64 decode failed: {0}")]
    Base64(#[from] data_encoding::DecodeError),
    #[error("pkcs1 der decode failed: {0}")]
    Pkcs1(#[from] rsa::pkcs1::Error),
    #[error("key validation failed: {0}")]
    Rsa(#[from] rsa::Error),
    #[error("key size is {0} bits, expected {EMULE_RSA_BITS}")]
    WrongKeySize(usize),
}

use crate::legacy_sui::EMULE_RSA_BITS;

/// Decodes a `cryptkey.dat` byte buffer (Base64-wrapped PKCS#1 DER).
/// All ASCII whitespace inside the file is ignored.
pub fn parse_cryptkey_bytes(raw: &[u8]) -> Result<RsaPrivateKey, CryptkeyError> {
    let compact: Vec<u8> = raw
        .iter()
        .copied()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();
    let der = data_encoding::BASE64.decode(&compact)?;
    let key = RsaPrivateKey::from_pkcs1_der(&der)?;
    key.validate()?;
    let bits = rsa::traits::PublicKeyParts::n(&key).bits();
    if bits != EMULE_RSA_BITS {
        return Err(CryptkeyError::WrongKeySize(bits));
    }
    Ok(key)
}

/// Serializes a private key in the eMule file format: `Base64(PKCS#1 DER)`,
/// wrapped at 64 columns like CryptoPP `Base64Encoder` output.
pub fn serialize_cryptkey_bytes(key: &RsaPrivateKey) -> Result<Vec<u8>, CryptkeyError> {
    let der = key.to_pkcs1_der()?;
    let b64 = data_encoding::BASE64.encode(der.as_bytes());
    let mut out = Vec::with_capacity(b64.len() + b64.len() / 64 + 2);
    for chunk in b64.as_bytes().chunks(64) {
        out.extend_from_slice(chunk);
        out.push(b'\n');
    }
    Ok(out)
}

/// Loads `cryptkey.dat` from `data_dir`.
///
/// - absent → `Missing` (caller decides whether to generate)
/// - valid  → `Loaded`
/// - invalid → file is renamed to `cryptkey.dat.corrupt-<unix_ts>` and
///   `Invalid` is returned; the caller must not silently regenerate.
pub fn load_cryptkey(data_dir: &Path) -> LegacyKeyLoad {
    let path = data_dir.join(CRYPTKEY_FILENAME);
    let raw = match std::fs::read(&path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return LegacyKeyLoad::Missing,
        Err(e) => {
            let quarantine_path = quarantine_path_for(&path);
            let _ = std::fs::rename(&path, &quarantine_path);
            return LegacyKeyLoad::Invalid {
                quarantine_path,
                reason: e.into(),
            };
        }
    };
    match parse_cryptkey_bytes(&raw) {
        Ok(key) => LegacyKeyLoad::Loaded(Box::new(key)),
        Err(reason) => {
            let quarantine_path = quarantine_path_for(&path);
            let _ = std::fs::rename(&path, &quarantine_path);
            if is_legacy_blob(&raw) {
                return LegacyKeyLoad::LegacyBlob { quarantine_path };
            }
            LegacyKeyLoad::Invalid {
                quarantine_path,
                reason,
            }
        }
    }
}

/// Old daemon versions stored an 80-byte random blob in `cryptkey.dat`
/// instead of a real RSA key. Detect it: exactly 80 bytes and not printable
/// Base64 text.
fn is_legacy_blob(raw: &[u8]) -> bool {
    raw.len() == 80
        && !raw.iter().all(|b| {
            b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'=') || b.is_ascii_whitespace()
        })
}

/// Atomically writes `cryptkey.dat` in `data_dir`.
pub fn save_cryptkey(data_dir: &Path, key: &RsaPrivateKey) -> Result<(), CryptkeyError> {
    std::fs::create_dir_all(data_dir)?;
    let bytes = serialize_cryptkey_bytes(key)?;
    let target = data_dir.join(CRYPTKEY_FILENAME);
    let tmp = data_dir.join(format!("{CRYPTKEY_FILENAME}.tmp"));
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(&bytes)?;
        f.sync_all()?;
    } // close handle before rename (required on Windows)
    std::fs::rename(&tmp, &target)?;
    Ok(())
}

/// Generates a fresh RSA-384 key and persists it atomically.
pub fn generate_cryptkey(data_dir: &Path) -> Result<RsaPrivateKey, CryptkeyError> {
    let key = RsaPrivateKey::new(&mut rsa::rand_core::OsRng, EMULE_RSA_BITS)?;
    save_cryptkey(data_dir, &key)?;
    Ok(key)
}

fn quarantine_path_for(path: &Path) -> PathBuf {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    path.with_file_name(format!("cryptkey.dat.corrupt-{ts}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsa::traits::{PrivateKeyParts, PublicKeyParts};

    #[test]
    fn cryptkey_roundtrip_preserves_key() {
        let dir = tempfile::tempdir().unwrap();
        let key = generate_cryptkey(dir.path()).unwrap();
        match load_cryptkey(dir.path()) {
            LegacyKeyLoad::Loaded(loaded) => {
                assert_eq!(loaded.n(), key.n());
                assert_eq!(loaded.e(), key.e());
                assert_eq!(loaded.d(), key.d());
            }
            _ => panic!("expected Loaded"),
        }
        // File content must be Base64 text (eMule compatible), not raw DER.
        let raw = std::fs::read(dir.path().join(CRYPTKEY_FILENAME)).unwrap();
        assert!(raw.iter().all(|b| b.is_ascii_alphanumeric()
            || matches!(b, b'+' | b'/' | b'=')
            || b.is_ascii_whitespace()));
    }

    #[test]
    fn cryptkey_missing_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(load_cryptkey(dir.path()), LegacyKeyLoad::Missing));
    }

    #[test]
    fn cryptkey_corrupt_is_quarantined_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(CRYPTKEY_FILENAME);
        std::fs::write(&path, b"not-base64!!!").unwrap();
        match load_cryptkey(dir.path()) {
            LegacyKeyLoad::Invalid {
                quarantine_path, ..
            } => {
                assert!(quarantine_path.exists());
                assert_eq!(std::fs::read(&quarantine_path).unwrap(), b"not-base64!!!");
                assert!(!path.exists());
            }
            _ => panic!("expected Invalid"),
        }
    }

    #[test]
    fn cryptkey_legacy_blob_detected_and_quarantined() {
        // Pre-RSA daemon versions wrote an 80-byte random blob (non-text).
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(CRYPTKEY_FILENAME);
        let blob: Vec<u8> = (0u8..80)
            .map(|i| i.wrapping_mul(37).wrapping_add(11))
            .collect();
        assert!(is_legacy_blob(&blob));
        std::fs::write(&path, &blob).unwrap();
        match load_cryptkey(dir.path()) {
            LegacyKeyLoad::LegacyBlob { quarantine_path } => {
                assert!(quarantine_path.exists());
                assert_eq!(std::fs::read(&quarantine_path).unwrap(), blob);
                assert!(!path.exists());
            }
            _ => panic!("expected LegacyBlob"),
        }
    }

    #[test]
    fn cryptkey_80_byte_text_file_is_invalid_not_legacy() {
        // An 80-byte file that *is* Base64-alphabet text is not the legacy
        // binary blob — treat as ordinary corruption.
        let raw = vec![b'A'; 80];
        assert!(!is_legacy_blob(&raw));
    }

    #[test]
    fn cryptkey_wrong_size_key_rejected() {
        // RSA-512 key must not be accepted as an eMule SUI key.
        let key = RsaPrivateKey::new(&mut rsa::rand_core::OsRng, 512).unwrap();
        let bytes = serialize_cryptkey_bytes(&key).unwrap();
        let err = parse_cryptkey_bytes(&bytes).unwrap_err();
        assert!(matches!(err, CryptkeyError::WrongKeySize(512)));
    }
}
