//! Bounded dictionary flag and compiled-rule trace rendering.
// Copyright (C) 2005-2014 Jonathan Duddington, 2015-2017 Reece H. Dunn.
// SPDX-License-Identifier: GPL-3.0-or-later
pub const TEXT_BYTES: usize = 200;
pub const RULE_BYTES: usize = 512;
const FLAGS: &[(&[u8], u32)] = &[
    (b"$1", 65),
    (b"$2", 66),
    (b"$3", 67),
    (b"$4", 68),
    (b"$5", 69),
    (b"$6", 70),
    (b"$7", 71),
    (b"$u", 72),
    (b"$u1", 73),
    (b"$u2", 74),
    (b"$u3", 75),
    (b"$u+", 76),
    (b"$u1+", 77),
    (b"$u2+", 78),
    (b"$u3+", 79),
    (b"$pause", 8),
    (b"$strend", 9),
    (b"$strend2", 10),
    (b"$unstressend", 11),
    (b"$accent_before", 12),
    (b"$abbrev", 13),
    (b"$double", 14),
    (b"$alt", 15),
    (b"$alt1", 15),
    (b"$alt2", 16),
    (b"$alt3", 17),
    (b"$alt4", 18),
    (b"$alt5", 19),
    (b"$alt6", 20),
    (b"$alt7", 21),
    (b"$combine", 23),
    (b"$dot", 24),
    (b"$hasdot", 25),
    (b"$max3", 27),
    (b"$brk", 28),
    (b"$text", 29),
    (b"$verbf", 32),
    (b"$verbsf", 33),
    (b"$nounf", 34),
    (b"$pastf", 35),
    (b"$verb", 36),
    (b"$noun", 37),
    (b"$past", 38),
    (b"$verbextend", 40),
    (b"$capital", 41),
    (b"$allcaps", 42),
    (b"$accent", 43),
    (b"$sentence", 45),
    (b"$only", 46),
    (b"$onlys", 47),
    (b"$stem", 48),
    (b"$atend", 49),
    (b"$atstart", 50),
    (b"$native", 51),
    (b"$?", 100),
    (b"$textmode", 200),
    (b"$phonememode", 201),
];
const DOLLARS: &[(&[u8], u32)] = &[
    (b"unpr", 1),
    (b"noprefix", 2),
    (b"list", 3),
    (b"w_alt1", 17),
    (b"w_alt2", 18),
    (b"w_alt3", 19),
    (b"w_alt4", 20),
    (b"w_alt5", 21),
    (b"w_alt6", 22),
    (b"w_alt", 17),
    (b"p_alt1", 33),
    (b"p_alt2", 34),
    (b"p_alt3", 35),
    (b"p_alt4", 36),
    (b"p_alt5", 37),
    (b"p_alt6", 38),
    (b"p_alt", 33),
];
const SYMBOLS: &[u8; 32] = b"          &%+#SDZAL! @?JNKV?TX?W";
fn name(table: &'static [(&'static [u8], u32)], value: u32) -> &'static [u8] {
    table
        .iter()
        .find_map(|(name, id)| (*id == value).then_some(*name))
        .unwrap_or(b"")
}
struct Text<const N: usize> {
    bytes: [u8; N],
    used: usize,
}
impl<const N: usize> Text<N> {
    fn new() -> Self {
        Self {
            bytes: [0; N],
            used: 0,
        }
    }
    fn append(&mut self, bytes: &[u8]) -> Option<()> {
        let end = self.used.checked_add(bytes.len())?;
        if end >= N {
            return None;
        }
        self.bytes[self.used..end].copy_from_slice(bytes);
        self.used = end;
        self.bytes[end] = 0;
        Some(())
    }
    fn number(&mut self, mut value: u32, width: usize) -> Option<()> {
        let mut digits = [0; 10];
        let mut start = digits.len();
        loop {
            start -= 1;
            digits[start] = b'0' + (value % 10) as u8;
            value /= 10;
            if value == 0 {
                break;
            }
        }
        for _ in digits.len() - start..width {
            self.append(b" ")?;
        }
        self.append(&digits[start..])
    }
    fn string(&self) -> &[u8] {
        &self.bytes[..self.used]
    }
    fn publish(&self, output: &mut [u8]) -> Option<usize> {
        let destination = output.get_mut(..=self.used)?;
        destination.copy_from_slice(&self.bytes[..=self.used]);
        Some(self.used)
    }
}
/// Preserve stress-first names, first synonym, unknown-bit spaces and the
/// cumulative budget (including skipped names). Failure leaves output intact.
pub fn flags(words: [u32; 2], output: &mut [u8]) -> Option<usize> {
    if output.is_empty() {
        return None;
    }
    let mut text = Text::<RULE_BYTES>::new();
    let stress = words[0] & 15;
    if stress != 0 {
        text.append(name(FLAGS, stress + 64))?;
    }
    let mut total = text.used;
    if total >= output.len() {
        return None;
    }
    for bit in 8..64 {
        let set = if bit < 30 {
            words[0] & (1u32 << bit) != 0
        } else if bit >= 32 {
            words[1] & (1u32 << (bit - 32)) != 0
        } else {
            false
        };
        if !set {
            continue;
        }
        let label = name(FLAGS, bit);
        total = total.checked_add(label.len() + 1)?;
        if total >= output.len() {
            continue;
        }
        text.append(b" ")?;
        text.append(label)?;
    }
    text.publish(output)
}
fn plain(byte: u8, signed: bool) -> i32 {
    if signed {
        byte as i8 as i32
    } else {
        byte as i32
    }
}
/// Columns is the historical pointer-width display budget in the C adapter
/// (C took `sizeof(output)` of a pointer parameter). Typed callers can supply
/// their actual display width. The legacy eight-column minimum and bytewise
/// pre-context reversal are retained. When the line number, condition and
/// pre-context already pass the budget, C's truncation index
/// (`buf[p_end - p]`) is negative: it writes before its scratch buffer and
/// truncates nothing, so the whole match follows. That is what C prints, so the
/// whole match follows here too, with no write outside the output. On 32-bit
/// targets (four columns) any condition or pre-context reaches this case.
pub fn rule(
    group: &[u8],
    encoded: &[u8],
    control: u32,
    signed: bool,
    columns: usize,
    output: &mut [u8],
) -> Option<usize> {
    if group.len() >= TEXT_BYTES || columns == 0 || columns > RULE_BYTES {
        return None;
    }
    let mut root = Text::<TEXT_BYTES>::new();
    root.append(&group[..group.iter().position(|b| *b == 0).unwrap_or(group.len())])?;
    let mut pre = Text::<TEXT_BYTES>::new();
    let mut before = false;
    let mut at_start = false;
    let mut line = 0i32;
    let mut condition = 0i32;
    let mut offset = 0;
    let mut take = || {
        if offset >= RULE_BYTES {
            return None;
        }
        let b = *encoded.get(offset)?;
        offset += 1;
        Some(b)
    };
    loop {
        let byte = take()?;
        if byte <= 9 {
            match byte {
                0 | 3 => break,
                1 | 8 => {
                    at_start |= byte == 8;
                    before = true;
                    pre = Text::new();
                }
                2 => {
                    before = false;
                    root.append(b" (")?;
                }
                5 => condition = plain(take()?, signed),
                9 => {
                    let first = take()? as i32 - 1;
                    let second = take()? as i32 - 1;
                    line = first + second * 255;
                }
                _ => {}
            }
            continue;
        }
        let text = if before { &mut pre } else { &mut root };
        match byte {
            28 => {
                let value = take()? as u32;
                if value != 1 || control & 0x8000_0000 != 0 {
                    text.append(b"$")?;
                    text.append(name(DOLLARS, value))?;
                }
                text.append(b" ")?;
            }
            14 => {
                let mut bits = ((take()? & 127) as u32) * 256 + (take()? & 127) as u32;
                text.append(if bits & 4 != 0 { b"P" } else { b"S" })?;
                text.number((take()? & 127) as u32, 0)?;
                for flag in b"eipvdfq t" {
                    if bits & 1 != 0 {
                        text.append(&[*flag])?;
                    }
                    bits >>= 1;
                }
                text.append(b" ")?;
            }
            17 => {
                let index = plain(take()?, signed) - b'A' as i32;
                text.append(&[*b"ABCHFGY".get(usize::try_from(index).ok()?)?])?;
            }
            18 => {
                let value = (plain(take()?, signed) - b'A' as i32).rem_euclid(256) as u8;
                let digits = [b'L', value / 10 + b'0', value % 10 + b'0'];
                if before {
                    text.append(&[digits[2], digits[1], digits[0]])?;
                } else {
                    text.append(&digits)?;
                }
            }
            10..=31 => text.append(&[SYMBOLS[byte as usize]])?,
            32 => text.append(b"_")?,
            _ => text.append(&[byte])?,
        }
    }
    let mut result = Text::<RULE_BYTES>::new();
    if line > 0 {
        result.number(line as u32, 5)?;
        result.append(b":\t")?;
    }
    if condition > 0 {
        result.append(b"?")?;
        result.number(condition as u32, 0)?;
        result.append(b" ")?;
    }
    if pre.used > 0 || at_start {
        if at_start {
            result.append(b"_")?;
        }
        for byte in pre.string().iter().rev() {
            if result.used >= columns.saturating_sub(4) {
                break;
            }
            result.append(&[*byte])?;
        }
        result.append(b") ")?;
    }
    let budget = columns.saturating_sub(1);
    let remaining = if result.used > budget {
        root.used
    } else {
        (budget - result.used).min(root.used)
    };
    result.append(&root.string()[..remaining])?;
    while result.used < 8 {
        result.append(b" ")?;
    }
    result.publish(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flags_keep_first_synonyms_unknown_spaces_and_cumulative_omissions() {
        let mut out = [0x55; 80];
        let n = flags([1 | (1 << 15), 1 << 7], &mut out).unwrap();
        assert_eq!(&out[..n], b"$1 $alt ");
        let mut short = [0x55; 9];
        let n = flags([1 | (1 << 12) | (1 << 15), 0], &mut short).unwrap();
        assert_eq!(&short[..n], b"$1");
        assert_eq!(&short[n + 1..], [0x55; 6]);
        let mut tiny = [0x55; 2];
        assert!(flags([15, 0], &mut tiny).is_none());
        assert_eq!(tiny, [0x55; 2]);
    }
    #[test]
    fn pointer_columns_reverse_pre_context_and_preserve_dollar_suffix_symbols() {
        let mut out = [0x55; 80];
        let n = rule(b"a", &[1, b'x', b'y', 2, b'z', 3], 0, true, 8, &mut out).unwrap();
        assert_eq!(&out[..n], b"yx) a ( ");
        let n = rule(
            b"a",
            &[18, b'A' + 94, 28, 1, 14, 0, 4, 2, 32, 3],
            0,
            true,
            80,
            &mut out,
        )
        .unwrap();
        assert_eq!(&out[..n], b"aL94 P2p _");
    }
    #[test]
    fn metadata_past_the_column_budget_keeps_the_whole_match() {
        let mut out = [0x55; 80];
        let n = rule(b"a", &[9, 2, 1, 5, 1, 3], 0, true, 8, &mut out).unwrap();
        assert_eq!(&out[..n], b"    1:\t?1 a");
        assert_eq!(out[n], 0);
        assert!(out[n + 1..].iter().all(|b| *b == 0x55));
        let n = rule(b"a", &[5, 255, 3], 0, true, 8, &mut out).unwrap();
        assert_eq!(&out[..n], b"a       ");
        let n = rule(b"a", &[5, 255, 3], 0, false, 8, &mut out).unwrap();
        assert_eq!(&out[..n], b"?255 a  ");
        // 32-bit: four columns, so a two-digit condition already passes them
        let n = rule(b"ab", &[5, 7, 3], 0, true, 4, &mut out).unwrap();
        assert_eq!(&out[..n], b"?7      ");
        let n = rule(b"ab", &[5, 12, 3], 0, true, 4, &mut out).unwrap();
        assert_eq!(&out[..n], b"?12 ab  ");
    }
    #[test]
    fn malformed_and_short_output_reject_without_publication() {
        for encoded in [&[28][..], &[14, 1, 1], &[17, b'H'], &[18], &[9, 1], b"a"] {
            let mut out = [0x55; 80];
            assert!(rule(b"a", encoded, 0, true, 8, &mut out).is_none());
            assert_eq!(out, [0x55; 80]);
        }
        let mut tiny = [0x55; 8];
        assert!(rule(b"a", &[3], 0, true, 8, &mut tiny).is_none());
        assert_eq!(tiny, [0x55; 8]);
        let mut out = [0x55; 80];
        assert!(rule(&[b'x'; 200], &[3], 0, true, 8, &mut out).is_none());
        assert_eq!(out, [0x55; 80]);
        assert!(rule(b"a", &[b'x'; 512], 0, true, 8, &mut out).is_none());
        assert_eq!(out, [0x55; 80]);
    }
}
