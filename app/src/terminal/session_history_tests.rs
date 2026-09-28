use super::*;

#[test]
fn output_size_counts_lines_and_bytes() {
    assert_eq!(OutputSize::of(""), OutputSize { lines: 0, bytes: 0 });
    assert_eq!(OutputSize::of("one"), OutputSize { lines: 1, bytes: 3 });
    // A trailing newline terminates the last line rather than starting a new one.
    assert_eq!(OutputSize::of("one\n"), OutputSize { lines: 1, bytes: 4 });
    assert_eq!(
        OutputSize::of("one\ntwo\n"),
        OutputSize { lines: 2, bytes: 8 }
    );
}

#[test]
fn output_size_counts_bytes_not_chars() {
    // Multi-byte output should report its byte length, matching what writing the
    // block to a file would produce.
    let size = OutputSize::of("héllo");
    assert_eq!(size.bytes, 6);
    assert_eq!(size.lines, 1);
}

#[test]
fn durations_read_like_a_stopwatch() {
    assert_eq!(format_duration(0.0), "0ms");
    assert_eq!(format_duration(0.42), "420ms");
    assert_eq!(format_duration(1.0), "1.0s");
    assert_eq!(format_duration(22.14), "22.1s");
    assert_eq!(format_duration(59.9), "59.9s");
    assert_eq!(format_duration(60.0), "1m 00s");
    assert_eq!(format_duration(64.0), "1m 04s");
    assert_eq!(format_duration(3600.0), "1h 00m");
    assert_eq!(format_duration(3660.0), "1h 01m");
}

#[test]
fn bytes_switch_units_at_1024() {
    assert_eq!(format_bytes(0), "0 B");
    assert_eq!(format_bytes(214), "214 B");
    assert_eq!(format_bytes(1023), "1023 B");
    assert_eq!(format_bytes(1024), "1.0 KB");
    assert_eq!(format_bytes(1536), "1.5 KB");
    // Ten and above drops the decimal so the column stays narrow.
    assert_eq!(format_bytes(10 * 1024), "10 KB");
    assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
    assert_eq!(format_bytes(1024 * 1024 * 1024), "1.0 GB");
    // Beyond the largest unit it keeps counting in GB rather than overflowing.
    assert_eq!(format_bytes(2048 * 1024 * 1024), "2.0 GB");
}

#[test]
fn pluralize_only_omits_the_suffix_at_one() {
    assert_eq!(pluralize(0, "line"), "0 lines");
    assert_eq!(pluralize(1, "line"), "1 line");
    assert_eq!(pluralize(2, "line"), "2 lines");
}

#[test]
fn pluralize_uses_es_after_a_sibilant() {
    assert_eq!(pluralize(1, "match"), "1 match");
    assert_eq!(pluralize(7, "match"), "7 matches");
    assert_eq!(pluralize(0, "match"), "0 matches");
}

#[test]
fn exit_codes_are_decimal_without_the_hex_form() {
    assert_eq!(format_exit_code_inner(1, false), "exit 1");
    assert_eq!(format_exit_code_inner(127, false), "exit 127");
    // Windows status codes overflow i32, so the decimal stays signed.
    assert_eq!(
        format_exit_code_inner(-1073741819, false),
        "exit -1073741819"
    );
}

#[test]
fn exit_codes_gain_the_hex_form_at_the_threshold() {
    // Everything a shell can report fits in 0-255 and reads as decimal.
    assert_eq!(format_exit_code_inner(1, true), "exit 1");
    assert_eq!(format_exit_code_inner(127, true), "exit 127");
    assert_eq!(format_exit_code_inner(255, true), "exit 255");
    // Past that it is a Windows status code, where hex is the form
    // documentation and search results use. 9009 is cmd's "not found".
    assert_eq!(format_exit_code_inner(256, true), "exit 256 (0x100)");
    assert_eq!(format_exit_code_inner(9009, true), "exit 9009 (0x2331)");
}

#[test]
fn crash_codes_show_the_unsigned_hex_microsoft_documents() {
    // The threshold compares the unsigned value: signed, these are far below
    // 256 and would lose the hex that makes them identifiable.
    assert_eq!(
        format_exit_code_inner(-1073741819, true),
        "exit -1073741819 (0xC0000005)"
    );
    assert_eq!(
        format_exit_code_inner(-1073741510, true),
        "exit -1073741510 (0xC000013A)"
    );
}

#[test]
fn row_position_ids_are_distinct_per_block() {
    let first = session_history_row_position_id(BlockIndex(0));
    let second = session_history_row_position_id(BlockIndex(1));
    assert_ne!(first, second);
    assert!(first.starts_with("session_history_row_"));
}
