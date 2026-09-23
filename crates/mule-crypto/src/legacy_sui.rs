// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

//! Legacy eMule Secure User Identification (SUI) — RSA-384, RSASSA-PKCS1v15-SHA1.
//!
//! Confirmed against eMule `ClientCredits.cpp` / aMule `ClientCreditsList.cpp`:
//! the signature scheme is `CryptoPP::RSASSA_PKCS1v15_SHA` and the signed payload
//! is `challenger_pubkey_der || challenge_u32_le [|| ip_u32_le || kind_u8]`.
//!
//! Compatibility only — never reuse this for modern EMule-Rust cryptography.

use rsa::pkcs1v15::Pkcs1v15Sign;
use rsa::traits::PublicKeyParts;
use rsa::{RsaPrivateKey, RsaPublicKey};

/// eMule `RSAKEYSIZE` (opcodes.h): 384 bits.
pub const EMULE_RSA_BITS: usize = 384;

#[derive(Debug, thiserror::Error)]
pub enum LegacySuiError {
    #[error("rsa operation failed: {0}")]
    Rsa(#[from] rsa::Error),
    #[error("signature failure: {0}")]
    Signature(#[from] rsa::signature::Error),
    #[error("missing V2 ip field")]
    MissingV2Ip,
    #[error("missing V2 ip kind field")]
    MissingV2IpKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecureIdentVersion {
    V1,
    V2,
}

/// Builds the exact byte sequence eMule signs:
/// `challenger_pubkey || challenge_u32_le [|| ip_u32_le || kind_u8]`.
pub fn build_sui_signing_payload(
    version: SecureIdentVersion,
    challenger_public_key_der: &[u8],
    challenge: u32,
    ip: Option<u32>,
    ip_kind: Option<u8>,
) -> Result<Vec<u8>, LegacySuiError> {
    let extra = match version {
        SecureIdentVersion::V1 => 4,
        SecureIdentVersion::V2 => 9,
    };
    let mut payload = Vec::with_capacity(challenger_public_key_der.len() + extra);
    payload.extend_from_slice(challenger_public_key_der);
    payload.extend_from_slice(&challenge.to_le_bytes());
    if matches!(version, SecureIdentVersion::V2) {
        payload.extend_from_slice(&ip.ok_or(LegacySuiError::MissingV2Ip)?.to_le_bytes());
        payload.push(ip_kind.ok_or(LegacySuiError::MissingV2IpKind)?);
    }
    Ok(payload)
}

/// RSASSA-PKCS1v15-SHA1 signature, matching `RSASSA_PKCS1v15_SHA_Signer::SignMessage`.
/// The `rsa` crate expects a pre-hashed digest; we hash the payload with SHA-1 here.
pub fn sign_sui_payload(key: &RsaPrivateKey, payload: &[u8]) -> Result<Vec<u8>, LegacySuiError> {
    use sha1::Digest;
    let digest = sha1::Sha1::digest(payload);
    let scheme = Pkcs1v15Sign::new::<sha1::Sha1>();
    Ok(key.sign_with_rng(&mut rsa::rand_core::OsRng, scheme, &digest)?)
}

/// eMule `MAXPUBKEYSIZE`: maximum accepted public key wire length.
pub const MAX_PUBKEY_WIRE: usize = 80;

/// Parses a peer's wire public key. Accepts PKCS#1 `RSAPublicKey` DER
/// (`SEQUENCE { n INTEGER, e INTEGER }`, the same bytes Crypto++
/// `GetMaterial().Save()` produces) and, as fallback, X.509
/// `SubjectPublicKeyInfo` (`SEQUENCE { AlgorithmIdentifier, BIT STRING }`)
/// which some clients send instead.
pub fn parse_peer_public_key(der: &[u8]) -> Result<RsaPublicKey, LegacySuiError> {
    use rsa::pkcs1::DecodeRsaPublicKey;
    if der.len() > MAX_PUBKEY_WIRE {
        return Err(LegacySuiError::Signature(rsa::signature::Error::new()));
    }
    let key = RsaPublicKey::from_pkcs1_der(der)
        .or_else(|_| {
            use rsa::pkcs8::DecodePublicKey;
            RsaPublicKey::from_public_key_der(der)
        })
        .map_err(|e| LegacySuiError::Signature(rsa::signature::Error::from_source(e)))?;
    // from_pkcs1_der already performs basic sanity checks; reject degenerate
    // moduli that would make signature verification meaningless.
    use rsa::traits::PublicKeyParts;
    if key.size() == 0 || key.size() > 1024 {
        return Err(LegacySuiError::Signature(rsa::signature::Error::new()));
    }
    Ok(key)
}

/// Verification, matching `RSASSA_PKCS1v15_SHA_Verifier::VerifyMessage`.
/// `signature.len()` must equal the modulus length (48 bytes for RSA-384).
pub fn verify_sui_signature(
    public_key: &RsaPublicKey,
    payload: &[u8],
    signature: &[u8],
) -> Result<(), LegacySuiError> {
    let expected = public_key.size();
    if signature.len() != expected {
        return Err(LegacySuiError::Signature(rsa::signature::Error::new()));
    }
    use sha1::Digest;
    let digest = sha1::Sha1::digest(payload);
    let scheme = Pkcs1v15Sign::new::<sha1::Sha1>();
    public_key.verify(scheme, &digest, signature)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsa::traits::{PrivateKeyParts, PublicKeyParts};

    /// Spike: does the `rsa` crate accept 384-bit key generation?
    #[test]
    fn rsa_384_generation_is_supported() {
        let mut rng = rsa::rand_core::OsRng;
        let key = RsaPrivateKey::new(&mut rng, EMULE_RSA_BITS)
            .expect("RSA-384 generation must be supported for legacy SUI");
        assert_eq!(key.n().bits(), EMULE_RSA_BITS);
    }

    /// Spike: sign + verify roundtrip at 384 bits with the eMule scheme.
    #[test]
    fn rsa_384_pkcs1v15_sha1_roundtrip() {
        let mut rng = rsa::rand_core::OsRng;
        let key = RsaPrivateKey::new(&mut rng, EMULE_RSA_BITS).unwrap();
        let public = RsaPublicKey::from(&key);

        let payload = build_sui_signing_payload(
            SecureIdentVersion::V1,
            &[0x30; 76], // placeholder DER-shaped pubkey bytes
            0xDEADBEEF,
            None,
            None,
        )
        .unwrap();

        let sig = sign_sui_payload(&key, &payload).unwrap();
        assert_eq!(sig.len(), EMULE_RSA_BITS / 8); // 48 bytes
        verify_sui_signature(&public, &payload, &sig).unwrap();

        // Wrong signature / wrong payload must fail
        assert!(verify_sui_signature(&public, &payload, &[0u8; 48]).is_err());
        let mut bad = payload.clone();
        bad[0] ^= 1;
        assert!(verify_sui_signature(&public, &bad, &sig).is_err());
    }

    /// Spike: reconstruct a key from components (cryptkey.dat import path).
    #[test]
    fn rsa_384_from_components_roundtrip() {
        let mut rng = rsa::rand_core::OsRng;
        let key = RsaPrivateKey::new(&mut rng, EMULE_RSA_BITS).unwrap();
        let rebuilt = RsaPrivateKey::from_components(
            key.n().clone(),
            key.e().clone(),
            key.d().clone(),
            key.primes().to_vec(),
        )
        .unwrap();
        let payload = build_sui_signing_payload(
            SecureIdentVersion::V2,
            &[0x30; 76],
            7,
            Some(0x0100007F),
            Some(20), // CRYPT_CIP_LOCALCLIENT
        )
        .unwrap();
        assert_eq!(payload.len(), 76 + 4 + 4 + 1);
        let sig = sign_sui_payload(&rebuilt, &payload).unwrap();
        verify_sui_signature(&RsaPublicKey::from(&rebuilt), &payload, &sig).unwrap();
    }

    /// Public key wire size check: eMule caps OP_PUBLICKEY at 80 bytes.
    #[test]
    fn rsa_384_pubkey_fits_80_byte_wire_limit() {
        let mut rng = rsa::rand_core::OsRng;
        let key = RsaPrivateKey::new(&mut rng, EMULE_RSA_BITS).unwrap();
        let public = RsaPublicKey::from(&key);
        // DER SEQUENCE { n INTEGER, e INTEGER } = PKCS#1 RSAPublicKey,
        // matching CryptoPP `GetMaterial().Save()` / `DEREncode()` output.
        use rsa::pkcs1::EncodeRsaPublicKey;
        let der = public.to_pkcs1_der().unwrap();
        assert!(
            der.as_bytes().len() <= 80,
            "der len = {}",
            der.as_bytes().len()
        );
    }

    /// Peer keys arriving as X.509 `SubjectPublicKeyInfo` (observed on the
    /// live network: `SEQUENCE { AlgorithmIdentifier, BIT STRING }`) are
    /// accepted via the SPKI fallback, not only bare PKCS#1.
    #[test]
    fn spki_wrapped_pubkey_is_accepted() {
        use rsa::pkcs1::EncodeRsaPublicKey;
        use rsa::pkcs8::EncodePublicKey;
        let mut rng = rsa::rand_core::OsRng;
        let key = RsaPrivateKey::new(&mut rng, EMULE_RSA_BITS).unwrap();
        let public = RsaPublicKey::from(&key);

        let pkcs1 = public.to_pkcs1_der().unwrap();
        let spki = public.to_public_key_der().unwrap();
        assert!(
            spki.as_bytes().len() <= MAX_PUBKEY_WIRE,
            "spki len = {}",
            spki.as_bytes().len()
        );

        let parsed_pkcs1 = parse_peer_public_key(pkcs1.as_bytes()).unwrap();
        let parsed_spki = parse_peer_public_key(spki.as_bytes()).unwrap();
        assert_eq!(parsed_pkcs1, parsed_spki);

        // A signature verifies against the SPKI-parsed key too.
        let payload = build_sui_signing_payload(
            SecureIdentVersion::V1,
            pkcs1.as_bytes(),
            0x12345678,
            None,
            None,
        )
        .unwrap();
        let sig = sign_sui_payload(&key, &payload).unwrap();
        verify_sui_signature(&parsed_spki, &payload, &sig).unwrap();
    }

    /// V2 payload ordering: pubkey || challenge || ip || kind.
    #[test]
    fn v2_payload_byte_order() {
        let payload = build_sui_signing_payload(
            SecureIdentVersion::V2,
            &[0xAA, 0xBB],
            0x01020304,
            Some(0x7F000001),
            Some(10),
        )
        .unwrap();
        assert_eq!(
            payload,
            vec![0xAA, 0xBB, 0x04, 0x03, 0x02, 0x01, 0x01, 0x00, 0x00, 0x7F, 10]
        );
    }
}
