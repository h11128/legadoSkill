//! Pattern clustering: structural_hash + verify-ok clusters (§10.5).

mod cluster;
mod fields;
mod hash;
mod load;
mod remain;
mod write;

pub use cluster::{cluster_verify_ok, ClusterSample};
pub use fields::{chapter_list, content_rule, search_book_list, search_url, source_type};
pub use hash::{normalize_search_url_shape, structural_hash, structural_hash_from_source};
pub use load::{sample_from_value, samples_from_json};
pub use remain::{cluster_remain, RemainBucket, RemainClusterReport};
pub use write::{write_cluster_json, WriteError};
