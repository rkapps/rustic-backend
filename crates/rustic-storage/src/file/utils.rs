use std::fmt::Display;
use std::path::{Path, PathBuf};

/// Returns `"<id>.json"`.
pub fn json_filename<T: Display>(id: T) -> String {
    format!("{}.json", id)
}

/// Returns `<base>/<id>.json` as a [`PathBuf`].
pub fn build_json_file_path<T: Display>(base: &Path, id: T) -> PathBuf {
    base.join(json_filename(id))
}
