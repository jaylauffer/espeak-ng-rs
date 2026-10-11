//! Owned phoneme bytecode assembly and source-driven instruction controllers.
// SPDX-License-Identifier: GPL-3.0-or-later
pub const WORDS: usize = 2020;
const IF_LIMIT: usize = 1999;
pub const LEVELS: usize = 12;
#[derive(Clone, Copy)]
#[repr(C)]
pub struct Token {
    pub value: i32,
    pub kind: i32,
    pub terminator: i32,
    pub text: [u8; 256],
}
impl Default for Token {
    fn default() -> Self {
        Self {
            value: 0,
            kind: -1,
            terminator: 0,
            text: [0; 256],
        }
    }
}
impl Token {
    pub fn name(&self) -> &[u8] {
        &self.text[..self.text.iter().position(|b| *b == 0).unwrap_or(256)]
    }
}
pub trait Host {
    fn next(&mut self, kind: i32) -> Result<Token, i32>;
    fn unget(&mut self);
    fn error(&mut self, message: &[u8]);
    fn lookup(&mut self, name: &[u8]) -> i32;
    fn load(&mut self, name: &[u8], control: i32) -> i32;
    fn position(&mut self) -> Result<i64, i32>;
    fn write(&mut self, log: bool, bytes: &[u8]) -> Result<(), i32>;
}
#[derive(Clone, Copy, Default)]
struct Branch {
    then: Option<usize>,
    other: Option<usize>,
    returned: bool,
}
#[derive(Default)]
pub struct Program {
    words: Vec<u16>,
    bytes: Vec<u8>,
    branches: [Branch; LEVELS],
    level: usize,
    then_count: i32,
    after_if: bool,
    failed: bool,
    published: bool,
}
impl Program {
    pub fn reset(&mut self) -> Result<(), i32> {
        if self.words.capacity() < WORDS {
            self.words
                .try_reserve_exact(WORDS - self.words.len())
                .map_err(|_| 12)?;
        }
        if self.bytes.capacity() < WORDS * 2 {
            self.bytes
                .try_reserve_exact(WORDS * 2 - self.bytes.len())
                .map_err(|_| 12)?;
        }
        self.words.clear();
        self.bytes.clear();
        self.level = 0;
        self.then_count = 0;
        self.after_if = false;
        self.branches = [Branch::default(); LEVELS];
        self.failed = false;
        self.published = false;
        Ok(())
    }
    pub fn words(&self) -> &[u16] {
        &self.words
    }
    pub fn after_if(&self) -> bool {
        self.after_if
    }
    pub fn level(&self) -> usize {
        self.level
    }
    pub fn then_count(&self) -> i32 {
        self.then_count
    }
    pub fn returned(&self) -> bool {
        self.branches[self.level].returned
    }
    pub fn mark_returned(&mut self) {
        self.branches[self.level].returned = true;
    }
    pub fn decrement(&mut self) {
        if self.then_count > 0 {
            self.then_count -= 1;
        }
    }
    pub fn clear_words(&mut self) {
        self.words.clear();
        // A cleared live branch belongs to the old instruction stream. Refuse
        // later patching even if new words would make its offsets fit again.
        if self.level != 0 {
            self.failed = true;
        }
    }
    pub fn emit(&mut self, words: &[u16], host: &mut impl Host) -> Result<(), i32> {
        if self.failed || self.published {
            return Err(22);
        }
        if words.len() > WORDS - self.words.len() {
            host.error(b"Phoneme program too large");
            self.failed = true;
            return Err(22);
        }
        self.words.extend_from_slice(words);
        Ok(())
    }
    fn brackets(host: &mut impl Host, kind: i32, control: i32) -> Result<Token, i32> {
        if control & 1 == 0 && host.next(6)?.value == 0 {
            host.error(b"Expected '('");
        }
        let token = host.next(kind)?;
        if control & 2 != 0 && token.terminator == 44 {
            return Ok(token);
        }
        if token.terminator != 41 {
            host.error(b"Expected ')'");
        }
        Ok(token)
    }
    pub fn transition(
        &mut self,
        which: i32,
        phoneme_kind: i32,
        host: &mut impl Host,
    ) -> Result<(), i32> {
        let (mut len, mut rms) = match (which, phoneme_kind) {
            (1, 4) => (21i32, 15i32),
            (1, _) => (25, 12),
            (2, _) => (18, 8),
            _ => (0, 0),
        };
        let (mut f1, mut f2, mut lo, mut hi, mut f3, mut amp, mut flags, mut colour) =
            (0i32, 0i32, 0i32, 0i32, 0i32, 0i32, 0i32, 0i32);
        loop {
            let token = host.next(7)?;
            if token.kind != 3 {
                host.unget();
                break;
            }
            match token.value & 15 {
                1 => {
                    len = range(host.next(3)?.value, 2, 0, 63) & 63;
                    flags |= 1;
                }
                2 => {
                    rms = range(host.next(3)?.value, 2, 0, 31) & 31;
                    flags |= 1;
                }
                3 => f1 = host.next(3)?.value,
                4 => {
                    f2 = range(host.next(3)?.value, 50, 0, 63) & 63;
                    lo = range(host.next(4)?.value, 50, -15, 15) & 31;
                    hi = range(host.next(4)?.value, 50, -15, 15) & 31;
                    if lo > hi {
                        std::mem::swap(&mut lo, &mut hi);
                    }
                }
                5 => {
                    f3 = range(host.next(4)?.value, 50, -15, 15) & 31;
                    amp = range(host.next(3)?.value, 8, 0, 15) & 31;
                }
                6..=11 => flags |= 1 << (token.value & 15).wrapping_sub(5),
                12 => colour = host.next(3)?.value,
                13 => {
                    rms = (range(host.next(3)?.value, 1, 0, 31) & 31) | 32;
                    flags |= 1;
                }
                _ => {}
            }
        }
        let a = (len as u32)
            .wrapping_add((rms as u32) << 6)
            .wrapping_add((flags as u32) << 12);
        let b = (f2 as u32)
            .wrapping_add((lo as u32) << 6)
            .wrapping_add((hi as u32) << 11)
            .wrapping_add((f3 as u32) << 16)
            .wrapping_add((amp as u32) << 21)
            .wrapping_add((f1 as u32) << 26)
            .wrapping_add((colour as u32) << 29);
        self.emit(
            &[
                if which == 2 { 0xa200 } else { 0xa100 } + ((a >> 16) & 255) as u16,
                a as u16,
                (b >> 16) as u16,
                b as u16,
            ],
            host,
        )
    }
    pub fn condition(&mut self, elif: bool, host: &mut impl Host) -> Result<i32, i32> {
        self.then_count = 2;
        self.after_if = true;
        let mut word = 0i32;
        loop {
            if self.words.len() >= IF_LIMIT {
                host.error(b"Phoneme program too large");
                return Ok(0);
            }
            let mut token = host.next(8)?;
            if token.value < 0 {
                name_error(host, b"Expected a condition, not '", token.name());
            }
            let negated = token.kind == 0 && token.value == 4;
            if negated {
                token = host.next(8)?;
                if token.value < 0 {
                    name_error(host, b"Expected a condition, not '", token.name());
                }
            }
            let mut second = 0;
            if token.kind == 6 {
                let mut key = token.value;
                if key >= 6 {
                    second = key;
                    key = 6;
                }
                let property = Self::brackets(host, 9, 0)?;
                let data = if property.value >= 0 {
                    property.value.wrapping_add(0x700)
                } else {
                    host.lookup(property.name())
                };
                word = key.wrapping_shl(8).wrapping_add(data);
            } else if token.kind == 7 {
                if token.value == 0x800 {
                    let mut bits = 0i32;
                    let mut brackets = 2;
                    loop {
                        let stress = Self::brackets(host, 3, brackets)?;
                        if stress.value > 7 {
                            host.error(b"Expected list of stress levels");
                        }
                        if !(0..32).contains(&stress.value) {
                            self.failed = true;
                            return Err(22);
                        }
                        bits |= 1i32.wrapping_shl(stress.value as u32);
                        brackets = 3;
                        if stress.terminator != 44 {
                            break;
                        }
                    }
                    word = 0x800 | bits;
                } else {
                    word = token.value;
                }
            } else {
                name_error(host, b"Unexpected keyword '", token.name());
                if token.name() == b"phoneme" || token.name() == b"endphoneme" || token.value == -2
                {
                    return Ok(-1);
                }
            }
            let index = self.words.len();
            self.emit(&[(word | 0x2000) as u16], host)?;
            if second != 0 {
                self.emit(&[second as u16], host)?;
            }
            if negated {
                self.emit(&[3], host)?;
            }
            match host.next(8)?.value {
                1 => {}
                2 => self.words[index] |= 0x1000,
                3 => break,
                -2 => {
                    host.error(b"Expected AND, OR, THEN");
                    return Ok(-1);
                }
                _ => host.error(b"Expected AND, OR, THEN"),
            }
        }
        if !elif {
            if self.level + 1 >= LEVELS {
                host.error(b"IF nesting too deep");
                self.failed = true;
                return Err(22);
            }
            self.level += 1;
            self.branches[self.level].other = None;
        }
        self.branches[self.level].returned = false;
        self.branches[self.level].then = Some(self.words.len());
        self.emit(&[0x6800], host)?;
        Ok(0)
    }
    fn fill_then(&mut self, add: usize, host: &mut impl Host) {
        if let Some(index) = self.branches[self.level].then.take() {
            if index < self.words.len() {
                let offset = self.words.len() - index + add;
                if self.then_count == 1 && self.level == 1 {
                    self.words.remove(index);
                } else {
                    if offset > 255 {
                        host.error(b"IF block is too long");
                    }
                    self.words[index] = (0x6800 + offset) as u16;
                }
            }
        }
        self.then_count = 0;
    }
    pub fn otherwise(&mut self, host: &mut impl Host) -> Result<(), i32> {
        if self.failed {
            return Err(22);
        }
        if self.level == 0 {
            host.error(b"ELSE not expected");
            return Ok(());
        }
        let returned = self.branches[self.level].returned;
        self.fill_then(usize::from(!returned), host);
        if !returned {
            let index = self.words.len();
            let offset = match self.branches[self.level].other {
                Some(previous) => index.checked_sub(previous).ok_or(22)?,
                None => 0,
            };
            self.emit(&[offset as u16], host)?;
            self.branches[self.level].other = Some(index);
        }
        Ok(())
    }
    pub fn elif(&mut self, host: &mut impl Host) -> Result<(), i32> {
        if self.level == 0 {
            host.error(b"ELIF not expected");
            return Ok(());
        }
        self.otherwise(host)?;
        self.condition(true, host)?;
        Ok(())
    }
    pub fn endif(&mut self, host: &mut impl Host) -> Result<(), i32> {
        if self.failed {
            return Err(22);
        }
        if self.level == 0 {
            host.error(b"ENDIF not expected");
            return Ok(());
        }
        self.fill_then(0, host);
        if let Some(mut index) = self.branches[self.level].other {
            loop {
                let chain = usize::from(*self.words.get(index).ok_or(22)?);
                let offset = self.words.len() - index;
                if offset > 255 {
                    host.error(b"IF block is too long");
                }
                self.words[index] = (0x6000 + offset) as u16;
                if chain == 0 {
                    break;
                }
                index = index.checked_sub(chain).ok_or(22)?;
            }
        }
        self.level -= 1;
        Ok(())
    }
    pub fn switch(&mut self, which: i32, host: &mut impl Host) -> Result<(), i32> {
        match which {
            1 => self.emit(&[0x6c06], host),
            2 => self.emit(&[0x6a06], host),
            _ => Ok(()),
        }
    }
    pub fn tone(&mut self, host: &mut impl Host) -> Result<(u8, u8), i32> {
        let p = Self::brackets(host, 3, 2)?;
        let q = Self::brackets(host, 3, 3)?;
        let mut token = q;
        let mut pitch = 0;
        let mut amp = 0;
        if token.terminator == 44 {
            token = Self::brackets(host, 2, 3)?;
            pitch = host.load(token.name(), 0);
        }
        if token.terminator == 44 {
            token = Self::brackets(host, 2, 1)?;
            amp = host.load(token.name(), 0);
        }
        if pitch != 0 {
            self.emit(&[0x9200 + ((pitch >> 16) & 15) as u16, pitch as u16], host)?;
        }
        if amp != 0 {
            self.emit(&[0x9300 + ((amp >> 16) & 15) as u16, amp as u16], host)?;
        }
        Ok((p.value.min(q.value) as u8, p.value.max(q.value) as u8))
    }
    pub fn sound(&mut self, keyword: i32, vowel: i32, host: &mut impl Host) -> Result<(), i32> {
        let first = Self::brackets(host, 2, 2)?;
        let mut value = 0;
        if first.terminator == 44 {
            if keyword == 23 || keyword == 24 {
                value = Self::brackets(host, 4, 1)?.value;
                if value > 127 {
                    value = 127;
                    host.error(b"Parameter > 127");
                }
                if value < -128 {
                    value = -128;
                    host.error(b"Parameter < -128");
                }
            } else {
                value = Self::brackets(host, 3, 1)?.value;
                if value > 255 {
                    value = 255;
                    host.error(b"Parameter > 255");
                }
            }
        }
        let address = host.load(first.name(), vowel) / 4;
        let op = *[0xb000u16, 0xc000, 0xd000, 0xe000, 0xf000]
            .get(keyword.checked_sub(21).ok_or(22)? as usize)
            .ok_or(22)?;
        self.emit(
            &[
                op + ((value & 255) << 4) as u16 + ((address >> 16) & 15) as u16,
                address as u16,
            ],
            host,
        )
    }
    pub fn finish(&mut self, host: &mut impl Host) -> Result<(), i32> {
        if self.level > 0 {
            host.error(b"Missing ENDIF");
        }
        if !self.words.is_empty() && !self.branches[0].returned {
            self.emit(&[1], host)?;
        }
        Ok(())
    }
    pub fn publish(&mut self, log: bool, host: &mut impl Host) -> Result<Option<i32>, i32> {
        if self.failed || self.published {
            return Err(22);
        }
        self.published = true;
        if self.words.is_empty() {
            return Ok(None);
        }
        let position = host.position()?;
        if position < 0 {
            return Err(22);
        }
        let address = (position / 2) as i32;
        if log {
            let mut bytes = [0; 8];
            bytes[..4].copy_from_slice(&u32::from(address as u16).to_le_bytes());
            bytes[4..].copy_from_slice(&(self.words.len() as u32).to_le_bytes());
            host.write(true, &bytes)?;
        }
        self.bytes.clear();
        for word in &self.words {
            self.bytes.extend_from_slice(&word.to_le_bytes());
        }
        host.write(false, &self.bytes)?;
        Ok(Some(address))
    }
}
fn range(value: i32, divide: i32, min: i32, max: i32) -> i32 {
    let adjusted = i64::from(value)
        + if value < 0 {
            -i64::from(divide / 2)
        } else {
            i64::from(divide / 2)
        };
    (adjusted / i64::from(divide)).clamp(i64::from(min), i64::from(max)) as i32 - min
}
fn name_error(host: &mut impl Host, prefix: &[u8], name: &[u8]) {
    let mut message = [0; 320];
    message[..prefix.len()].copy_from_slice(prefix);
    message[prefix.len()..prefix.len() + name.len()].copy_from_slice(name);
    message[prefix.len() + name.len()] = b'\'';
    host.error(&message[..prefix.len() + name.len() + 1]);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Effects {
        tokens: std::collections::VecDeque<Token>,
        errors: Vec<Vec<u8>>,
        writes: Vec<(bool, Vec<u8>)>,
        failure: i32,
        position: i64,
    }
    impl Host for Effects {
        fn next(&mut self, _: i32) -> Result<Token, i32> {
            Ok(self.tokens.pop_front().unwrap_or(Token {
                value: -2,
                ..Token::default()
            }))
        }
        fn unget(&mut self) {}
        fn error(&mut self, bytes: &[u8]) {
            self.errors.push(bytes.to_vec());
        }
        fn lookup(&mut self, _: &[u8]) -> i32 {
            31
        }
        fn load(&mut self, _: &[u8], _: i32) -> i32 {
            0x12340
        }
        fn position(&mut self) -> Result<i64, i32> {
            Ok(self.position)
        }
        fn write(&mut self, log: bool, bytes: &[u8]) -> Result<(), i32> {
            self.writes.push((log, bytes.to_vec()));
            if self.failure == 0 {
                Ok(())
            } else {
                Err(self.failure)
            }
        }
    }
    fn condition(h: &mut Effects) {
        h.tokens.extend([
            Token {
                value: 0xf01,
                kind: 7,
                ..Token::default()
            },
            Token {
                value: 3,
                kind: 0,
                ..Token::default()
            },
        ]);
    }
    #[test]
    fn nested_conditions_patch_offsets_and_remove_single_statement_jump() {
        let mut p = Program::default();
        p.reset().unwrap();
        let mut h = Effects::default();
        condition(&mut h);
        p.condition(false, &mut h).unwrap();
        p.emit(&[0x1234], &mut h).unwrap();
        p.decrement();
        p.endif(&mut h).unwrap();
        assert_eq!(p.words(), [0x2f01, 0x1234]);
        p.reset().unwrap();
        condition(&mut h);
        p.condition(false, &mut h).unwrap();
        condition(&mut h);
        p.condition(false, &mut h).unwrap();
        p.emit(&[2], &mut h).unwrap();
        p.decrement();
        p.endif(&mut h).unwrap();
        p.otherwise(&mut h).unwrap();
        p.emit(&[3], &mut h).unwrap();
        p.endif(&mut h).unwrap();
        assert_eq!(p.words(), [0x2f01, 0x6805, 0x2f01, 0x6802, 2, 0x6002, 3]);
    }
    #[test]
    fn caps_and_eof_refuse_without_overrun_or_condition_spin() {
        let mut p = Program::default();
        p.reset().unwrap();
        let mut h = Effects::default();
        for _ in 1..LEVELS {
            condition(&mut h);
            p.condition(false, &mut h).unwrap();
        }
        condition(&mut h);
        assert_eq!(p.condition(false, &mut h), Err(22));
        assert_eq!(p.level(), LEVELS - 1);
        p.reset().unwrap();
        assert_eq!(p.condition(false, &mut h), Ok(-1));
        assert!(p.words().is_empty());
        p.reset().unwrap();
        h.tokens.extend([
            Token {
                value: 0x800,
                kind: 7,
                ..Token::default()
            },
            Token {
                value: 1,
                ..Token::default()
            },
            Token {
                value: 32,
                terminator: 41,
                ..Token::default()
            },
        ]);
        assert_eq!(p.condition(false, &mut h), Err(22));
        assert!(p.words().is_empty());
        p.reset().unwrap();
        p.emit(&[0; IF_LIMIT], &mut h).unwrap();
        assert_eq!(p.condition(false, &mut h), Ok(0));
        p.finish(&mut h).unwrap();
        assert_eq!(p.words().len(), IF_LIMIT + 1);
        assert_eq!(p.publish(false, &mut h), Ok(Some(0)));
        p.reset().unwrap();
        p.emit(&[0; WORDS], &mut h).unwrap();
        assert_eq!(p.emit(&[1], &mut h), Err(22));
        assert_eq!(p.words().len(), WORDS);
    }
    #[test]
    fn publication_is_little_endian_retained_and_never_replayed_after_failure() {
        let mut p = Program::default();
        p.reset().unwrap();
        let mut h = Effects {
            position: 0x2468,
            ..Effects::default()
        };
        let words = p.words.as_ptr();
        let bytes = p.bytes.as_ptr();
        for _ in 0..64 {
            p.reset().unwrap();
            p.emit(&[0x1234, 0xff01], &mut h).unwrap();
            assert_eq!(p.publish(true, &mut h), Ok(Some(0x1234)));
            assert_eq!(p.words.as_ptr(), words);
            assert_eq!(p.bytes.as_ptr(), bytes);
        }
        assert_eq!(h.writes[0], (true, vec![0x34, 0x12, 0, 0, 2, 0, 0, 0]));
        assert_eq!(h.writes[1], (false, vec![0x34, 0x12, 1, 0xff]));
        p.reset().unwrap();
        p.emit(&[2], &mut h).unwrap();
        h.failure = 28;
        assert_eq!(p.publish(true, &mut h), Err(28));
        let n = h.writes.len();
        assert_eq!(p.publish(true, &mut h), Err(22));
        assert_eq!(h.writes.len(), n);
    }
    #[test]
    fn transition_defaults_wide_rounding_and_byte_diagnostics_are_bounded() {
        let mut p = Program::default();
        p.reset().unwrap();
        let mut h = Effects::default();
        p.transition(1, 4, &mut h).unwrap();
        assert_eq!(p.words(), [0xa100, 21 + 15 * 64, 0, 0]);
        assert_eq!(range(i32::MAX, 2, 0, 63), 63);
        assert_eq!(range(i32::MIN, 50, -15, 15), 0);
        h.tokens.push_back(Token {
            value: -1,
            text: {
                let mut a = [0; 256];
                a[0] = 255;
                a
            },
            ..Token::default()
        });
        p.condition(false, &mut h).unwrap();
        assert_eq!(h.errors[0], b"Expected a condition, not '\xff'");
    }
    #[test]
    fn cleared_program_refuses_stale_branch_offsets() {
        let mut p = Program::default();
        p.reset().unwrap();
        let mut h = Effects::default();
        condition(&mut h);
        p.condition(false, &mut h).unwrap();
        p.emit(&[2, 3], &mut h).unwrap();
        p.otherwise(&mut h).unwrap();
        p.clear_words();
        assert_eq!(p.emit(&[0; 32], &mut h), Err(22));
        assert_eq!(p.otherwise(&mut h), Err(22));
        assert_eq!(p.endif(&mut h), Err(22));
    }
}
