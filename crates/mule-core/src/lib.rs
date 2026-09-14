pub mod error;
pub mod model;
pub mod persistence;
pub mod ranges;
pub mod storage;
pub mod verification;

pub use error::CoreError;
pub use model::{
    Download, DownloadCategory, DownloadStatus, HistoryEntry, HistoryFilter, HistoryStats,
};
pub use persistence::Database;
pub use ranges::RangeSet;
pub use storage::{finalize_file, AllocationPolicy, CollisionStrategy, PartFile};
pub use verification::{verify_aich_block, verify_part_hash};
