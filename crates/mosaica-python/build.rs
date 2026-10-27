//! macOS links a cdylib only when every symbol resolves, and an extension module leaves
//! libpython's symbols to the interpreter that loads it; this passes `-undefined dynamic_lookup`
//! there. It adds nothing on Linux.

fn main() {
    pyo3_build_config::add_extension_module_link_args();
}
