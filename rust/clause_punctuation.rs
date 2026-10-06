//! Punctuation announcement with separate dictionary and input effects.
// Copyright (C) 2005-2015 Jonathan Duddington, 2015-2017 Reece H. Dunn.
// Rust adaptation (C) 2026. SPDX-License-Identifier: GPL-3.0-or-later
use crate::clause_input;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Bounds,
    Capacity,
    Count,
    Backend,
}
pub trait Host {
    fn soundicon(&mut self, code: i32) -> i32;
    fn name(&mut self, code: i32, period: bool) -> Result<Option<[u8; 74]>, Error>;
    fn eof(&self) -> bool;
    fn read(&mut self) -> i32;
    fn unread(&mut self, code: i32);
    fn unread_second(&mut self, code: i32);
    fn flags(&self) -> i32;
    fn speed(&self) -> i32;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request {
    pub code: i32,
    pub next: i32,
    pub offset: usize,
    pub end_clause: bool,
    pub capacity: usize,
}
pub struct Plan {
    pub bytes: [u8; 200],
    pub length: usize,
    pub next: i32,
    pub terminator: i32,
    pub write: bool,
}
impl Plan {
    fn append(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let end = self
            .length
            .checked_add(bytes.len())
            .filter(|end| *end < self.bytes.len())
            .ok_or(Error::Capacity)?;
        self.bytes[self.length..end].copy_from_slice(bytes);
        self.length = end;
        Ok(())
    }
    fn number(&mut self, value: i32) -> Result<(), Error> {
        let mut digits = [0; 11];
        let mut position = digits.len();
        let mut remaining = value.unsigned_abs();
        loop {
            position -= 1;
            digits[position] = b'0' + (remaining % 10) as u8;
            remaining /= 10;
            if remaining == 0 {
                break;
            }
        }
        if value < 0 {
            position -= 1;
            digits[position] = b'-';
        }
        self.append(&digits[position..])
    }
}
fn name(bytes: &[u8; 74], period: bool) -> Result<&[u8], Error> {
    let length = bytes
        .iter()
        .position(|byte| *byte == 0)
        .ok_or(Error::Bounds)?;
    if period && length >= 30 {
        return Err(Error::Bounds);
    }
    Ok(&bytes[..length])
}
/// Backend/source effects run on the caller owner/worker without any engine
/// storage borrow. They can precede a rejected output plan; no rollback is
/// promised. Output admission includes NUL and the legacy200-byte scratch limit.
pub fn announce(request: Request, host: &mut impl Host) -> Result<Plan, Error> {
    if request.offset > request.capacity || request.capacity > i32::MAX as usize {
        return Err(Error::Bounds);
    }
    let mut result = Plan {
        bytes: [0; 200],
        length: 0,
        next: request.next,
        terminator: -1,
        write: false,
    };
    let icon = host.soundicon(request.code);
    if icon >= 0 {
        result.append(b"\x01")?;
        result.number(icon)?;
        result.append(b"I ")?;
        host.unread(result.next);
    } else {
        let period = request.code == 46 && request.end_clause && result.next != 46;
        let special = if period {
            host.name(request.code, true)?
        } else {
            None
        };
        let (selected, is_period) = if let Some(selected) = special {
            (Some(selected), true)
        } else {
            (host.name(request.code, false)?, false)
        };
        let Some(selected) = selected else {
            return Ok(result);
        };
        let selected = name(&selected, is_period)?;
        if request.offset == 0 || !request.end_clause || host.flags() & 2 != 0 {
            let mut count = 1i32;
            while !host.eof() && result.next == request.code && request.code != 60 {
                count = count.checked_add(1).ok_or(Error::Count)?;
                result.next = host.read();
            }
            if request.end_clause {
                host.unread(result.next);
            }
            if count == 1 {
                result.append(b" ")?;
                result.append(selected)?;
            } else if count < 4 {
                let accelerate = host.speed() < 300;
                if accelerate {
                    result.append(b"\x01+10S")?;
                }
                for _ in 0..count {
                    result.append(b" ")?;
                    result.append(selected)?;
                }
                if accelerate {
                    result.append(b" \x01-10S")?;
                }
            } else {
                result.append(b" ")?;
                result.append(selected)?;
                result.append(b" ")?;
                result.number(count)?;
                result.append(b" ")?;
                result.append(selected)?;
            }
        } else {
            host.unread_second(request.code);
            result.append(b" ")?;
            host.unread(result.next);
        }
    }
    if request
        .offset
        .checked_add(result.length + 1)
        .filter(|end| *end <= request.capacity)
        .is_none()
    {
        return Err(Error::Capacity);
    }
    result.write = true;
    if !request.end_clause {
        return Ok(result);
    }
    if request.code == 45 {
        result.terminator = 0x4000;
        return Ok(result);
    }
    let attributes = clause_input::clause_type(request.code as u32);
    let short = if attributes & 0x7000 == 0x1000 {
        0x41004
    } else {
        0x40004
    };
    result.terminator = if request.offset > 0 && host.flags() & 2 == 0 {
        if attributes & !0x8000 == 0x4101e {
            0x40004
        } else {
            short
        }
    } else if attributes & 0x80000 != 0 {
        attributes
    } else {
        short
    };
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Backend {
        input: Vec<i32>,
        position: usize,
        pending: i32,
        second: i32,
        flags: i32,
        speed: i32,
        icon: i32,
        long: bool,
        trace: Vec<&'static str>,
    }
    impl Host for Backend {
        fn soundicon(&mut self, _: i32) -> i32 {
            self.trace.push("icon");
            self.icon
        }
        fn name(&mut self, _: i32, period: bool) -> Result<Option<[u8; 74]>, Error> {
            self.trace.push(if period { "period" } else { "character" });
            let mut name = [0; 74];
            if self.long {
                name[..73].fill(b'x');
            } else {
                name[..3].copy_from_slice(b"dot");
            }
            Ok(Some(name))
        }
        fn eof(&self) -> bool {
            self.position >= self.input.len()
        }
        fn read(&mut self) -> i32 {
            let code = self.input[self.position];
            self.position += 1;
            code
        }
        fn unread(&mut self, code: i32) {
            self.pending = code;
        }
        fn unread_second(&mut self, code: i32) {
            self.second = code;
        }
        fn flags(&self) -> i32 {
            self.flags
        }
        fn speed(&self) -> i32 {
            self.speed
        }
    }
    fn backend() -> Backend {
        Backend {
            input: vec![46, 88],
            position: 0,
            pending: 0,
            second: 0,
            flags: 2,
            speed: 175,
            icon: -1,
            long: false,
            trace: Vec::new(),
        }
    }
    #[test]
    fn repeated_names_preserve_speed_commands_stream_effects_and_pause() {
        let mut host = backend();
        let plan = announce(
            Request {
                code: 46,
                next: 46,
                offset: 0,
                end_clause: true,
                capacity: 100,
            },
            &mut host,
        )
        .unwrap();
        assert_eq!(
            &plan.bytes[..=plan.length],
            b"\x01+10S dot dot dot \x01-10S\0"
        );
        assert_eq!(host.pending, 88);
        assert_eq!(plan.next, 88);
        assert_eq!(plan.terminator, 0x80028);
        assert_eq!(host.trace, ["icon", "character"]);
        let mut host = backend();
        host.flags = 0;
        let plan = announce(
            Request {
                code: 59,
                next: 88,
                offset: 20,
                end_clause: true,
                capacity: 100,
            },
            &mut host,
        )
        .unwrap();
        assert_eq!(&plan.bytes[..=plan.length], b" \0");
        assert_eq!(plan.terminator, 0x40004);
        assert_eq!(host.second, 59);
        assert_eq!(host.pending, 88);
    }
    #[test]
    fn capacity_rejection_retains_earlier_stream_effects_and_bounds_scratch() {
        let mut host = backend();
        host.icon = 7;
        assert!(matches!(
            announce(
                Request {
                    code: 45,
                    next: 88,
                    offset: 5,
                    end_clause: true,
                    capacity: 5
                },
                &mut host
            ),
            Err(Error::Capacity)
        ));
        assert_eq!(host.pending, 88);
        let mut host = backend();
        host.long = true;
        assert!(matches!(
            announce(
                Request {
                    code: 46,
                    next: 46,
                    offset: 0,
                    end_clause: true,
                    capacity: 512
                },
                &mut host
            ),
            Err(Error::Capacity)
        ));
        assert_eq!(host.position, 2);
        assert_eq!(host.pending, 88);
    }
}
