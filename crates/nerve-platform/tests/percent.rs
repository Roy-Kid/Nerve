//! Percent coding, at the byte level it actually operates on.

use nerve_platform::path::percent_decode;

#[test]
fn escapes_spell_one_character_together() {
    // Three escapes, one character: decoding them one at a time is what used
    // to hand the Git panel a path that does not exist.
    assert_eq!(
        percent_decode("/Users/me/%E9%A1%B9%E7%9B%AE"),
        "/Users/me/项目"
    );
}

#[test]
fn a_literal_percent_survives_decoding() {
    // `%/d` is not an escape; leaving it alone is what keeps a real directory
    // named `100%` readable.
    assert_eq!(percent_decode("/tmp/100%/done"), "/tmp/100%/done");
}

#[test]
fn invalid_utf8_degrades_rather_than_losing_the_path() {
    // A lone continuation byte cannot be part of any character. The path is
    // still more useful with a replacement character in it than dropped.
    let decoded = percent_decode("/tmp/%FF");
    assert!(decoded.starts_with("/tmp/"), "{decoded}");
}
