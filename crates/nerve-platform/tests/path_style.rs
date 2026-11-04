//! What counts as an absolute path, and whose rules it is written in.

use nerve_platform::path::{PathStyle, basename, is_absolute, style_of};

#[test]
fn a_drive_with_either_slash_is_windows() {
    assert_eq!(style_of("C:\\work\\nerve"), Some(PathStyle::Windows));
    assert_eq!(style_of("c:/work/nerve"), Some(PathStyle::Windows));
    assert_eq!(style_of("C:\\"), Some(PathStyle::Windows));
}

#[test]
fn a_drive_relative_path_is_not_absolute() {
    // `C:work` names a directory relative to the current one on drive C.
    // A drive letter alone does not root a path.
    assert_eq!(style_of("C:work"), None);
    assert!(!is_absolute("C:work"));
}

#[test]
fn basename_is_the_last_named_segment() {
    assert_eq!(basename("/Users/me/proj"), Some("proj"));
    assert_eq!(basename("C:\\work\\nerve"), Some("nerve"));
    assert_eq!(basename("c:/work/nerve"), Some("nerve"));
    assert_eq!(basename("\\\\srv\\share\\proj"), Some("proj"));
}

#[test]
fn a_backslash_is_a_filename_character_on_posix() {
    // `a\b` is one legal POSIX filename, not two segments. Splitting on both
    // separators regardless of style would invent a directory here.
    assert_eq!(basename("/tmp/a\\b"), Some("a\\b"));
}
