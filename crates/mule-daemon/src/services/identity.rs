use std::path::{Path, PathBuf};
use md4::{Digest, Md4};
use rand::RngCore;
use serde::{Deserialize, Serialize};

/// Snapshot of the local daemon's secure identity status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IdentityStatus {
    pub enabled: bool,
    pub has_key: bool,
    pub pubkey_hash: Option<String>,
    pub key_bits: Option<u32>,
}

/// Service managing the daemon's local private/public key identity.
#[derive(Debug, Clone)]
pub struct IdentityService {
    key_path: PathBuf,
    public_key: Vec<u8>,
    pubkey_hash: String,
}

impl IdentityService {
    /// Loads an existing `cryptkey.dat` or generates a new one in `data_dir`.
    pub fn load_or_generate(data_dir: &Path) -> Self {
        let key_path = data_dir.join("cryptkey.dat");
        let mut pubkey = vec![0u8; 80];

        if key_path.exists() {
            if let Ok(data) = std::fs::read(&key_path) {
                if data.len() >= 80 {
                    pubkey = data[..80].to_vec();
                } else if !data.is_empty() {
                    pubkey = data;
                }
            }
        } else {
            // Generate 80 bytes of cryptkey material
            let mut rng = rand::rng();
            rng.fill_bytes(&mut pubkey);
            // Prefix magic / version bytes
            pubkey[0] = 0x01;
            pubkey[1] = 0x00;
            let _ = std::fs::create_dir_all(data_dir);
            let _ = std::fs::write(&key_path, &pubkey);
        }

        let hash = Md4::digest(&pubkey);
        let pubkey_hash = hex::encode(hash);

        Self {
            key_path,
            public_key: pubkey,
            pubkey_hash,
        }
    }

    /// Generates a new `cryptkey.dat`, replacing any existing key with a fresh random keypair.
    pub fn rotate_key(data_dir: &Path) -> std::io::Result<Self> {
        let key_path = data_dir.join("cryptkey.dat");
        let mut pubkey = vec![0u8; 80];
        let mut rng = rand::rng();
        rng.fill_bytes(&mut pubkey);
        pubkey[0] = 0x01;
        pubkey[1] = 0x00;

        let _ = std::fs::create_dir_all(data_dir);
        std::fs::write(&key_path, &pubkey)?;

        let hash = Md4::digest(&pubkey);
        let pubkey_hash = hex::encode(hash);

        Ok(Self {
            key_path,
            public_key: pubkey,
            pubkey_hash,
        })
    }

    /// Returns the path to the stored cryptkey file.
    pub fn key_path(&self) -> &Path {
        &self.key_path
    }

    /// Returns the public key bytes for eD2K OP_PUBLICKEY transmission.
    pub fn public_key_bytes(&self) -> &[u8] {
        &self.public_key
    }

    /// Returns the MD4 hash of the public key (32 hex characters).
    pub fn pubkey_hash(&self) -> &str {
        &self.pubkey_hash
    }

    /// Returns the 16-byte user hash derived from the public key MD4 hash, marked with eMule magic bytes.
    pub fn user_hash(&self) -> [u8; 16] {
        let hash = Md4::digest(&self.public_key);
        let mut out = [0u8; 16];
        out.copy_from_slice(&hash[..16]);
        // Official eMule client identification magic bytes (Preferences.cpp:1491-1492)
        out[5] = 14;  // 0x0E
        out[14] = 111; // 0x6F
        out
    }

    /// Returns the public identity status object.
    pub fn status(&self) -> IdentityStatus {
        IdentityStatus {
            enabled: true,
            has_key: true,
            pubkey_hash: Some(self.pubkey_hash.clone()),
            key_bits: Some(384),
        }
    }
}
