//! The integration tests of `mosaica-cli`, one module per file, linked as one test binary.
//! A file in this directory runs only once it is declared below.

mod common;

mod artifact_fixture_smoke;
mod build_invocation;
mod catalogue_cli;
mod check_and_reports;
mod corpus_cli;
mod identity_cli;
mod large_utf8_columns;
mod minted_views_build;
mod multiview_build;
mod records_cli;
mod refused_rows_cli;
mod serve_process;
mod version;
mod writes_cli;
