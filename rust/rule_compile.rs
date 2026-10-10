//! Bounded dictionary rule parsing and bytecode assembly.
// Copyright (C) 2005-2014 Jonathan Duddington; Rust adaptation (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later

pub const INPUT_BYTES: usize = 500;
pub const PART_BYTES: usize = 80;
pub const OUTPUT_BYTES: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Diagnostic {
    LetterDigits = 1,
    LetterUndefined,
    DollarUnknown,
    SpaceAfterParen,
    Syntax,
    EncodedOverflow,
    BadPhoneme,
    WrongGroup,
    BadCondition,
    Overflow,
    TruncatedEscape,
}

/// Effects receive owned scratch; no input or destination loan survives a call.
pub trait Host {
    fn encode(&mut self, input: &[u8], output: &mut [u8; PART_BYTES]) -> Option<i32>;
    fn digit(&mut self, code: u32) -> bool;
    fn diagnostic(&mut self, kind: Diagnostic, value: i32, text: &[u8]);
}

pub struct Environment<'a> {
    pub group: &'a [u8],
    pub letters: &'a [u8; 95],
    pub spaces: &'a [u8; 256],
    pub line: i32,
    pub debug: bool,
    /// Preserve the compiler target's plain-char conversion for Lnn diagnostics.
    pub signed_char: bool,
}

#[derive(Clone)]
struct Text {
    bytes: [u8; PART_BYTES],
    length: usize,
}
impl Text {
    fn new() -> Self {
        Self {
            bytes: [0; PART_BYTES],
            length: 0,
        }
    }
    fn push(&mut self, byte: u8) -> Result<(), Diagnostic> {
        if self.length >= PART_BYTES - 1 {
            return Err(Diagnostic::Overflow);
        }
        self.bytes[self.length] = byte;
        self.length += 1;
        self.bytes[self.length] = 0;
        Ok(())
    }
    fn string(&self) -> &[u8] {
        &self.bytes[..self.bytes.iter().position(|&b| b == 0).unwrap()]
    }
    fn terminated(&self) -> &[u8] {
        &self.bytes[..self.string().len() + 1]
    }
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
const DOLLARS: &[(&[u8], u8)] = &[
    (b"unpr", 1),
    (b"noprefix", 2),
    (b"list", 3),
    (b"w_alt1", 0x11),
    (b"w_alt2", 0x12),
    (b"w_alt3", 0x13),
    (b"w_alt4", 0x14),
    (b"w_alt5", 0x15),
    (b"w_alt6", 0x16),
    (b"w_alt", 0x11),
    (b"p_alt1", 0x21),
    (b"p_alt2", 0x22),
    (b"p_alt3", 0x23),
    (b"p_alt4", 0x24),
    (b"p_alt5", 0x25),
    (b"p_alt6", 0x26),
    (b"p_alt", 0x21),
];

fn copy_part(
    host: &mut impl Host,
    env: &Environment<'_>,
    parts: &mut [Text; 5],
    token: &Text,
    state: &mut usize,
) -> Result<(), Diagnostic> {
    let source = token.terminated();
    if source[0] == 0 {
        return Ok(());
    }
    let mut output = Text::new();
    if *state == 4 && !parts[4].string().is_empty() {
        for &byte in parts[4].string() {
            output.push(byte)?;
        }
        output.push(b' ')?;
    }
    let mut position = 0;
    let mut hexadecimal = false;
    let mut suffix = 0x808000u32;
    loop {
        let mut byte = *source.get(position).ok_or(Diagnostic::TruncatedEscape)?;
        position += 1;
        let mut literal = false;
        let at = |i: usize| source.get(i).copied().unwrap_or(0);
        if byte == b'0'
            && at(position) == b'x'
            && hex(at(position + 1)).is_some()
            && hex(at(position + 2)).is_some()
        {
            hexadecimal = true;
            byte = at(position + 1);
            position += 2;
        }
        if byte == b'\\' {
            byte = *source.get(position).ok_or(Diagnostic::TruncatedEscape)?;
            position += 1;
            if (b'0'..=b'3').contains(&byte)
                && (b'0'..=b'7').contains(&at(position))
                && (b'0'..=b'7').contains(&at(position + 1))
            {
                byte = (byte - b'0') * 64 + (at(position) - b'0') * 8 + (at(position + 1) - b'0');
                position += 2;
            }
            literal = true;
        }
        if hexadecimal {
            if let (Some(first), Some(second)) = (hex(byte), hex(at(position))) {
                byte = first * 16 + second;
                position += 1;
                literal = true;
            } else {
                hexadecimal = false;
            }
        }
        if matches!(*state, 1 | 3) && !literal {
            byte = match byte {
                b'_' => 32,
                b'A' | b'B' | b'C' | b'H' | b'F' | b'G' | b'Y' => {
                    let group = match byte {
                        b'A' => 0,
                        b'B' => 1,
                        b'C' => 2,
                        b'H' => 3,
                        b'F' => 4,
                        b'G' => 5,
                        _ => 6,
                    } + b'A';
                    if *state == 1 {
                        output.push(group)?;
                        17
                    } else {
                        output.push(17)?;
                        group
                    }
                }
                b'D' => 15,
                b'K' => 25,
                b'N' => 24,
                b'V' => 26,
                b'Z' => 16,
                b'+' => 12,
                b'<' => 60,
                b'@' => 21,
                b'&' => 10,
                b'%' => 11,
                b'#' => 13,
                b'!' => 19,
                b'W' => 31,
                b'X' => 29,
                b'J' => 23,
                b'T' => {
                    output.push(28)?;
                    0x11
                }
                b'L' => {
                    let first = *source.get(position).ok_or(Diagnostic::TruncatedEscape)?;
                    let second = *source
                        .get(position + 1)
                        .ok_or(Diagnostic::TruncatedEscape)?;
                    position += 2;
                    // Assignments to C plain char wrap before validation.
                    let code = first
                        .wrapping_sub(48)
                        .wrapping_mul(10)
                        .wrapping_add(second.wrapping_sub(48));
                    let mut group = if env.signed_char {
                        i32::from(code as i8)
                    } else {
                        i32::from(code)
                    };
                    if !second.is_ascii_digit() {
                        group = 0;
                        host.diagnostic(Diagnostic::LetterDigits, 0, b"\0");
                    } else if group <= 0 || group >= 95 || env.letters[group as usize] == 0 {
                        host.diagnostic(Diagnostic::LetterUndefined, group, b"\0");
                    }
                    let group = (group as u8).wrapping_add(b'A');
                    if *state == 1 {
                        output.push(group)?;
                        18
                    } else {
                        output.push(18)?;
                        group
                    }
                }
                b'$' => {
                    let mut value = 0;
                    for &(name, code) in DOLLARS {
                        if source[position..].starts_with(name) {
                            value = code;
                            position += name.len();
                            break;
                        }
                    }
                    let result = if *state == 1 {
                        output.push(value)?;
                        28
                    } else {
                        output.push(28)?;
                        value
                    };
                    if value == 0 {
                        host.diagnostic(Diagnostic::DollarUnknown, 0, b"\0");
                    }
                    result
                }
                b'P' | b'S' => {
                    if byte == b'P' {
                        suffix |= 0x400;
                    }
                    output.push(14)?;
                    let mut value = 0u32;
                    loop {
                        let c = at(position);
                        if c == 0 || env.spaces[c as usize] != 0 {
                            break;
                        }
                        position += 1;
                        suffix |= match c {
                            b'e' => 0x100,
                            b'i' => 0x200,
                            b'p' => 0x400,
                            b'v' => 0x800,
                            b'd' => 0x1000,
                            b'f' => 0x2000,
                            b'q' => 0x4000,
                            b't' => 0x10000,
                            b'b' => 0x20000,
                            b'a' => 0x40000,
                            b'm' => 0x80000,
                            _ => 0,
                        };
                        if c.is_ascii_digit() {
                            value = value
                                .checked_mul(10)
                                .and_then(|v| v.checked_add(u32::from(c - b'0')))
                                .filter(|&v| v <= i32::MAX as u32)
                                .ok_or(Diagnostic::Overflow)?;
                        }
                    }
                    output.push((suffix >> 16) as u8)?;
                    output.push((suffix >> 8) as u8)?;
                    (value | 0x80) as u8
                }
                _ => byte,
            };
        }
        if byte == 0 {
            break;
        }
        output.push(byte)?;
    }
    parts[*state] = output;
    *state = [2, 2, 4, 4, 4][*state];
    Ok(())
}

fn atoi(bytes: &[u8], spaces: &[u8; 256]) -> Option<i32> {
    let mut index = 0;
    while bytes.get(index).is_some_and(|b| spaces[*b as usize] != 0) {
        index += 1;
    }
    let negative = bytes.get(index) == Some(&b'-');
    if negative || bytes.get(index) == Some(&b'+') {
        index += 1;
    }
    let mut value = 0i32;
    while let Some(&byte) = bytes.get(index).filter(|b| b.is_ascii_digit()) {
        value = value.checked_mul(10)?.checked_add(if negative {
            -i32::from(byte - b'0')
        } else {
            i32::from(byte - b'0')
        })?;
        index += 1;
    }
    Some(value)
}

/// Returns the complete record length, or zero for a blank/rejected rule.
/// The destination is published only after all primitive effects complete.
pub fn compile(
    host: &mut impl Host,
    env: &Environment<'_>,
    input: &[u8],
    output: &mut [u8],
) -> usize {
    match compile_inner(host, env, input, output) {
        Ok(length) => length,
        Err(kind) => {
            host.diagnostic(kind, 0, b"\0");
            0
        }
    }
}
fn compile_inner(
    host: &mut impl Host,
    env: &Environment<'_>,
    input: &[u8],
    output: &mut [u8],
) -> Result<usize, Diagnostic> {
    if input.len() >= INPUT_BYTES || env.group.len() > 12 {
        return Err(Diagnostic::Overflow);
    }
    let mut parts: [Text; 5] = std::array::from_fn(|_| Text::new());
    let mut token = Text::new();
    let mut state = 2;
    for index in 0..=input.len() {
        match input.get(index).copied().unwrap_or(0) {
            b')' => {
                state = 1;
                copy_part(host, env, &mut parts, &token, &mut state)?;
                token = Text::new();
            }
            b'(' => {
                state = 2;
                copy_part(host, env, &mut parts, &token, &mut state)?;
                state = 3;
                token = Text::new();
                if input.get(index + 1) == Some(&b' ') {
                    host.diagnostic(Diagnostic::SpaceAfterParen, 0, b"\0");
                }
            }
            0 | b'\n' | b'\r' => {
                copy_part(host, env, &mut parts, &token, &mut state)?;
                break;
            }
            b' ' | b'\t' => {
                copy_part(host, env, &mut parts, &token, &mut state)?;
                token = Text::new();
            }
            b'?' if state == 2 => {
                state = 0;
            }
            byte => token.push(byte)?,
        }
    }
    if parts[2].string() == b"$group" {
        parts[2] = Text::new();
        for &byte in env.group {
            parts[2].push(byte)?;
        }
    }
    if parts[2].string().is_empty() {
        if !parts[3].string().is_empty() {
            host.diagnostic(Diagnostic::Syntax, 0, b"\0");
        }
        return Ok(0);
    }
    let mut encoded = [0u8; PART_BYTES];
    let Some(bad) = host.encode(parts[4].terminated(), &mut encoded) else {
        return Err(Diagnostic::EncodedOverflow);
    };
    if bad != 0 {
        let mut original = [0u8; INPUT_BYTES];
        original[..input.len()].copy_from_slice(input);
        host.diagnostic(Diagnostic::BadPhoneme, bad, &original[..input.len() + 1]);
    }
    let phoneme_len = encoded
        .iter()
        .position(|&b| b == 0)
        .ok_or(Diagnostic::EncodedOverflow)?
        + 1;
    if !env.group.is_empty()
        && !parts[2].string().starts_with(env.group)
        && !(env.group.first() == Some(&b'9')
            && host.digit(crate::word_key::decode(parts[2].terminated(), 0).0))
    {
        host.diagnostic(Diagnostic::WrongGroup, 0, parts[2].terminated());
    }
    let mut scratch = [0u8; OUTPUT_BYTES];
    let mut length: usize = 0;
    let mut append = |bytes: &[u8]| -> Result<(), Diagnostic> {
        let end = length
            .checked_add(bytes.len())
            .filter(|&n| n <= OUTPUT_BYTES)
            .ok_or(Diagnostic::Overflow)?;
        scratch[length..end].copy_from_slice(bytes);
        length = end;
        Ok(())
    };
    append(&encoded[..phoneme_len])?;
    append(parts[2].string())?;
    if env.debug {
        append(&[9, (env.line % 255 + 1) as u8, (env.line / 255 + 1) as u8])?;
    }
    let condition = parts[0].string();
    if !condition.is_empty() {
        let value = if condition[0] == b'!' {
            atoi(&condition[1..], env.spaces).and_then(|v| v.checked_add(32))
        } else {
            atoi(condition, env.spaces)
        }
        .ok_or(Diagnostic::Overflow)?;
        if (1..255).contains(&value) {
            append(&[5, value as u8])?;
        } else {
            host.diagnostic(Diagnostic::BadCondition, value, b"\0");
        }
    }
    let pre = parts[1].string();
    if !pre.is_empty() {
        let start = usize::from(pre[0] == 32);
        append(&[if start == 1 { 8 } else { 1 }])?;
        for &byte in pre[start..].iter().rev() {
            append(&[byte])?;
        }
    }
    if !parts[3].string().is_empty() {
        append(&[2])?;
        append(parts[3].string())?;
    }
    append(&[0])?;
    if output.len() < length {
        return Err(Diagnostic::Overflow);
    }
    output[..length].copy_from_slice(&scratch[..length]);
    Ok(length)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Effects {
        reports: Vec<(Diagnostic, i32, Vec<u8>)>,
        calls: usize,
        fail: bool,
    }
    impl Host for Effects {
        fn encode(&mut self, input: &[u8], output: &mut [u8; 80]) -> Option<i32> {
            self.calls += 1;
            if self.fail {
                return None;
            }
            output[0] = if input[0] == 0 { 0 } else { 42 };
            output[1] = 0;
            Some(0)
        }
        fn digit(&mut self, code: u32) -> bool {
            (48..=57).contains(&code) || (0x966..=0x96f).contains(&code)
        }
        fn diagnostic(&mut self, kind: Diagnostic, value: i32, text: &[u8]) {
            self.reports.push((kind, value, text.to_vec()));
        }
    }
    fn run(
        input: &[u8],
        group: &[u8],
        debug: bool,
        output: &mut [u8],
        host: &mut Effects,
    ) -> usize {
        let mut spaces = [0; 256];
        for c in b" \t\r\n" {
            spaces[*c as usize] = 1;
        }
        compile(
            host,
            &Environment {
                group,
                letters: &[1; 95],
                spaces: &spaces,
                line: 255,
                debug,
                signed_char: true,
            },
            input,
            output,
        )
    }
    #[test]
    fn reverse_pre_context_and_pack_conditions_debug_and_suffix() {
        let mut host = Effects::default();
        let mut output = [0; 512];
        let length = run(b"?!2 _A)a(Sep2 a", b"a", true, &mut output, &mut host);
        assert_eq!(
            &output[..length],
            &[42, 0, b'a', 9, 1, 2, 5, 34, 8, 17, b'A', 2, 14, 0x80, 0x85, 0x82, 0]
        );
        assert!(host.reports.is_empty());
        assert_eq!(host.calls, 1);
    }
    #[test]
    fn first_nul_terminates_converted_context_and_hex_literal() {
        let mut host = Effects::default();
        let mut output = [0; 512];
        let length = run(b"0x610062 a", b"a", false, &mut output, &mut host);
        assert_eq!(&output[..length], &[42, 0, b'a', 0]);
        let length = run(b"a($wrong a", b"a", false, &mut output, &mut host);
        assert_eq!(&output[..length], &[42, 0, b'a', 2, 28, 0]);
        assert_eq!(host.reports[0].0, Diagnostic::DollarUnknown);
    }
    #[test]
    fn numeric_groups_accept_unicode_digits_and_group_alias() {
        let mut host = Effects::default();
        let mut output = [0; 512];
        assert!(run("१२ a".as_bytes(), b"9x", false, &mut output, &mut host) > 0);
        assert!(host.reports.is_empty());
        let length = run(b"$group a", b"abc", false, &mut output, &mut host);
        assert_eq!(&output[..length], &[42, 0, b'a', b'b', b'c', 0]);
    }
    #[test]
    fn blank_rule_skips_encoder_and_reports_post_syntax() {
        let mut host = Effects::default();
        let mut output = [0x55; 512];
        assert_eq!(run(b"(A a", b"", false, &mut output, &mut host), 0);
        assert_eq!(host.calls, 0);
        assert_eq!(host.reports[0].0, Diagnostic::Syntax);
        assert!(output.iter().all(|&b| b == 0x55));
    }
    #[test]
    fn capacity_refusal_retains_effects_without_output_publication() {
        let mut host = Effects::default();
        let mut output = [0x55; 3];
        assert_eq!(run(b"z a", b"a", false, &mut output, &mut host), 0);
        assert_eq!(output, [0x55; 3]);
        assert_eq!(host.calls, 1);
        assert_eq!(
            host.reports.iter().map(|r| r.0).collect::<Vec<_>>(),
            vec![Diagnostic::WrongGroup, Diagnostic::Overflow]
        );
        host.fail = true;
        assert_eq!(run(b"a a", b"a", false, &mut output, &mut host), 0);
        assert_eq!(host.reports.last().unwrap().0, Diagnostic::EncodedOverflow);
    }
    #[test]
    fn expansion_and_numeric_overflow_and_truncated_letter_refused() {
        for input in [
            b"a(L a".as_slice(),
            b"?999999999999999999999 a a",
            b"a(S99999999999999 a",
            b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA)a a",
        ] {
            let mut host = Effects::default();
            let mut output = [0x55; 512];
            assert_eq!(run(input, b"a", false, &mut output, &mut host), 0);
            assert!(output.iter().all(|&b| b == 0x55));
            assert_eq!(host.calls, usize::from(input.starts_with(b"?")));
        }
    }
    #[test]
    fn invalid_letter_group_diagnostics_follow_target_char_signedness() {
        for (signed_char, expected) in [(true, -47), (false, 209)] {
            let mut host = Effects::default();
            let mut output = [0; 512];
            let env = Environment {
                group: b"a",
                letters: &[1; 95],
                spaces: &[0; 256],
                line: 1,
                debug: false,
                signed_char,
            };
            assert!(compile(&mut host, &env, b"Lx1)a a", &mut output) > 0);
            assert_eq!(host.reports[0].0, Diagnostic::LetterUndefined);
            assert_eq!(host.reports[0].1, expected);
        }
    }
}
