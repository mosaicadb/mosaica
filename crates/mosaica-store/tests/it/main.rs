//! The integration tests of `mosaica-store`, one module per file, linked as one test binary.
//! A file in this directory runs only once it is declared below.

mod fixture;

mod bundle_read;
mod cell_histogram;
mod empty_shard;
mod flush_segment;
mod fold_row_space;
mod incremental_bundle;
mod manifest_write;
mod merge_execution;
mod merge_props;
mod merge_selection;
mod pairs_parquet;
mod permutation_extents;
mod permutation_paged;
mod permutation_project_parallel;
mod reclaim;
mod row_entity;
mod segment_roundtrip;
mod term_images;
