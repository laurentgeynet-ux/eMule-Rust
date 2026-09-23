// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Key derivation for eMule protocol obfuscation (MD5 over UserHash and DH secrets).

use md5::{Digest, Md5};

use super::rc4::Rc4Cipher;

/// Modification of the requester-send and server-receive key (`34` = `0x22`).
pub const MAGICVALUE_REQUESTER: u8 = 34;

/// Modification of the server-send and requester-receive key (`203` = `0xCB`).
pub const MAGICVALUE_SERVER: u8 = 203;

/// Derives RC4-drop1024 ciphers for Client A (outgoing peer connection initiator).
///
/// Returns `(send_cipher, recv_cipher)`.
pub fn derive_initiator_peer_ciphers(
    target_user_hash: &[u8; 16],
    random_key_part: u32,
) -> (Rc4Cipher, Rc4Cipher) {
    let (send_key, recv_key) = derive_initiator_peer_raw_keys(target_user_hash, random_key_part);
    (
        Rc4Cipher::new_drop1024(&send_key),
        Rc4Cipher::new_drop1024(&recv_key),
    )
}

/// Derives raw 16-byte keys `(send_key, recv_key)` for Client A (initiator).
pub fn derive_initiator_peer_raw_keys(
    target_user_hash: &[u8; 16],
    random_key_part: u32,
) -> ([u8; 16], [u8; 16]) {
    // SendKey = MD5(target_user_hash[16] || 34 || random_key_part[4])
    let mut mat_send = [0u8; 21];
    mat_send[..16].copy_from_slice(target_user_hash);
    mat_send[16] = MAGICVALUE_REQUESTER;
    mat_send[17..21].copy_from_slice(&random_key_part.to_le_bytes());
    let send_key: [u8; 16] = Md5::digest(mat_send).into();

    // RecvKey = MD5(target_user_hash[16] || 203 || random_key_part[4])
    let mut mat_recv = [0u8; 21];
    mat_recv[..16].copy_from_slice(target_user_hash);
    mat_recv[16] = MAGICVALUE_SERVER;
    mat_recv[17..21].copy_from_slice(&random_key_part.to_le_bytes());
    let recv_key: [u8; 16] = Md5::digest(mat_recv).into();

    (send_key, recv_key)
}

/// Derives RC4-drop1024 ciphers for Client B (inbound peer connection receiver).
///
/// Returns `(send_cipher, recv_cipher)`.
pub fn derive_responder_peer_ciphers(
    local_user_hash: &[u8; 16],
    random_key_part: u32,
) -> (Rc4Cipher, Rc4Cipher) {
    let (send_key, recv_key) = derive_responder_peer_raw_keys(local_user_hash, random_key_part);
    (
        Rc4Cipher::new_drop1024(&send_key),
        Rc4Cipher::new_drop1024(&recv_key),
    )
}

/// Derives raw 16-byte keys `(send_key, recv_key)` for Client B (responder).
pub fn derive_responder_peer_raw_keys(
    local_user_hash: &[u8; 16],
    random_key_part: u32,
) -> ([u8; 16], [u8; 16]) {
    // Responder SendKey = MD5(local_user_hash[16] || 203 || random_key_part[4])
    let mut mat_send = [0u8; 21];
    mat_send[..16].copy_from_slice(local_user_hash);
    mat_send[16] = MAGICVALUE_SERVER;
    mat_send[17..21].copy_from_slice(&random_key_part.to_le_bytes());
    let send_key: [u8; 16] = Md5::digest(mat_send).into();

    // Responder RecvKey = MD5(local_user_hash[16] || 34 || random_key_part[4])
    let mut mat_recv = [0u8; 21];
    mat_recv[..16].copy_from_slice(local_user_hash);
    mat_recv[16] = MAGICVALUE_REQUESTER;
    mat_recv[17..21].copy_from_slice(&random_key_part.to_le_bytes());
    let recv_key: [u8; 16] = Md5::digest(mat_recv).into();

    (send_key, recv_key)
}

/// Derives RC4-drop1024 ciphers for a Client <-> Server connection from DH shared secret.
///
/// Returns `(send_cipher, recv_cipher)`.
pub fn derive_server_ciphers(shared_secret: &[u8; 96]) -> (Rc4Cipher, Rc4Cipher) {
    let mut mat_send = [0u8; 97];
    mat_send[..96].copy_from_slice(shared_secret);
    mat_send[96] = MAGICVALUE_REQUESTER;
    let send_key: [u8; 16] = Md5::digest(mat_send).into();

    let mut mat_recv = [0u8; 97];
    mat_recv[..96].copy_from_slice(shared_secret);
    mat_recv[96] = MAGICVALUE_SERVER;
    let recv_key: [u8; 16] = Md5::digest(mat_recv).into();

    (
        Rc4Cipher::new_drop1024(&send_key),
        Rc4Cipher::new_drop1024(&recv_key),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_peer_key_symmetry() {
        let user_hash_b = [0x55u8; 16];
        let random_key_part_a = 0x12345678u32;

        let (a_send_key, a_recv_key) =
            derive_initiator_peer_raw_keys(&user_hash_b, random_key_part_a);
        let (b_send_key, b_recv_key) =
            derive_responder_peer_raw_keys(&user_hash_b, random_key_part_a);

        // Client A's SendKey must match Client B's RecvKey!
        assert_eq!(
            a_send_key, b_recv_key,
            "Client A SendKey must equal Client B RecvKey"
        );
        // Client B's SendKey must match Client A's RecvKey!
        assert_eq!(
            b_send_key, a_recv_key,
            "Client B SendKey must equal Client A RecvKey"
        );
        // Send and Receive keys must differ
        assert_ne!(a_send_key, a_recv_key);
    }
}
