//! The integration tests of `mosaica-build`, one module per file, linked as one test binary.
//! A file in this directory runs only once it is declared below.

mod common;

mod attribute_pass;
mod attribute_sources;
mod build_equivalence;
mod build_layers;
mod build_smoke;
mod check_inline_artifacts;
mod check_shape_views;
mod check_unique_key;
mod coordinate_width;
mod derived_names;
mod discovered_vocabulary;
mod empty_bundle;
mod extent_route;
mod filter_postings;
mod identity_rule;
mod keyword_column;
mod multi_view_build;
mod named_fields;
mod projected_build;
mod record_blob;
mod render_presence;
mod render_tail;
mod scoped_layer_keys;
mod several_shards;
mod shape_span;
mod term_images_build;
mod term_sweep_pairs;
mod text_index;
mod unique_build;
mod verify_deep;
