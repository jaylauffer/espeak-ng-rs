//! Complete NextItem token/property policy over the owned source scanner.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{compiler_items_tables::TABLES, compiler_program::Token, compiler_source};
pub fn keyword(kind: i32, name: &[u8]) -> (i32, i32) {
    let Some(table) = kind.checked_sub(7).and_then(|n| TABLES.get(n as usize)) else {
        return (-1, -1);
    };
    table
        .iter()
        .find(|entry| entry.0 == name)
        .map_or((-1, -1), |entry| (entry.1, entry.2))
}
pub trait Host {
    fn error(&mut self, line: i32, message: &[u8]);
    fn lookup(&mut self, line: i32, name: &[u8]) -> i32;
}
pub fn complete(
    source: Option<compiler_source::Token>,
    kind: i32,
    mut previous: Token,
    previous_line: i32,
    host: &mut impl Host,
) -> Result<(Token, i32), i32> {
    previous.kind = -1;
    let Some(source) = source else {
        previous.value = -2;
        return Ok((previous, previous_line));
    };
    if source.length >= 0 {
        let n = source.length as usize;
        if n >= 256 || source.bytes[n] != 0 {
            return Err(22);
        }
        previous.text[..=n].copy_from_slice(&source.bytes[..=n]);
        previous.terminator = source.terminator;
    }
    previous.value = if source.status == -3 {
        host.error(source.line, b"Item exceeds 255 bytes");
        -1
    } else if source.status != 0 {
        source.status
    } else if kind == 2 {
        0
    } else if kind == 3 || kind == 4 {
        match source.diagnostic {
            1 => host.error(source.line, b"Expected an unsigned number"),
            2 => host.error(source.line, b"Expected a number"),
            3 => host.error(source.line, b"Number exceeds signed integer range"),
            _ => {}
        }
        source.value
    } else if (7..=9).contains(&kind) {
        let (category, value) = keyword(kind, previous.name());
        previous.kind = category;
        value
    } else if kind == 5 {
        host.lookup(source.line, previous.name())
    } else {
        -1
    };
    Ok((previous, source.line))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Effects {
        diagnostics: Vec<(i32, Vec<u8>)>,
        lookups: usize,
    }
    impl Host for Effects {
        fn error(&mut self, line: i32, bytes: &[u8]) {
            self.diagnostics.push((line, bytes.to_vec()));
        }
        fn lookup(&mut self, _: i32, _: &[u8]) -> i32 {
            self.lookups += 1;
            41
        }
    }
    #[test]
    fn duplicate_keyword_retains_first_entry_and_case() {
        assert_eq!(keyword(7, b"ChangeIfStressed"), (5, 19));
        assert_eq!(keyword(7, b"changeifstressed"), (-1, -1));
        assert_eq!(keyword(8, b"nextVowel"), (6, 7));
        assert_eq!(keyword(9, b"isWordEnd"), (0, 0x93));
        assert_eq!(keyword(10, b"tune"), (-1, -1));
    }
    #[test]
    fn eof_and_brackets_preserve_initialized_token_tails() {
        let old = Token {
            text: [0x55; 256],
            value: 1,
            kind: 7,
            terminator: 44,
        };
        let mut h = Effects::default();
        let (value, line) = complete(None, 7, old, 13, &mut h).unwrap();
        assert_eq!(
            (value.value, value.kind, value.terminator, line),
            (-2, -1, 44, 13)
        );
        assert_eq!(value.text, old.text);
        let source = compiler_source::Token {
            status: 1,
            line: 15,
            length: -1,
            terminator: 0,
            value: 0,
            diagnostic: 0,
            bytes: [0; 256],
        };
        let (value, line) = complete(Some(source), 6, old, 13, &mut h).unwrap();
        assert_eq!((value.value, value.terminator, line), (1, 44, 15));
        assert_eq!(value.text, old.text);
    }
    #[test]
    fn numeric_error_uses_scanner_line_and_does_not_lookup() {
        let source = compiler_source::Token {
            status: 0,
            line: 8,
            length: 1,
            terminator: 41,
            value: 0,
            diagnostic: 1,
            bytes: [0; 256],
        };
        let mut h = Effects::default();
        let (value, _) = complete(Some(source), 3, Token::default(), 1, &mut h).unwrap();
        assert_eq!(value.value, 0);
        assert_eq!(
            h.diagnostics,
            [(8, b"Expected an unsigned number".to_vec())]
        );
        assert_eq!(h.lookups, 0);
    }
}
