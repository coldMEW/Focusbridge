pub use focusbridge_core::cert::*;

use std::path::{Path, PathBuf};

pub fn app_cert_dir(app_data_root: &Path) -> PathBuf {
    app_data_root.join("certs")
}
