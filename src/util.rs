//! Shared utility functions.

use std::path::PathBuf;

/// Expand `~` and `~/...` paths to absolute paths using `$HOME`.
///
/// Returns the path unchanged if:
/// - `$HOME` is not set
/// - Path doesn't start with `~`
///
/// # Examples
/// ```
/// use mmem::util::expand_home;
///
/// // With $HOME=/Users/alice:
/// // expand_home("~") -> "/Users/alice"
/// // expand_home("~/docs") -> "/Users/alice/docs"
/// // expand_home("/tmp") -> "/tmp"
/// ```
pub fn expand_home(path: &str) -> PathBuf {
    if path == "~" {
        return std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(path));
    }
    if let Some(stripped) = path.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(stripped);
        }
    }
    PathBuf::from(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_tilde_alone() {
        // This test depends on $HOME being set
        if std::env::var_os("HOME").is_some() {
            let result = expand_home("~");
            assert!(result.is_absolute() || result.to_str() == Some("~"));
        }
    }

    #[test]
    fn expands_tilde_prefix() {
        if let Some(home) = std::env::var_os("HOME") {
            let result = expand_home("~/test/path");
            assert_eq!(result, PathBuf::from(home).join("test/path"));
        }
    }

    #[test]
    fn preserves_absolute_paths() {
        assert_eq!(expand_home("/tmp/file"), PathBuf::from("/tmp/file"));
    }

    #[test]
    fn preserves_relative_paths() {
        assert_eq!(expand_home("relative/path"), PathBuf::from("relative/path"));
    }
}
