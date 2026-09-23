// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

//! RC4 stream cipher implementation conforming to eMule protocol obfuscation (`RC4-drop1024`).

/// Deterministic 256-byte state RC4 stream cipher.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rc4Cipher {
    state: [u8; 256],
    x: u8,
    y: u8,
}

impl Rc4Cipher {
    /// Creates a new `Rc4Cipher` with standard KSA key setup.
    ///
    /// If `skip_discard` is `false`, discards the first 1024 bytes of keystream (`RC4-drop1024`),
    /// as specified in eMule's protocol obfuscation.
    pub fn new(key: &[u8], skip_discard: bool) -> Self {
        assert!(!key.is_empty(), "RC4 key cannot be empty");
        let mut state = [0u8; 256];
        for (i, item) in state.iter_mut().enumerate() {
            *item = i as u8;
        }
        let mut j: u8 = 0;
        let mut key_idx = 0;
        for i in 0..256 {
            j = j.wrapping_add(state[i]).wrapping_add(key[key_idx]);
            state.swap(i, j as usize);
            key_idx = (key_idx + 1) % key.len();
        }
        let mut cipher = Self { state, x: 0, y: 0 };
        if !skip_discard {
            cipher.drop_bytes(1024);
        }
        cipher
    }

    /// Creates a standard eMule obfuscation cipher with the first 1024 bytes discarded.
    pub fn new_drop1024(key: &[u8]) -> Self {
        Self::new(key, false)
    }

    /// Discards `count` bytes of keystream without modifying any payload.
    pub fn drop_bytes(&mut self, count: usize) {
        for _ in 0..count {
            self.x = self.x.wrapping_add(1);
            self.y = self.y.wrapping_add(self.state[self.x as usize]);
            self.state.swap(self.x as usize, self.y as usize);
        }
    }

    /// Encrypts or decrypts bytes in-place using the PRGA keystream.
    pub fn process(&mut self, data: &mut [u8]) {
        for byte in data.iter_mut() {
            self.x = self.x.wrapping_add(1);
            self.y = self.y.wrapping_add(self.state[self.x as usize]);
            self.state.swap(self.x as usize, self.y as usize);
            let xor_index = self.state[self.x as usize].wrapping_add(self.state[self.y as usize]);
            let k = self.state[xor_index as usize];
            *byte ^= k;
        }
    }

    /// Returns the current internal indices `(x, y)` for debugging/state inspections.
    #[inline]
    pub fn indices(&self) -> (u8, u8) {
        (self.x, self.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rc4_rfc6229_vector() {
        // RFC 6229 test vector: Key 0102030405
        let key = [0x01, 0x02, 0x03, 0x04, 0x05];
        let mut cipher = Rc4Cipher::new(&key, true); // no drop
        let mut plaintext = vec![0u8; 16];
        cipher.process(&mut plaintext);
        // RFC 6229 first 16 bytes: B2 39 63 05 F0 3D C0 27 CC C3 52 4A 0A 11 18 A8
        assert_eq!(
            hex::encode(&plaintext).to_uppercase(),
            "B2396305F03DC027CCC3524A0A1118A8"
        );
    }

    #[test]
    fn test_rc4_drop1024_bidirectional() {
        let key = b"emule_test_key_16b";
        let mut enc = Rc4Cipher::new_drop1024(key);
        let mut dec = Rc4Cipher::new_drop1024(key);

        let original = b"Hello, eMule Obfuscation Protocol World! 0xE3 PR_ED2K";
        let mut buffer = original.to_vec();

        enc.process(&mut buffer);
        assert_ne!(
            &buffer, original,
            "Encrypted data must differ from plaintext"
        );
        assert_ne!(buffer[0], 0xE3, "First byte must not be 0xE3");

        dec.process(&mut buffer);
        assert_eq!(&buffer, original, "Decrypted data must match original");
    }
}
