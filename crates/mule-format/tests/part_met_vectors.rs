use mule_format::{MetError, PartMetFile, PARTFILE_VERSION};
use std::io::Cursor;

fn build_valid_part_met_bytes() -> Vec<u8> {
    let mut bytes = Vec::new();
    // Header
    bytes.push(PARTFILE_VERSION); // 0xe0 (offset 0)
    bytes.extend_from_slice(&1_700_000_000u32.to_le_bytes()); // mod date (offset 1..5)
    bytes.extend_from_slice(&[0xaa; 16]); // file hash (offset 5..21)

    // Part hashes count: 1
    bytes.extend_from_slice(&1u16.to_le_bytes()); // count (offset 21..23)
    bytes.extend_from_slice(&[0xbb; 16]); // part 0 hash (offset 23..39)

    // Tag count: 2 (offset 39..43)
    bytes.extend_from_slice(&2u32.to_le_bytes());

    // Tag 1: FT_FILENAME (0x01), string type (0x02)
    // Using short name header: raw_type | 0x80 = 0x82, name_id = 0x01
    bytes.push(0x82); // tag type 0x02 + 0x80 flag
    bytes.push(0x01); // FT_FILENAME
    let filename = b"video.mp4";
    bytes.extend_from_slice(&(filename.len() as u16).to_le_bytes());
    bytes.extend_from_slice(filename);

    // Tag 2: FT_FILESIZE (0x02), uint32 type (0x03)
    bytes.push(0x83); // tag type 0x03 + 0x80 flag
    bytes.push(0x02); // FT_FILESIZE
    bytes.extend_from_slice(&10_485_760u32.to_le_bytes()); // 10 MiB

    bytes
}

#[test]
fn test_valid_part_met_parsing() {
    let raw = build_valid_part_met_bytes();
    let mut cursor = Cursor::new(&raw);
    let met = PartMetFile::read_from(&mut cursor).unwrap();

    assert_eq!(met.version, PARTFILE_VERSION);
    assert_eq!(met.file_name.as_deref(), Some("video.mp4"));
    assert_eq!(met.file_size, Some(10_485_760));
    assert_eq!(met.part_hashes.len(), 1);
    assert_eq!(met.part_hashes[0], [0xbb; 16]);
}

#[test]
fn test_unsupported_header_offset() {
    let mut raw = build_valid_part_met_bytes();
    raw[0] = 0x42; // Invalid header version

    let mut cursor = Cursor::new(&raw);
    let err = PartMetFile::read_from(&mut cursor).unwrap_err();

    assert_eq!(
        err,
        MetError::UnsupportedHeader {
            offset: 0,
            value: 0x42
        }
    );
}

#[test]
fn test_unexpected_eof_at_various_offsets() {
    let raw = build_valid_part_met_bytes();

    // 1. Truncate at offset 0 (empty stream)
    let mut cursor = Cursor::new(&[]);
    let err = PartMetFile::read_from(&mut cursor).unwrap_err();
    assert_eq!(
        err,
        MetError::UnexpectedEof {
            offset: 0,
            context: "version header"
        }
    );

    // 2. Truncate before mod date is complete (length 3)
    let mut cursor = Cursor::new(&raw[..3]);
    let err = PartMetFile::read_from(&mut cursor).unwrap_err();
    assert_eq!(
        err,
        MetError::UnexpectedEof {
            offset: 1,
            context: "mod date"
        }
    );

    // 3. Truncate in file hash (length 10)
    let mut cursor = Cursor::new(&raw[..10]);
    let err = PartMetFile::read_from(&mut cursor).unwrap_err();
    assert_eq!(
        err,
        MetError::UnexpectedEof {
            offset: 5,
            context: "file hash"
        }
    );
}

#[test]
fn test_invalid_tag_type_reports_offset() {
    let mut raw = build_valid_part_met_bytes();
    // Offset where tag 1 begins: 43
    // Tag 1 raw type is at offset 43
    raw[43] = 0x80 | 0x7e; // Tag type 0x7e is invalid

    let mut cursor = Cursor::new(&raw);
    let err = PartMetFile::read_from(&mut cursor).unwrap_err();

    assert_eq!(
        err,
        MetError::InvalidTagType {
            offset: 43,
            tag_type: 0x7e
        }
    );
}
