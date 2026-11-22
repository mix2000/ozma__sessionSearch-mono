//! Makes sure `web/dist` exists so `rust-embed` and change tracking work on a
//! fresh clone before the UI is built. The UI build fills it later; the binary
//! then embeds whatever is there (an empty folder means a built-in placeholder
//! page is served).

use std::path::Path;

fn main() {
    let dist = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/dist");
    if !dist.exists() {
        let _ = std::fs::create_dir_all(&dist);
    }
    println!("cargo:rerun-if-changed=../../web/dist");
    println!("cargo:rerun-if-changed=build.rs");
}
