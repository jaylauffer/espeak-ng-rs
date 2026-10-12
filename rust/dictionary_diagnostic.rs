//! Complete dictionary compiler diagnostic bytes, without allocation or C replay.
// SPDX-License-Identifier: GPL-3.0-or-later
#[derive(Clone, Copy)]
pub enum Family {
    List,
    Rule,
    File,
    Driver,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Route {
    Log,
    Error,
    Flush,
}
pub struct Diagnostic {
    bytes: [u8; 16384],
    length: usize,
    pub increment: bool,
    pub route: Route,
}
impl Diagnostic {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes[..self.length]
    }
    fn add(&mut self, bytes: &[u8]) -> Option<()> {
        let end = self.length.checked_add(bytes.len())?;
        self.bytes.get_mut(self.length..end)?.copy_from_slice(bytes);
        self.length = end;
        Some(())
    }
    fn decimal(&mut self, value: i32, width: usize, precision: usize) -> Option<()> {
        let mut bytes = [0; 11];
        let mut i = bytes.len();
        let mut n = value.unsigned_abs();
        loop {
            i -= 1;
            bytes[i] = b'0' + (n % 10) as u8;
            n /= 10;
            if n == 0 {
                break;
            }
        }
        let digits = bytes.len() - i;
        let zeroes = precision.saturating_sub(digits);
        for _ in 0..width.saturating_sub(digits + zeroes + usize::from(value < 0)) {
            self.add(b" ")?;
        }
        if value < 0 {
            self.add(b"-")?;
        }
        for _ in 0..zeroes {
            self.add(b"0")?;
        }
        self.add(&bytes[i..])
    }
    fn hex(&mut self, value: i32) -> Option<()> {
        let mut bytes = [0; 8];
        let mut i = 8;
        let mut n = value as u32;
        loop {
            i -= 1;
            bytes[i] = b"0123456789abcdef"[(n & 15) as usize];
            n >>= 4;
            if n == 0 {
                break;
            }
        }
        self.add(&bytes[i..])
    }
    fn prefix(&mut self, line: i32) -> Option<()> {
        self.decimal(line, 5, 0)?;
        self.add(b": ")
    }
    fn bad(&mut self, value: i32, first: &[u8], second: Option<&[u8]>) -> Option<()> {
        let (spelling, n) = crate::suffix::encode(value as u32);
        let n = spelling[..n].iter().position(|b| *b == 0).unwrap_or(n);
        self.add(b"Bad phoneme [")?;
        self.add(&spelling[..n])?;
        self.add(b"] (U+")?;
        self.hex(value)?;
        self.add(b") in: ")?;
        self.add(first)?;
        if let Some(second) = second {
            self.add(b"  ")?;
            self.add(second)?;
        }
        self.add(b"\n")
    }
}
/// Inputs are already copied C-string prefixes; bytes need not be UTF-8.
pub fn format(
    family: Family,
    line: i32,
    kind: i32,
    value: i32,
    first: &[u8],
    second: &[u8],
    group: &[u8],
) -> Option<Diagnostic> {
    let mut d = Diagnostic {
        bytes: [0; 16384],
        length: 0,
        increment: true,
        route: Route::Log,
    };
    match family {
        Family::List => {
            if kind != 9 && kind != 10 {
                d.prefix(line)?;
            }
            match kind {
                1 => {
                    d.add(b"Unknown keyword: ")?;
                    d.add(first)?;
                    d.add(b"\n")?;
                }
                2 => d.add(b"Missing '('\n")?,
                3 => {
                    d.add(b"Need to compile dictionary again\n")?;
                    d.increment = false;
                }
                4 => d.add(b"Encoded phonemes exceed output buffer\n")?,
                5 => d.bad(value, first, Some(second))?,
                6 => {
                    d.add(b"Dictionary line length would overflow the data buffer: ")?;
                    d.decimal(value, 0, 0)?;
                    d.add(b"\n")?;
                }
                7 => {
                    d.add(b"Two many parts in a multi-word entry: ")?;
                    d.decimal(value, 0, 0)?;
                    d.add(b"\n")?;
                }
                8 => d.add(b"Dictionary compiler resource or record bound exceeded\n")?,
                9 => {
                    d.add(b"Compiling: '")?;
                    d.add(first)?;
                    d.add(b"'\n")?;
                    d.increment = false;
                }
                10 => {
                    d.add(b"\t")?;
                    d.decimal(value, 0, 0)?;
                    d.add(b" entries\n")?;
                    d.increment = false;
                }
                _ => d.add(b"Dictionary list I/O failed\n")?,
            }
        }
        Family::Rule | Family::File => {
            if (1..=14).contains(&kind) || kind == 15 || kind == 17 || kind == 19 {
                d.prefix(line)?;
            }
            match kind {
                1 => d.add(b"Expected 2 digits after 'L'\n")?,
                2 => {
                    d.add(b"Letter group L")?;
                    d.decimal(value, 0, 2)?;
                    d.add(b" not defined\n")?;
                }
                3 => d.add(b"$ command not recognized\n")?,
                4 => {
                    d.add(b"Syntax error. Space after (, or negative score for previous rule\n")?
                }
                5 => d.add(b"Syntax error\n")?,
                6 => d.add(b"Encoded phonemes exceed output buffer\n")?,
                7 => d.bad(value, first, None)?,
                8 => {
                    d.add(b"Wrong initial letters '")?;
                    d.add(first)?;
                    d.add(b"' for group '")?;
                    d.add(group)?;
                    d.add(b"'\n")?;
                }
                9 => {
                    d.add(b"bad condition number ?")?;
                    d.decimal(value, 0, 0)?;
                    d.add(b"\n")?;
                }
                10 => d.add(b"Rule exceeds compiler buffer or numeric limit\n")?,
                11 => d.add(b"Incomplete rule escape or letter group\n")?,
                12 if matches!(family, Family::File) => d.add(b"Expected 2 digits after '.L'\n")?,
                13 if matches!(family, Family::File) => {
                    d.add(b"lettergroup out of range (01-94)\n")?
                }
                14 if matches!(family, Family::File) => {
                    d.add(b"lettergroup L")?;
                    d.decimal(value, 0, 2)?;
                    d.add(b" is already defined\n")?;
                }
                15 if matches!(family, Family::File) => {
                    d.add(b"Group name longer than 2 bytes (UTF8)")?
                }
                16 if matches!(family, Family::File) => {
                    d.route = Route::Error;
                    d.add(b"\nExceeded limit of rules (")?;
                    d.decimal(value, 0, 0)?;
                    d.add(b") in group '")?;
                    d.add(group)?;
                    d.add(b"'\n")?;
                }
                17 if matches!(family, Family::File) => {
                    d.add(b"Rule-file compiler resource limit or reentry\n")?
                }
                18 if matches!(family, Family::File) => {
                    d.add(first)?;
                    d.increment = false;
                }
                19 if matches!(family, Family::File) => {
                    d.add(b"Rule-file input/output failed\n")?
                }
                _ => {
                    d.length = 0;
                }
            }
        }
        Family::Driver => {
            d.increment = false;
            match kind {
                1 => d.add(b"Using phonemetable: '")?,
                2 => d.add(b"Compiling: '")?,
                _ => {
                    d.route = Route::Flush;
                    return Some(d);
                }
            }
            d.add(first)?;
            d.add(b"'\n")?;
        }
    }
    Some(d)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signed_width_precision_hex_and_embedded_zero_spelling() {
        let d = format(Family::Rule, -3, 2, -1, b"", b"", b"").unwrap();
        assert_eq!(d.bytes(), b"   -3: Letter group L-01 not defined\n");
        let d = format(Family::List, i32::MIN, 5, 0, b"word", b"phon", b"").unwrap();
        assert_eq!(
            d.bytes(),
            b"-2147483648: Bad phoneme [] (U+0) in: word  phon\n"
        );
    }
    #[test]
    fn distinct_streams_no_newline_unknown_count_and_bounded_storage() {
        let d = format(Family::File, 1, 15, 0, b"", b"", b"").unwrap();
        assert!(!d.bytes().ends_with(b"\n"));
        let d = format(Family::File, 1, 16, 3000, b"", b"", b"ab").unwrap();
        assert_eq!(d.route, Route::Error);
        let d = format(Family::Rule, 1, 99, 0, b"", b"", b"").unwrap();
        assert!(d.bytes().is_empty() && d.increment);
        let d = format(Family::Driver, 1, 99, 0, b"", b"", b"").unwrap();
        assert_eq!(d.route, Route::Flush);
        assert!(!d.increment);
        assert!(format(Family::List, 1, 5, 65, &[b'x'; 16384], b"", b"").is_none());
    }
}
