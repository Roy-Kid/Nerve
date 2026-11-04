//! The `openURL` wire contract, from both ends.
//!
//! The POSIX literals below are what `nerve-hub` has always emitted. Four
//! independent decoders read them — this crate, `Nerve/…/ActionService.swift`,
//! `vsc-ext/src/model/job.ts`, and the published docs — so changing one is a
//! contract break, not a refactor. Windows-shaped inputs are the only place new
//! output is allowed.

use nerve_platform::path::{from_file_uri, to_file_uri};

#[test]
fn posix_output_is_frozen() {
    assert_eq!(
        to_file_uri("/Users/me/proj").as_deref(),
        Some("file:///Users/me/proj")
    );
}

#[test]
fn posix_percent_escapes_are_frozen() {
    assert_eq!(
        to_file_uri("/tmp/100%/done").as_deref(),
        Some("file:///tmp/100%25/done")
    );
    assert_eq!(
        to_file_uri("/Users/me/proj foo").as_deref(),
        Some("file:///Users/me/proj%20foo")
    );
}

#[test]
fn a_windows_drive_keeps_its_colon_and_gains_the_rfc_slash() {
    assert_eq!(
        to_file_uri("C:\\work\\nerve").as_deref(),
        Some("file:///C:/work/nerve")
    );
    assert_eq!(
        to_file_uri("c:/work/nerve").as_deref(),
        Some("file:///c:/work/nerve")
    );
}

#[test]
fn every_shape_round_trips() {
    for path in [
        "/Users/me/proj",
        "/tmp/100%/done",
        "/Users/me/proj foo",
        "/Users/me/项目",
        "/a-b_c.d~e/9Z",
        "/tmp/a\\b",
        "C:\\work\\nerve",
        "C:\\work\\项目",
        "C:\\work\\a b",
    ] {
        let uri = to_file_uri(path).expect("absolute path encodes");
        let back = from_file_uri(&uri).expect("its own output decodes");
        let expected = path.replace('\\', "/");
        let expected = if path.starts_with('/') {
            path
        } else {
            &expected
        };
        assert_eq!(back, expected, "round trip of {path} via {uri}");
    }
}

#[test]
fn a_unc_path_round_trips_to_backslashes() {
    let uri = to_file_uri("\\\\srv\\share\\proj").unwrap();
    assert_eq!(from_file_uri(&uri).as_deref(), Some("\\\\srv\\share\\proj"));
}

#[test]
fn a_drive_survives_both_slash_counts_and_an_escaped_colon() {
    // Shapes seen in the wild from editors and producers.
    for uri in ["file:///C:/work", "file://C:/work", "file:///C%3A/work"] {
        assert_eq!(from_file_uri(uri).as_deref(), Some("C:/work"), "{uri}");
    }
}
