//! The integration tests of `mosaica-authz`, one module per file, linked as one test binary.
//! A file in this directory runs only once it is declared below.

mod delta_tier;
mod fragment;
mod fragment_cache_key;
mod fragment_deltas;
mod postings_roundtrip;
mod streaming_writers;
mod term_sweep;
mod tier_coalesce;
