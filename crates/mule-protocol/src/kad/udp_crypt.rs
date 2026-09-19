//! Decryption of obfuscated eD2K/Kad UDP datagrams (eMule
//! `CEncryptedDatagramSocket::DecryptReceivedClient`, EncryptedDatagramSocket.cpp).
//!
//! eMule encrypts certain Kad UDP replies — notably `KADEMLIA2_FIREWALLUDP`
//! answers — with an RC4 stream keyed by
//! `MD5(receiver_verify_key:u32 LE || random_key_part:u16 LE)`.
//! Wire layout (all bytes from offset 3 are RC4-encrypted):
//!
//! ```text
//! [0]     semi-random protocol byte (not a valid Kad protocol)
//! [1..2]  random key part (u16 LE, feeds the key)
//! [3..6]  magic u32 = MAGICVALUE_UDP_SYNC_CLIENT (0x395F2EC1)
//! [7]     padding length byte (low 4 bits; currently 0)
//! [8+p..] dwReceiverKey:u32 + dwSenderKey:u32 + payload (plaintext Kad packet)
//! ```

use crate::ed2k::obfuscation::Rc4Cipher;
use md5::{Digest, Md5};

/// eMule `MAGICVALUE_UDP_SYNC_CLIENT` — sanity magic inside encrypted
/// client-to-client UDP datagrams.
pub const MAGICVALUE_UDP_SYNC_CLIENT: u32 = 0x395F_2EC1;

/// Result of a successful Kad UDP datagram decryption.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecryptedKadUdp {
    /// Decrypted wire packet (proto + opcode + payload), ready for
    /// `KadCodec::decode`.
    pub payload: Vec<u8>,
    /// `dwReceiverKey` carried in the datagram (should equal the verify key
    /// used for decryption).
    pub receiver_key: u32,
    /// `dwSenderKey` carried in the datagram.
    pub sender_key: u32,
}

/// Attempts to decrypt `buf` as an encrypted Kad UDP datagram using
/// `receiver_verify_key` (the key the remote peer was told to use — for the
/// UDP firewall check, the `sender_key` we sent in `OP_FWCHECKUDPREQ`).
///
/// Returns `None` when the buffer is not an encrypted packet or the magic
/// does not check out (wrong key / corrupt).
pub fn decrypt_kad_udp_packet(buf: &[u8], receiver_verify_key: u32) -> Option<DecryptedKadUdp> {
    // Minimum: proto(1) + randkey(2) + magic(4) + padbyte(1) + keys(8) + >=1 payload
    if buf.len() <= 16 || receiver_verify_key == 0 {
        return None;
    }
    let random_key_part = u16::from_le_bytes([buf[1], buf[2]]);

    let mut key_data = [0u8; 6];
    key_data[..4].copy_from_slice(&receiver_verify_key.to_le_bytes());
    key_data[4..6].copy_from_slice(&random_key_part.to_le_bytes());
    let md5_key: [u8; 16] = Md5::digest(&key_data).into();
    // eMule RC4CreateKey(..., bSkipDiscard=true) → no 1024-byte drop.
    let mut cipher = Rc4Cipher::new(&md5_key, true);

    let mut dec = buf[3..].to_vec();
    cipher.process(&mut dec);

    let magic = u32::from_le_bytes(dec[..4].try_into().ok()?);
    if magic != MAGICVALUE_UDP_SYNC_CLIENT {
        return None;
    }
    let pad_len = (dec[4] & 0x0F) as usize;
    let keys_off = 5 + pad_len;
    if dec.len() < keys_off + 8 {
        return None;
    }
    let receiver_key = u32::from_le_bytes(dec[keys_off..keys_off + 4].try_into().ok()?);
    let sender_key = u32::from_le_bytes(dec[keys_off + 4..keys_off + 8].try_into().ok()?);
    Some(DecryptedKadUdp {
        payload: dec[keys_off + 8..].to_vec(),
        receiver_key,
        sender_key,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reproduces `CEncryptedDatagramSocket::EncryptSendClient` (Kad, reckey
    /// path, zero padding) so the decrypt path can be roundtrip-tested.
    fn encrypt_for_test(plain: &[u8], receiver_key: u32, sender_key: u32) -> Vec<u8> {
        let random_key_part: u16 = 0xBEEF;
        let mut key_data = [0u8; 6];
        key_data[..4].copy_from_slice(&receiver_key.to_le_bytes());
        key_data[4..6].copy_from_slice(&random_key_part.to_le_bytes());
        let md5_key: [u8; 16] = Md5::digest(&key_data).into();
        let mut cipher = Rc4Cipher::new(&md5_key, true);

        let mut out = Vec::with_capacity(3 + 5 + 8 + plain.len());
        out.push(0x46u8); // semi-random proto byte (reckey marker bits)
        out.extend_from_slice(&random_key_part.to_le_bytes());
        let mut tail = Vec::new();
        tail.extend_from_slice(&MAGICVALUE_UDP_SYNC_CLIENT.to_le_bytes());
        tail.push(0); // padding length = 0
        tail.extend_from_slice(&receiver_key.to_le_bytes());
        tail.extend_from_slice(&sender_key.to_le_bytes());
        tail.extend_from_slice(plain);
        cipher.process(&mut tail);
        out.extend_from_slice(&tail);
        out
    }

    #[test]
    fn decrypt_roundtrip() {
        let plain = [0xE4u8, 0x62, 0x00, 0x42, 0x12]; // FIREWALLUDP err=0 port=0x1242
        let enc = encrypt_for_test(&plain, 0xDEAD_BEEF, 0x1234_5678);
        assert_ne!(enc[0], 0xE4);
        let d = decrypt_kad_udp_packet(&enc, 0xDEAD_BEEF).expect("should decrypt");
        assert_eq!(d.payload, plain);
        assert_eq!(d.receiver_key, 0xDEAD_BEEF);
        assert_eq!(d.sender_key, 0x1234_5678);
    }

    #[test]
    fn decrypt_wrong_key_fails() {
        let enc = encrypt_for_test(&[0xE4u8, 0x62, 0, 1, 2], 0xDEAD_BEEF, 1);
        assert!(decrypt_kad_udp_packet(&enc, 0x1111_2222).is_none());
    }

    #[test]
    fn decrypt_short_or_zero_key_fails() {
        assert!(decrypt_kad_udp_packet(&[0xAA; 8], 1).is_none());
        let enc = encrypt_for_test(&[0xE4u8, 0x62, 0, 1, 2], 1, 1);
        assert!(decrypt_kad_udp_packet(&enc, 0).is_none());
    }
}
