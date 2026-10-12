//! Native compiler diagnostic byte formatting, without allocation or C replay.
// SPDX-License-Identifier: GPL-3.0-or-later
#[derive(Clone, Copy)]
pub enum Kind {
    Text,
    Table(i32, i32),
    Resource(i32, i32, i32),
    Status(bool),
}
pub struct Diagnostic {
    bytes: [u8; 8192],
    length: usize,
    prefix: usize,
    pub increment: i32,
}
impl Diagnostic {
    fn add(&mut self, bytes: &[u8]) -> Option<()> {
        let end = self.length.checked_add(bytes.len())?;
        self.bytes.get_mut(self.length..end)?.copy_from_slice(bytes);
        self.length = end;
        Some(())
    }
    fn number(&mut self, number: i32) -> Option<()> {
        let mut digits = [0; 11];
        let mut index = digits.len();
        let mut n = number.unsigned_abs();
        loop {
            index -= 1;
            digits[index] = b'0' + (n % 10) as u8;
            n /= 10;
            if n == 0 {
                break;
            }
        }
        if number < 0 {
            index -= 1;
            digits[index] = b'-';
        }
        self.add(&digits[index..])
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes[..self.length]
    }
    pub fn parts(&self) -> [&[u8]; 3] {
        [
            &self.bytes[..self.prefix],
            &self.bytes[self.prefix..self.length - 1],
            &self.bytes[self.length - 1..self.length],
        ]
    }
}
fn text(bytes: &[u8]) -> &[u8] {
    &bytes[..bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len())]
}
pub fn render(
    source: &[u8],
    line: i32,
    kind: Kind,
    name: &[u8],
    message: &[u8],
) -> Option<Diagnostic> {
    if source.len() > 79 || name.len() > 4096 || message.len() > 511 {
        return None;
    }
    let line = line.checked_sub(1)?;
    let mut d = Diagnostic {
        bytes: [0; 8192],
        length: 0,
        prefix: 0,
        increment: 1,
    };
    d.add(text(source))?;
    d.add(b"(")?;
    d.number(line)?;
    d.add(b"): ")?;
    d.prefix = d.length;
    let name = text(name);
    match kind {
        Kind::Text => d.add(name)?,
        Kind::Table(code, value) => match code {
            1 => {
                d.add(b"Bad phoneme name '")?;
                d.add(name)?;
                d.add(b"'")?;
            }
            2 => {
                d.add(b"compile: unknown phoneme table: '")?;
                d.add(name)?;
                d.add(b"'")?;
            }
            3 => {
                d.add(b"Phoneme reference not found: '")?;
                d.add(name)?;
                d.add(b"'")?;
            }
            4 => {
                d.add(b"Phoneme [")?;
                d.add(name)?;
                d.add(b"] not declared, referenced at line ")?;
                d.number(value)?;
                d.increment = 2;
            }
            5 => d.add(b"Too many phonemetables")?,
            6 => d.add(b"Out of memory")?,
            7 => {
                d.add(b"Can't find base phonemetable '")?;
                d.add(name)?;
                d.add(b"'")?;
            }
            8 => {
                d.add(b"Cannot find phoneme '")?;
                d.add(name)?;
                d.add(b"' to import.")?;
            }
            9 => d.add(b"Phoneme import will override set properties.")?,
            _ => return None,
        },
        Kind::Resource(code, a, b) => match code {
            1 => {
                d.add(b"Can't read file: ")?;
                d.add(name)?;
            }
            2 => {
                d.add(b"File not SPEC or RIFF: ")?;
                d.add(name)?;
            }
            3 => {
                d.add(b"Bad vowel file: '")?;
                d.add(name)?;
                d.add(b"'")?;
            }
            4 => {
                d.add(b"Can't resample (")?;
                d.number(a)?;
                d.add(b" to ")?;
                d.number(b)?;
                d.add(b"): ")?;
                d.add(name)?;
            }
            5 => {
                d.add(b"WAV file is not mono: ")?;
                d.add(name)?;
            }
            _ => return None,
        },
        Kind::Status(context) => {
            d.add(text(message))?;
            if context {
                d.add(b": '")?;
                d.add(name)?;
                d.add(b"'.")?;
            } else {
                d.add(b".")?;
            }
        }
    }
    d.add(b"\n")?;
    Some(d)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn formats_signed_boundaries_and_preserves_non_utf8_and_nul_precision() {
        let d = render(
            b"file\xff",
            i32::MIN + 1,
            Kind::Resource(4, i32::MIN, i32::MAX),
            b"name\xfe\0ignored",
            b"",
        )
        .unwrap();
        assert_eq!(
            d.bytes(),
            b"file\xff(-2147483648): Can't resample (-2147483648 to 2147483647): name\xfe\n"
        );
        assert!(render(b"file", i32::MIN, Kind::Text, b"message", b"").is_none());
    }
    #[test]
    fn preserves_double_table_count_and_empty_status_context_presence() {
        assert_eq!(
            render(b"", 0, Kind::Table(4, -7), b"x", b"")
                .unwrap()
                .increment,
            2
        );
        assert_eq!(
            render(b"f", 2, Kind::Status(true), b"", b"status")
                .unwrap()
                .bytes(),
            b"f(1): status: ''.\n"
        );
        assert_eq!(
            render(b"f", 2, Kind::Status(false), b"", b"status")
                .unwrap()
                .bytes(),
            b"f(1): status.\n"
        );
    }
}
