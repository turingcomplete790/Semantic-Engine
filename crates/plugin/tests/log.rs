use plugin::log::{format_line, Level};

#[test]
fn each_level_uses_stash_prefix_bytes() {
    for (level, ch) in [
        (Level::Trace, b't'),
        (Level::Debug, b'd'),
        (Level::Info, b'i'),
        (Level::Warning, b'w'),
        (Level::Error, b'e'),
        (Level::Progress, b'p'),
    ] {
        let line = format_line(level, "hello", &[]);
        assert_eq!(&line[..3], &[0x01, ch, 0x02]);
        assert_eq!(&line[3..], b"hello\n");
    }
}

#[test]
fn secrets_are_redacted() {
    let line = format_line(Level::Info, "cookie=abc123 here", &["abc123"]);
    let s = String::from_utf8(line).unwrap();
    assert!(!s.contains("abc123"));
    assert!(s.contains("<redacted>"));
}

#[test]
fn empty_secret_redacts_nothing() {
    let line = format_line(Level::Info, "plain", &[""]);
    assert_eq!(&line[3..], b"plain\n");
}
