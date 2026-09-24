// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! AICH keyword-storage wire tests — `TAG_KADAICHHASHPUB` (0x36),
//! `TAG_KADAICHHASHRESULT` (0x37), `TAG_PUBLISHINFO` (0x33).
//!
//! Reference: eMule 0.72a `Entry.cpp::WriteTagListWithPublishInfo`,
//! `KademliaUDPListener.cpp::Process_KADEMLIA2_PUBLISH_KEY_REQ`,
//! `Search.cpp::ProcessResultKeyword`.

use mule_protocol::kad::*;
use mule_protocol::KadCodec;

fn kw_entry() -> KadKeywordEntryWire {
    KadKeywordEntryWire {
        file_hash: [0x11u8; 16],
        file_name: "test-file.bin".to_string(),
        file_size: 1_234_567,
        source_count: 3,
        tags: Vec::new(),
        aich_pub: None,
        aich_votes: Vec::new(),
        publish_info: None,
    }
}

/// Finds the raw payload of a tag by 1-byte id inside a serialized
/// keyword entry — walks the entry tag list (`[type][u16 nlen][name][val]`).
fn find_tag_bytes(entry_bytes: &[u8], want_name: u8) -> Option<Vec<u8>> {
    if entry_bytes.is_empty() {
        return None;
    }
    let tag_count = entry_bytes[0] as usize;
    let mut pos = 1usize;
    for _ in 0..tag_count {
        if pos + 3 > entry_bytes.len() {
            return None;
        }
        let ty = entry_bytes[pos];
        let nlen = u16::from_le_bytes([entry_bytes[pos + 1], entry_bytes[pos + 2]]) as usize;
        pos += 3;
        if pos + nlen > entry_bytes.len() {
            return None;
        }
        let name = entry_bytes[pos];
        pos += nlen;
        let val_len = match ty {
            0x01 => 16,          // HASH
            0x02 => {
                let l = u16::from_le_bytes([entry_bytes[pos], entry_bytes[pos + 1]]) as usize;
                pos += 2;
                l
            }
            0x03 => 4,           // UINT32
            0x04 => 4,           // FLOAT32
            0x05 => 1,           // BOOL
            0x07 => {
                let l = u32::from_le_bytes([
                    entry_bytes[pos],
                    entry_bytes[pos + 1],
                    entry_bytes[pos + 2],
                    entry_bytes[pos + 3],
                ]) as usize;
                pos += 4;
                l
            }
            0x08 => 2,           // UINT16
            0x09 => 1,           // UINT8
            0x0A => {
                // BSOB: u8 len + data
                let l = entry_bytes[pos] as usize;
                if name == want_name {
                    return Some(entry_bytes[pos..pos + 1 + l].to_vec());
                }
                pos += 1;
                l
            }
            0x0B => 8,           // UINT64
            _ => return None,
        };
        if name == want_name {
            return Some(entry_bytes[pos..pos + val_len].to_vec());
        }
        pos += val_len;
    }
    None
}

/// Extracts the first keyword entry's serialized tag list from an encoded
/// `[PR][opcode][kh:16][count:u16]` publish request.
fn first_publish_entry_tags(pkt: &[u8]) -> &[u8] {
    // header 2 + keyword_hash 16 + count u16 + file_hash 16 = tag list at 36
    &pkt[2 + 16 + 2 + 16..]
}

/// Extracts the first keyword entry's serialized tag list from an encoded
/// `KADEMLIA2_SEARCH_RES` (`[sender 16][target 16][count u16][hash 16]`).
fn first_res_entry_tags(pkt: &[u8]) -> &[u8] {
    &pkt[2 + 16 + 16 + 2 + 16..]
}

#[test]
fn publish_req_encodes_aich_pub_bsob() {
    let kw = [0xAAu8; 16];
    let root = [0x42u8; 20];
    let mut e = kw_entry();
    e.aich_pub = Some(root);
    let pkt = KadCodec::encode_publish_keyword_req(&kw, &[e]);

    // The wire must carry a TAGTYPE_BSOB (0x0A) tag named 0x36 holding
    // exactly the 20-byte root (eMule `CKadTagBsob(TAG_KADAICHHASHPUB)`).
    let tags = first_publish_entry_tags(&pkt);
    let bsob = find_tag_bytes(tags, TAG_KADAICHHASH_PUB).expect("0x36 tag present");
    assert_eq!(bsob[0], 20, "BSOB len byte = AICH root size");
    assert_eq!(&bsob[1..], &root);

    // Decode extracts aich_pub and strips it from the stored tag list.
    match KadCodec::decode(&pkt).expect("decode publish req") {
        KadMessage::PublishKeywordReq(r) => {
            let e = &r.entries[0];
            assert_eq!(e.aich_pub, Some(root));
            assert!(!e.tags.iter().any(|t| t.name
                == mule_protocol::ed2k::tags::TagName::Id(TAG_KADAICHHASH_PUB)));
        }
        _ => panic!("expected PublishKeywordReq"),
    }
}

#[test]
fn publish_req_without_aich_has_no_pub_tag() {
    let kw = [0xAAu8; 16];
    let pkt = KadCodec::encode_publish_keyword_req(&kw, &[kw_entry()]);
    assert!(find_tag_bytes(first_publish_entry_tags(&pkt), TAG_KADAICHHASH_PUB).is_none());
}

#[test]
fn publish_req_multiple_aich_pub_first_wins() {
    // eMule `KademliaUDPListener`: extra TAG_KADAICHHASHPUB tags are
    // ignored — only the first valid BSOB-20 is applied.
    let kw = [0xAAu8; 16];
    let root1 = [0x11u8; 20];
    let root2 = [0x22u8; 20];
    // Hand-build a publish req whose entry carries two 0x36 tags.
    let mut pkt = Vec::new();
    pkt.extend_from_slice(&[PR_KADEMLIA, 0x43]);
    pkt.extend_from_slice(&kw);
    pkt.extend_from_slice(&1u16.to_le_bytes()); // 1 entry
    pkt.extend_from_slice(&[0x11u8; 16]); // file hash
    pkt.push(2u8); // tag count = 2
    for root in [&root1, &root2] {
        pkt.push(0x0A); // TAGTYPE_BSOB
        pkt.extend_from_slice(&1u16.to_le_bytes());
        pkt.push(TAG_KADAICHHASH_PUB);
        pkt.push(20u8);
        pkt.extend_from_slice(root);
    }
    match KadCodec::decode(&pkt).expect("decode") {
        KadMessage::PublishKeywordReq(r) => {
            assert_eq!(r.entries[0].aich_pub, Some(root1));
        }
        _ => panic!("expected PublishKeywordReq"),
    }
}

#[test]
fn publish_req_malformed_aich_pub_ignored() {
    let kw = [0xAAu8; 16];
    let mut pkt = Vec::new();
    pkt.extend_from_slice(&[PR_KADEMLIA, 0x43]);
    pkt.extend_from_slice(&kw);
    pkt.extend_from_slice(&1u16.to_le_bytes());
    pkt.extend_from_slice(&[0x11u8; 16]);
    pkt.push(1u8);
    pkt.push(0x0A);
    pkt.extend_from_slice(&1u16.to_le_bytes());
    pkt.push(TAG_KADAICHHASH_PUB);
    pkt.push(19u8); // wrong size — eMule only accepts exactly 20
    pkt.extend_from_slice(&[0x55u8; 19]);
    match KadCodec::decode(&pkt).expect("decode") {
        KadMessage::PublishKeywordReq(r) => assert_eq!(r.entries[0].aich_pub, None),
        _ => panic!("expected PublishKeywordReq"),
    }
}

#[test]
fn search_res_encodes_publishinfo_and_aich_result() {
    let kw = [0xAAu8; 16];
    let root = [0x77u8; 20];
    let mut e = kw_entry();
    e.publish_info = Some((2u32 << 24) | (5u32 << 16) | 150); // 2 names, 5 pubs, trust 1.50
    e.aich_votes = vec![AichHashVote {
        popularity: 5,
        root,
    }];
    let pkt = KadCodec::encode_search_keyword_res(&kw, &[e]);

    let tags = first_res_entry_tags(&pkt);
    // TAG_PUBLISHINFO: UINT32 value verbatim.
    let info = find_tag_bytes(tags, TAG_PUBLISHINFO).expect("0x33 tag present");
    assert_eq!(info.len(), 4);
    assert_eq!(u32::from_le_bytes(info.try_into().unwrap()), (2 << 24) | (5 << 16) | 150);
    // TAG_KADAICHHASHRESULT: <u8 count><u8 pop><20B root>.
    // BSOB payload as returned by find_tag_bytes: [u8 bsob_len][u8 count]<u8 pop><20B>.
    let bsob = find_tag_bytes(tags, TAG_KADAICHHASH_RESULT).expect("0x37 tag present");
    assert_eq!(bsob, [vec![22u8, 1u8, 5u8], root.to_vec()].concat());

    match KadCodec::decode(&pkt).expect("decode search res") {
        KadMessage::SearchKeywordRes(r) => {
            let e = &r.entries[0];
            assert_eq!(e.publish_info, Some((2 << 24) | (5 << 16) | 150));
            assert_eq!(e.aich_votes.len(), 1);
            assert_eq!(e.aich_votes[0].popularity, 5);
            assert_eq!(e.aich_votes[0].root, root);
            // Only the publisher TAG_SOURCES stays in the stored list —
            // 0x33/0x37 are node-computed and stripped.
            assert_eq!(e.tags.len(), 1);
            assert_eq!(
                e.tags[0].name,
                mule_protocol::ed2k::tags::TagName::Id(0x15),
                "result tags stripped from tag list"
            );
        }
        _ => panic!("expected SearchKeywordRes"),
    }
}

#[test]
fn aich_result_capped_at_eleven_votes() {
    // eMule `WriteTagListWithPublishInfo`: the BSOB must stay under 250
    // bytes — at most 11 `<u8 pop><20B>` pairs fit.
    let kw = [0xAAu8; 16];
    let mut e = kw_entry();
    e.aich_votes = (0..15)
        .map(|i| AichHashVote {
            popularity: i as u8 + 1,
            root: [i as u8; 20],
        })
        .collect();
    let pkt = KadCodec::encode_search_keyword_res(&kw, &[e]);
    let bsob =
        find_tag_bytes(first_res_entry_tags(&pkt), TAG_KADAICHHASH_RESULT).expect("0x37 tag");
    assert_eq!(bsob[1], MAX_AICH_RESULT_VOTES as u8);
    assert_eq!(bsob.len(), 1 + 1 + 21 * MAX_AICH_RESULT_VOTES);
    assert!(bsob.len() <= MAX_AICH_RESULT_BSOB);
}

#[test]
fn aich_result_zero_popularity_filtered() {
    // eMule writes only hashes with popularity > 0 into the result BSOB.
    let kw = [0xAAu8; 16];
    let mut e = kw_entry();
    e.aich_votes = vec![
        AichHashVote {
            popularity: 0,
            root: [0x11u8; 20],
        },
        AichHashVote {
            popularity: 2,
            root: [0x22u8; 20],
        },
    ];
    let pkt = KadCodec::encode_search_keyword_res(&kw, &[e]);
    let bsob =
        find_tag_bytes(first_res_entry_tags(&pkt), TAG_KADAICHHASH_RESULT).expect("0x37 tag");
    assert_eq!(bsob[1], 1);
    assert_eq!(&bsob[3..], &[0x22u8; 20]);
}

#[test]
fn aich_result_malformed_bsob_decodes_empty() {
    // eMule `ProcessResultKeyword`: a malformed AICH BSOB clears the
    // whole vote array.
    let kw = [0xAAu8; 16];
    let mut pkt = Vec::new();
    pkt.extend_from_slice(&[PR_KADEMLIA, 0x3B]);
    pkt.extend_from_slice(&[0u8; 16]); // sender id
    pkt.extend_from_slice(&kw); // target = keyword hash
    pkt.extend_from_slice(&1u16.to_le_bytes());
    pkt.extend_from_slice(&[0x11u8; 16]); // answer file hash
    pkt.push(1u8);
    pkt.push(0x0A);
    pkt.extend_from_slice(&1u16.to_le_bytes());
    pkt.push(TAG_KADAICHHASH_RESULT);
    pkt.push(10u8);
    pkt.extend_from_slice(&[0x33u8; 10]); // truncated histogram
    match KadCodec::decode(&pkt).expect("decode") {
        KadMessage::SearchKeywordRes(r) => assert!(r.entries[0].aich_votes.is_empty()),
        _ => panic!("expected SearchKeywordRes"),
    }
}

#[test]
fn publishinfo_pubs_byte_feeds_source_count_fallback() {
    // eMule readers derive the source count from TAG_PUBLISHINFO's
    // publishers byte when no TAG_SOURCES tag is present.
    let kw = [0xAAu8; 16];
    let mut pkt = Vec::new();
    pkt.extend_from_slice(&[PR_KADEMLIA, 0x3B]);
    pkt.extend_from_slice(&[0u8; 16]);
    pkt.extend_from_slice(&kw);
    pkt.extend_from_slice(&1u16.to_le_bytes());
    pkt.extend_from_slice(&[0x11u8; 16]);
    pkt.push(1u8);
    pkt.push(0x03); // TAGTYPE_UINT32
    pkt.extend_from_slice(&1u16.to_le_bytes());
    pkt.push(TAG_PUBLISHINFO);
    pkt.extend_from_slice(&((1u32 << 24) | (9u32 << 16) | 200u32).to_le_bytes());
    match KadCodec::decode(&pkt).expect("decode") {
        KadMessage::SearchKeywordRes(r) => {
            assert_eq!(r.entries[0].source_count, 9);
            assert_eq!(r.entries[0].publish_info, Some((1 << 24) | (9 << 16) | 200));
        }
        _ => panic!("expected SearchKeywordRes"),
    }
}
