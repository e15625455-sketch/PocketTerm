//! Files, package management, and child process primitives for PocketTerm.

use std::path::PathBuf;

/// App-private paths used by PocketTerm.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppPaths {
    /// Root `filesDir` supplied by the Android host.
    pub files_dir: PathBuf,
}

impl AppPaths {
    /// Returns the package prefix under `filesDir/usr`.
    #[must_use]
    pub fn prefix(&self) -> PathBuf {
        self.files_dir.join("usr")
    }
}

#[cfg(test)]
mod tests {
    use super::AppPaths;
    use std::path::PathBuf;

    #[test]
    fn prefix_is_inside_files_dir() {
        let paths = AppPaths {
            files_dir: PathBuf::from("/private/app/files"),
        };
        assert_eq!(paths.prefix(), PathBuf::from("/private/app/files/usr"));
    }
}
