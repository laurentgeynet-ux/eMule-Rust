//! Diffie-Hellman 768-bit MODP key agreement for eMule server obfuscation.

use num_bigint::BigUint;
use rand::RngCore;

/// Fixed 768-bit (96-byte) prime modulus used by eMule (`dh768_p`).
pub const DH768_P: [u8; 96] = [
    0xF2, 0xBF, 0x52, 0xC5, 0x5F, 0x58, 0x7A, 0xDD, 0x53, 0x71, 0xA9, 0x36, 0xE8, 0x86, 0xEB, 0x3C,
    0x62, 0x17, 0xA3, 0x3E, 0xC3, 0x4C, 0xB4, 0x0D, 0xC7, 0x3A, 0x41, 0xA6, 0x43, 0xAF, 0xFC, 0xE7,
    0x21, 0xFC, 0x28, 0x63, 0x66, 0x53, 0x5B, 0xDB, 0xCE, 0x25, 0x9F, 0x22, 0x86, 0xDA, 0x4A, 0x91,
    0xB2, 0x07, 0xCB, 0xAA, 0x52, 0x55, 0xD4, 0xF6, 0x1C, 0xCE, 0xAE, 0xD4, 0x5A, 0xD5, 0xE0, 0x74,
    0x7D, 0xF7, 0x78, 0x18, 0x28, 0x10, 0x5F, 0x34, 0x0F, 0x76, 0x23, 0x87, 0xF8, 0x8B, 0x28, 0x91,
    0x42, 0xFB, 0x42, 0x68, 0x8F, 0x05, 0x15, 0x0F, 0x54, 0x8B, 0x5F, 0x43, 0x6A, 0xF7, 0x0D, 0xF3,
];

/// Generator $g = 2$.
pub const DH768_G: u32 = 2;

/// A Diffie-Hellman 768-bit key pair.
#[derive(Clone, Debug)]
pub struct Dh768KeyPair {
    /// 128-bit random private exponent $a$.
    pub private_key: [u8; 16],
    /// 768-bit (96-byte) public key $A = g^a \pmod p$.
    pub public_key: [u8; 96],
}

impl Dh768KeyPair {
    /// Generates a new DH-768 key pair using the provided random generator.
    pub fn generate<R: RngCore>(rng: &mut R) -> Self {
        let mut priv_bytes = [0u8; 16];
        rng.fill_bytes(&mut priv_bytes);
        // Ensure private key is non-zero
        priv_bytes[0] |= 1;

        let p = BigUint::from_bytes_be(&DH768_P);
        let g = BigUint::from(DH768_G);
        let a = BigUint::from_bytes_be(&priv_bytes);

        let public_num = g.modpow(&a, &p);
        let public_key = encode_biguint_96(&public_num);

        Self {
            private_key: priv_bytes,
            public_key,
        }
    }

    /// Computes the shared secret $S = B^a \pmod p$ from the remote 96-byte public key $B$.
    pub fn compute_shared_secret(&self, remote_public_bytes: &[u8; 96]) -> [u8; 96] {
        let p = BigUint::from_bytes_be(&DH768_P);
        let a = BigUint::from_bytes_be(&self.private_key);
        let b = BigUint::from_bytes_be(remote_public_bytes);

        let s = b.modpow(&a, &p);
        encode_biguint_96(&s)
    }
}

/// Helper to format a `BigUint` as a fixed 96-byte big-endian array.
fn encode_biguint_96(num: &BigUint) -> [u8; 96] {
    let mut out = [0u8; 96];
    let bytes = num.to_bytes_be();
    if bytes.len() <= 96 {
        out[96 - bytes.len()..].copy_from_slice(&bytes);
    } else {
        // Truncate from the end if somehow larger
        out.copy_from_slice(&bytes[bytes.len() - 96..]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dh768_key_exchange_roundtrip() {
        let mut rng = rand::rng();
        let alice = Dh768KeyPair::generate(&mut rng);
        let bob = Dh768KeyPair::generate(&mut rng);

        let secret_alice = alice.compute_shared_secret(&bob.public_key);
        let secret_bob = bob.compute_shared_secret(&alice.public_key);

        assert_eq!(
            secret_alice, secret_bob,
            "Both parties must agree on the same shared secret S"
        );
        assert_ne!(secret_alice, [0u8; 96], "Shared secret must be non-zero");
    }
}
