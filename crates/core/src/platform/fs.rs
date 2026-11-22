//! Path probes on the real file system.

use std::path::Path;

use super::FsProbe;

pub struct OsFs;

impl FsProbe for OsFs {
    fn is_dir(&self, path: &Path) -> bool {
        path.is_dir()
    }

    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probes_files_and_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("a");
        std::fs::write(&f, "x").unwrap();
        assert!(OsFs.is_dir(dir.path()) && !OsFs.is_file(dir.path()));
        assert!(OsFs.is_file(&f) && !OsFs.is_dir(&f));
        assert!(!OsFs.is_file(&dir.path().join("missing")));
    }
}
