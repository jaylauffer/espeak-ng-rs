//! Native engine character extensions and byte-word helpers.
// Copyright (C) 2005-2013 Jonathan Duddington, 2013-2017 Reece H. Dunn.
// Rust adaptation (C) 2026. SPDX-License-Identifier: GPL-3.0-or-later
use crate::unicode;
pub fn emoji(code: u32) -> bool {
    matches!(code,0x2194..=0x2199|0x21a9|0x21aa|0x2600..=0x27bf|0x2b05..=0x2b07|0x2b1b|0x2b1c|0x2b50|0x2b55|0x1f000..=0x1fbff)
}
pub fn regional_indicator(code: u32) -> bool {
    (0x1f1e6..=0x1f1ff).contains(&code)
}
pub fn emoji_modifier(code: u32) -> bool {
    (0x1f3fb..=0x1f3ff).contains(&code)
}
pub fn emoji_tag(code: u32) -> bool {
    (0xe0020..=0xe007f).contains(&code)
}
/// Locale classification precedes the extended in-word ranges. Emoji remain
/// word characters so dictionary text-mode descriptions can see them.
pub fn word_alpha(code: u32, locale_alpha: impl FnOnce(u32) -> bool) -> bool {
    if locale_alpha(code) {
        return true;
    }
    if code < 0x300 {
        return false;
    }
    if (0x901..=0xdf7).contains(&code) {
        return code & 0x7f < 0x64 || matches!(code, 0xa70 | 0xa71 | 0xd7a..=0xd7f);
    }
    matches!(code,0x5b0..=0x5c2|0x605|0x670|0x64b..=0x65e|0x300..=0x36f|0xf40..=0xfbc|0x1100..=0x11ff|0x2800..=0x28ff|0x3041..=0xa700)
        || emoji(code)
}
/// The old membership helper returns the one-based list index, not just true.
pub fn bracket(code: i32) -> i32 {
    if (0x2014..=0x201f).contains(&code) {
        return 1;
    }
    const BRACKETS: [i32; 16] = [
        40, 41, 91, 93, 123, 125, 60, 62, 34, 39, 96, 0xab, 0xbb, 0x300a, 0x300b, 0xe03c,
    ];
    BRACKETS
        .iter()
        .position(|c| *c == code)
        .map_or(0, |index| index as i32 + 1)
}
pub fn digit09(code: u32) -> bool {
    (48..=57).contains(&code)
}
pub fn digit(code: u32, locale_digit: impl FnOnce(u32) -> bool) -> bool {
    locale_digit(code) || (0x966..=0x96f).contains(&code)
}
/// Preserve the host classifier's raw nonzero result on the fallback path.
pub fn space(code: u32, locale_space: impl FnOnce(u32) -> i32) -> i32 {
    if code == 0 {
        0
    } else if matches!(code,0x2500..=0x259f|0xfff9..=0xffff) {
        1
    } else {
        locale_space(code)
    }
}
pub fn byte_space(code: u32) -> bool {
    code & 0xff != 0 && code <= 32
}
pub fn totally_null(bytes: &[u8]) -> bool {
    !bytes.is_empty() && bytes.iter().all(|c| *c == 0)
}
/// Pack at most four initialized bytes; a NUL ends the word and high bytes
/// retain their bit pattern, including the most significant byte.
pub fn string_word(bytes: &[u8]) -> u32 {
    let mut word = 0u32;
    for (index, byte) in bytes.iter().take(4).take_while(|c| **c != 0).enumerate() {
        word |= u32::from(*byte) << (index * 8);
    }
    word
}
pub fn lower(code: u32, dotless_i: bool) -> u32 {
    if code == 73 && dotless_i {
        0x131
    } else {
        unicode::to_lower(code)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extensions_preserve_locale_precedence_and_raw_bracket_space_values() {
        assert!(word_alpha(65, |_| true));
        assert!(!word_alpha(65, |_| false));
        assert!(word_alpha(0xa70, |_| false));
        assert!(!word_alpha(0xa64, |_| false));
        assert!(word_alpha(0x1f600, |_| false));
        assert!(!word_alpha(0xe007f, |_| false));
        assert_eq!(bracket(93), 4);
        assert_eq!(bracket(0x201f), 1);
        assert_eq!(bracket(0xe03c), 16);
        assert_eq!(space(32, |_| 8192), 8192);
        assert_eq!(space(0, |_| panic!("NUL has no locale lookup")), 0);
        assert!(digit(0x966, |_| false));
        assert!(!digit09(0x966));
        assert!(emoji(0x2195));
        assert!(regional_indicator(0x1f1e6));
        assert!(emoji_modifier(0x1f3ff));
        assert!(emoji_tag(0xe007f));
    }
    #[test]
    fn byte_helpers_keep_nul_high_bytes_and_turkish_lowering() {
        assert!(!byte_space(0));
        assert!(byte_space(1));
        assert!(byte_space(32));
        assert!(!byte_space(0x100));
        assert!(totally_null(&[0; 4]));
        assert!(!totally_null(&[]));
        assert!(!totally_null(&[0, 0, 1]));
        assert_eq!(string_word(b"ab\0z"), 0x6261);
        assert_eq!(string_word(&[255; 5]), u32::MAX);
        assert_eq!(lower(73, true), 0x131);
        assert_eq!(lower(73, false), 105);
        assert_eq!(lower(0x1f600, true), 0x1f600);
    }
}
