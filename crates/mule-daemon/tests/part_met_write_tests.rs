//! Tests for `.part.met` companion file emission (eMule SavePartFile parity).

use md4::{Digest, Md4};
use mule_core::AllocationPolicy;
use mule_daemon::services::downloads::DownloadService;
use tempfile::tempdir;

#[test]
fn part_met_is_written_on_add_and_updated_on_progress() {
    let tmp = tempdir().unwrap();
    let mut svc = DownloadService::in_memory(tmp.path().join("temp")).unwrap();

    let content = vec![0x55u8; 1000];
    let file_hash: [u8; 16] = Md4::digest(&content).into();
    let link = format!("ed2k://|file|sample.bin|1000|{}|/", hex::encode(file_hash));
    let snap = svc
        .add_ed2k_link(&link, tmp.path(), AllocationPolicy::None, None)
        .unwrap();

    let (temp_path, _, _, _) = svc.get_download_file_info(&snap.id).unwrap();
    let mut met_name = temp_path.as_os_str().to_os_string();
    met_name.push(".met");
    let met_path = std::path::PathBuf::from(met_name);

    assert!(met_path.exists(), ".part.met must be created on add");

    let met = mule_format::PartMetFile::open(&met_path).unwrap();
    assert_eq!(met.file_hash.0, file_hash);
    assert_eq!(met.file_name.as_deref(), Some("sample.bin"));
    assert_eq!(met.file_size, Some(1000));
    assert_eq!(met.gaps, vec![(0, 1000)]);
    assert_eq!(
        met.compute_completed_ranges().unwrap(),
        Vec::<(u64, u64)>::new()
    );

    // After injecting data the met reflects covered ranges (no gaps).
    svc.inject_range(&snap.id, 0, &content).unwrap();
    let met = mule_format::PartMetFile::open(&met_path).unwrap();
    assert_eq!(met.transferred, 1000);
    assert!(met.gaps.is_empty());
    assert_eq!(
        met.compute_completed_ranges().unwrap(),
        vec![(0u64, 1000u64)]
    );
}

#[test]
fn part_met_is_deleted_on_remove_with_files() {
    let tmp = tempdir().unwrap();
    let mut svc = DownloadService::in_memory(tmp.path().join("temp")).unwrap();

    let link = format!("ed2k://|file|gone.bin|500|{}|/", hex::encode([0x77u8; 16]));
    let snap = svc
        .add_ed2k_link(&link, tmp.path(), AllocationPolicy::None, None)
        .unwrap();
    let (temp_path, _, _, _) = svc.get_download_file_info(&snap.id).unwrap();
    let mut met_name = temp_path.as_os_str().to_os_string();
    met_name.push(".met");
    let met_path = std::path::PathBuf::from(met_name);
    assert!(met_path.exists());

    svc.remove_download(&snap.id, true).unwrap();
    assert!(
        !met_path.exists(),
        ".part.met must be removed with delete_files"
    );
    assert!(!temp_path.exists());
}

#[test]
fn part_met_persists_partial_gaps() {
    let tmp = tempdir().unwrap();
    let mut svc = DownloadService::in_memory(tmp.path().join("temp")).unwrap();

    let link = format!(
        "ed2k://|file|sparse.bin|300|{}|/",
        hex::encode([0x33u8; 16])
    );
    let snap = svc
        .add_ed2k_link(&link, tmp.path(), AllocationPolicy::None, None)
        .unwrap();

    // Partial data does not trigger part completion; force a save via pause.
    svc.inject_range(&snap.id, 0, &[0xAAu8; 100]).unwrap();
    svc.pause_download(&snap.id).unwrap();

    let (temp_path, _, _, _) = svc.get_download_file_info(&snap.id).unwrap();
    let mut met_name = temp_path.as_os_str().to_os_string();
    met_name.push(".met");
    let met = mule_format::PartMetFile::open(std::path::PathBuf::from(met_name)).unwrap();
    assert_eq!(met.gaps, vec![(100, 300)]);
    assert_eq!(
        met.compute_completed_ranges().unwrap(),
        vec![(0u64, 100u64)]
    );
    assert_eq!(met.transferred, 100);
}
