//! Complete phoneme source-file/include control with a fixed owned stack.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::compiler_program::Token;
pub const DEPTH: usize = 12;
pub const PATH: usize = 4096;
#[derive(Clone, Copy)]
#[repr(C)]
pub struct Frame {
    pub file: i32,
    pub line: i32,
    pub name: [u8; 80],
}
impl Default for Frame {
    fn default() -> Self {
        Self {
            file: 0,
            line: 1,
            name: [0; 80],
        }
    }
}
pub trait Host {
    fn reset(&mut self) -> Result<(), i32>;
    fn eof(&mut self) -> bool;
    fn next(&mut self, kind: i32) -> Result<Token, i32>;
    fn frame(&mut self) -> Result<Frame, i32>;
    fn project(&mut self, frame: Frame) -> Result<(), i32>;
    fn open(&mut self, path: &[u8], depth: usize) -> Result<i32, i32>;
    fn close(&mut self) -> Result<(), i32>;
    fn tables(&mut self) -> i32;
    fn end_table(&mut self) -> Result<(), i32>;
    fn start_table(&mut self, name: &[u8]) -> Result<(), i32>;
    fn compile(&mut self, phoneme: bool) -> Result<(), i32>;
    fn terminate(&mut self) -> Result<(), i32>;
    fn error(&mut self, bytes: &[u8]);
}
fn name(bytes: &[u8]) -> &[u8] {
    &bytes[..bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len())]
}
pub fn run(root: &[u8], width: usize, h: &mut impl Host) -> Result<bool, i32> {
    if !(2..=PATH).contains(&width) || root.len() >= width || root.contains(&0) {
        return Err(22);
    }
    let mut stack = [Frame::default(); DEPTH];
    let mut depth = 0;
    let mut path = [0; PATH];
    h.reset()?;
    loop {
        if h.eof() {
            if depth == 0 {
                break;
            }
            h.close()?;
            depth -= 1;
            let saved = stack[depth];
            let mut current = h.frame()?;
            let n = name(&saved.name).len();
            if n == saved.name.len() {
                return Err(22);
            }
            // strcpy on restore leaves the previous initialized tail intact.
            current.name[..=n].copy_from_slice(&saved.name[..=n]);
            current.file = saved.file;
            current.line = saved.line;
            h.project(current)?;
        }
        let item = h.next(7)?;
        match item.value {
            2 => {} // UTF-8 BOM
            7 => {
                let item = h.next(2)?;
                let child = item.name();
                if child.len() == 256 {
                    return Err(22);
                }
                let mut n = 0;
                for &byte in root.iter().chain(std::iter::once(&b'/')).chain(child) {
                    if n == width - 1 {
                        break;
                    }
                    path[n] = byte;
                    n += 1;
                }
                let file = if depth < DEPTH {
                    h.open(&path[..n], depth + 1)?
                } else {
                    0
                };
                if file != 0 {
                    stack[depth] = h.frame()?;
                    depth += 1;
                    let mut frame = Frame {
                        file,
                        ..Frame::default()
                    };
                    let n = child.len().min(79);
                    frame.name[..n].copy_from_slice(&child[..n]);
                    h.project(frame)?;
                } else {
                    let mut message = [0; 280];
                    let prefix = b"Missing file: ";
                    message[..prefix.len()].copy_from_slice(prefix);
                    message[prefix.len()..prefix.len() + child.len()].copy_from_slice(child);
                    h.error(&message[..prefix.len() + child.len()]);
                }
            }
            6 => {
                h.end_table()?;
                let item = h.next(2)?;
                h.start_table(item.name())?;
            }
            1 => {
                if h.tables() == 0 {
                    h.error(b"phonemetable is missing");
                    return Ok(false);
                }
                h.compile(true)?;
            }
            3 => h.compile(false)?,
            _ => {
                if !h.eof() {
                    h.error(b"Keyword 'phoneme' expected");
                }
            }
        }
    }
    h.terminate()?;
    Ok(true)
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Effects {
        tokens: Vec<Token>,
        at: usize,
        frame: Frame,
        events: Vec<&'static str>,
        opened: Vec<u8>,
    }
    impl Host for Effects {
        fn reset(&mut self) -> Result<(), i32> {
            self.events.push("reset");
            Ok(())
        }
        fn eof(&mut self) -> bool {
            self.at == self.tokens.len()
        }
        fn next(&mut self, _: i32) -> Result<Token, i32> {
            let t = self.tokens[self.at];
            self.at += 1;
            Ok(t)
        }
        fn frame(&mut self) -> Result<Frame, i32> {
            Ok(self.frame)
        }
        fn project(&mut self, f: Frame) -> Result<(), i32> {
            self.frame = f;
            Ok(())
        }
        fn open(&mut self, path: &[u8], _: usize) -> Result<i32, i32> {
            self.opened = path.to_vec();
            Ok(0)
        }
        fn close(&mut self) -> Result<(), i32> {
            self.events.push("close");
            Ok(())
        }
        fn tables(&mut self) -> i32 {
            0
        }
        fn end_table(&mut self) -> Result<(), i32> {
            self.events.push("end");
            Ok(())
        }
        fn start_table(&mut self, _: &[u8]) -> Result<(), i32> {
            self.events.push("start");
            Ok(())
        }
        fn compile(&mut self, _: bool) -> Result<(), i32> {
            self.events.push("compile");
            Ok(())
        }
        fn terminate(&mut self) -> Result<(), i32> {
            self.events.push("terminate");
            Ok(())
        }
        fn error(&mut self, _: &[u8]) {
            self.events.push("error");
        }
    }
    fn effects(tokens: Vec<Token>) -> Effects {
        Effects {
            tokens,
            at: 0,
            frame: Frame::default(),
            events: vec![],
            opened: vec![],
        }
    }
    #[test]
    fn include_paths_keep_snprintf_truncation_and_missing_file_continues() {
        let mut child = Token::default();
        child.text[..5].copy_from_slice(b"child");
        let mut h = effects(vec![
            Token {
                value: 7,
                ..Token::default()
            },
            child,
        ]);
        assert!(run(b"root", 8, &mut h).unwrap());
        assert_eq!(h.opened, b"root/ch");
        assert_eq!(h.events, ["reset", "error", "terminate"]);
    }
    #[test]
    fn missing_table_stops_before_compile_or_terminator() {
        let mut h = effects(vec![Token {
            value: 1,
            ..Token::default()
        }]);
        assert!(!run(b"root", 20, &mut h).unwrap());
        assert_eq!(h.events, ["reset", "error"]);
    }
    #[test]
    fn invalid_path_admission_has_no_effects() {
        let mut h = effects(vec![]);
        for (root, width) in [(b"root".as_slice(), 4), (b"a\0b", 10), (b"root", PATH + 1)] {
            assert_eq!(run(root, width, &mut h), Err(22));
        }
        assert!(h.events.is_empty());
    }
}
