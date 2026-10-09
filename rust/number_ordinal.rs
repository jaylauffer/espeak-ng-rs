//! Dot ordinals and Hungarian month/range context decisions.
// SPDX-License-Identifier: GPL-3.0-or-later
const ORDINAL_DOT: u32 = 0x10000;
const FIRST_UPPER: u32 = 2;
const NO_SPACE: u32 = 0x100;
const HAS_DOT: u32 = 0x10000;
const COMMA_AFTER: u32 = 0x20000;
const ALT: u32 = 0x8000;
const ALT3: u32 = 0x20000;

/// Serialized primitive access; source bytes outside the initialized extent
/// are virtual NUL. Reads and writes never hold a foreign borrow across a
/// potentially reentrant translation. Word flags and dictionary state are live.
/// The initialized string is at most 800 bytes including its terminator.
pub trait Host {
    fn byte(&self, offset: isize) -> u8;
    fn space(&mut self, offset: usize);
    /// 0 options, 1 language, 2 current word, 3 next word, 4 previous dictionary.
    fn value(&self, field: u32) -> u32;
    fn alpha(&self, character: u32) -> bool;
    fn digit(&self, character: u32) -> bool;
    fn translate(&mut self, offset: usize) -> u32;
}

fn character(host: &impl Host, mut offset: usize) -> u32 {
    // The legacy decoder skips initial continuation bytes before decoding its
    // permissive head/tails. The adapter bounds reads at the admitted terminator.
    while host.byte(offset as isize) & 0xc0 == 0x80 {
        offset += 1;
    }
    crate::utf8::head(|i| Some(host.byte((offset + i) as isize)))
        .unwrap()
        .code
}

/// `end` identifies a cursor within the host's initialized string.
pub fn dot(host: &mut impl Host, end: usize, roman: bool) -> i32 {
    let next_flags = host.value(3);
    if host.value(0) & ORDINAL_DOT == 0
        || (host.byte(end as isize) != b'.' && host.value(2) & HAS_DOT == 0)
        || next_flags & NO_SPACE != 0
        || (!roman && next_flags & FIRST_UPPER != 0)
    {
        return 0;
    }
    let following = character(
        host,
        if host.byte(end as isize) == b'.' {
            end + 2
        } else {
            end
        },
    );
    if host.byte(end as isize) == 0
        || host.byte(end as isize + 1) == 0
        || !(following == 0 || host.value(2) & COMMA_AFTER != 0 || host.alpha(following))
    {
        return 0;
    }
    let mut ordinal = 2;
    if host.byte(end as isize) == b'.' {
        host.space(end);
    }
    if !roman && host.value(1) == 0x6875 {
        let next_flags = if host.alpha(following) {
            host.translate(end + 2)
        } else {
            0
        };
        if host.value(4) & ALT != 0
            && (following == 0 || host.value(2) & COMMA_AFTER != 0 || host.digit(following))
        {
            ordinal = 0;
        }
        if next_flags & ALT != 0 {
            ordinal = 0;
        }
        if next_flags & ALT3 != 0 {
            if host.byte(-2) == b'-' {
                ordinal = 0;
            }
            if host.value(4) & (ALT | ALT3) != 0 {
                ordinal = 0x22;
            }
        }
    }
    ordinal
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {
        source: [u8; 12],
        state: [u32; 5],
        result: u32,
        calls: usize,
    }
    impl Host for Fixture {
        fn byte(&self, offset: isize) -> u8 {
            usize::try_from(offset + 2)
                .ok()
                .and_then(|i| self.source.get(i))
                .copied()
                .unwrap_or(0)
        }
        fn space(&mut self, offset: usize) {
            self.source[offset + 2] = b' ';
        }
        fn value(&self, field: u32) -> u32 {
            self.state[field as usize]
        }
        fn alpha(&self, c: u32) -> bool {
            c == 0xe9 || (c < 128 && (c as u8).is_ascii_alphabetic())
        }
        fn digit(&self, c: u32) -> bool {
            c < 128 && (c as u8).is_ascii_digit()
        }
        fn translate(&mut self, offset: usize) -> u32 {
            assert_eq!(offset, 3);
            self.calls += 1;
            self.state[4] = ALT3;
            self.result
        }
    }
    fn fixture() -> Fixture {
        Fixture {
            source: *b"  2. month\0\0",
            state: [ORDINAL_DOT, 0x6875, 0, 0, 0],
            result: ALT3,
            calls: 0,
        }
    }
    #[test]
    fn month_translation_uses_updated_dictionary_state_after_dot_mutation() {
        let mut host = fixture();
        host.source[0] = b'-';
        assert_eq!(dot(&mut host, 1, false), 0x22);
        assert_eq!(host.source[3], b' ');
        assert_eq!(host.calls, 1);
    }
    #[test]
    fn roman_bypasses_uppercase_guard_and_month_callback() {
        let mut host = fixture();
        host.state[3] = FIRST_UPPER;
        assert_eq!(dot(&mut host, 1, false), 0);
        assert_eq!(host.source[3], b'.');
        assert_eq!(dot(&mut host, 1, true), 2);
        assert_eq!(host.calls, 0);
    }
    #[test]
    fn continuation_prefix_and_terminal_lookahead_are_bounded() {
        let mut host = fixture();
        host.source[5..9].copy_from_slice(&[0x80, 0xc3, 0xa9, 0]);
        assert_eq!(dot(&mut host, 1, false), 0x22);
        host.source[3] = b'.';
        host.source[4] = 0;
        assert_eq!(dot(&mut host, 1, false), 0);
        assert_eq!(host.source[3], b'.');
    }
}
