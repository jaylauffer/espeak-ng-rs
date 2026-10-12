//! Complete public phoneme compiler path/lifecycle policy.
// Copyright (C) 2005-2014 Jonathan Duddington, 2013-2017 Reece H. Dunn.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{compiler_manifest::Workspace, voice_reader::TextMode};
use std::io::{self, Write};
pub const COMPILE_ERROR: i32 = 0x100001ff;
#[cfg(feature = "c-abi")]
pub const NAME_TOO_LONG: i32 = libc::ENAMETOOLONG;
#[cfg(not(feature = "c-abi"))]
pub const NAME_TOO_LONG: i32 = if cfg!(windows) {
    38
} else if cfg!(any(
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
)) {
    63
} else {
    36
};
/// Each operation is one synchronous primitive, never a nested controller.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Allocate,
    Tables,
    Program,
    Rate,
    Reload,
    Voice,
    WaveInit,
    Initialize,
    SourceOpen,
    OutputOpen,
    BaseTable,
    Compile,
    EndTable,
    WriteTables,
    SourceClose,
    OutputClose,
    OutputError,
    CloseLog,
    WaveFini,
    Cleanup,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Counts {
    pub references: i32,
    pub reused: i32,
    pub errors: i32,
}
pub trait Host {
    fn effect(
        &mut self,
        op: Op,
        value: i32,
        source: &[u8],
        destination: &[u8],
        error_path: &mut [u8; 4096],
    ) -> i32;
    fn counts(&mut self) -> Counts;
    fn log(&mut self, bytes: &[u8]);
    fn file_error(&mut self, status: i32, path: &[u8]) -> i32;
}
pub struct Paths {
    pub source: [u8; 4096],
    pub destination: [u8; 4096],
    pub input: [u8; 4096],
    pub manifest: [u8; 4096],
    pub width: usize,
}
pub fn string(bytes: &[u8]) -> &[u8] {
    &bytes[..bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len())]
}
fn path(prefix: &[u8], suffix: &[u8], width: usize) -> [u8; 4096] {
    let mut out = [0; 4096];
    let n = prefix.len().min(width - 1);
    out[..n].copy_from_slice(&prefix[..n]);
    let m = suffix.len().min(width - 1 - n);
    out[n..n + m].copy_from_slice(&suffix[..m]);
    out
}
impl Paths {
    pub fn new(
        home: &[u8],
        source: Option<&[u8]>,
        destination: Option<&[u8]>,
        width: usize,
        windows: bool,
    ) -> Result<Self, i32> {
        if !(2..=4096).contains(&width) {
            return Err(22);
        }
        if home.contains(&0)
            || source.is_some_and(|s| s.contains(&0))
            || destination.is_some_and(|s| s.contains(&0))
        {
            return Err(22);
        }
        if source.is_some_and(|s| s.len() >= width) || destination.unwrap_or(home).len() >= width {
            return Err(NAME_TOO_LONG);
        }
        let source = match source {
            Some(s) => path(s, b"", width),
            None => path(home, b"/../phsource", width),
        };
        Ok(Self {
            input: path(string(&source), b"/phonemes", width),
            manifest: path(
                home,
                if windows {
                    b"\\phondata-manifest"
                } else {
                    b"/phondata-manifest"
                },
                width,
            ),
            source,
            destination: path(destination.unwrap_or(home), b"", width),
            width,
        })
    }
}
/// A bounded formatting buffer; paths remain arbitrary native bytes.
struct Log {
    bytes: [u8; 4608],
    len: usize,
}
impl Log {
    fn new() -> Self {
        Self {
            bytes: [0; 4608],
            len: 0,
        }
    }
    fn add(&mut self, bytes: &[u8]) {
        let n = bytes.len().min(self.bytes.len() - self.len);
        self.bytes[self.len..self.len + n].copy_from_slice(&bytes[..n]);
        self.len += n;
    }
    fn send(&self, host: &mut impl Host) {
        host.log(&self.bytes[..self.len]);
    }
}
impl Write for Log {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.add(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
pub fn errno(error: &io::Error) -> i32 {
    #[cfg(unix)]
    if let Some(code) = error.raw_os_error() {
        return code;
    }
    match error.kind() {
        io::ErrorKind::NotFound => 2,
        io::ErrorKind::PermissionDenied => 13,
        io::ErrorKind::IsADirectory => 21,
        io::ErrorKind::NotADirectory => 20,
        io::ErrorKind::OutOfMemory => 12,
        io::ErrorKind::WouldBlock => 11,
        io::ErrorKind::InvalidInput | io::ErrorKind::InvalidData => 22,
        io::ErrorKind::StorageFull => 28,
        _ => 5,
    }
}
/// Cleanup executes exactly once after admitted allocation, including refusal.
pub fn run(
    host: &mut impl Host,
    paths: &Paths,
    manifest: &mut Workspace,
    alpha: &[u8; 256],
    spaces: &[u8; 256],
    mode: TextMode,
) -> i32 {
    let mut error_path = [0; 4096];
    let status = host.effect(
        Op::Allocate,
        0,
        &paths.source,
        &paths.destination,
        &mut error_path,
    );
    if status != 0 {
        return status;
    }
    let result = live(host, paths, manifest, alpha, spaces, mode, &mut error_path);
    host.effect(
        Op::Cleanup,
        0,
        &paths.source,
        &paths.destination,
        &mut error_path,
    );
    result
}
fn live(
    host: &mut impl Host,
    paths: &Paths,
    manifest: &mut Workspace,
    alpha: &[u8; 256],
    spaces: &[u8; 256],
    mode: TextMode,
    error_path: &mut [u8; 4096],
) -> i32 {
    macro_rules! effect {
        ($op:ident) => {
            host.effect(Op::$op, 0, &paths.source, &paths.destination, error_path)
        };
    }
    for op in [
        Op::Tables,
        Op::Program,
        Op::Rate,
        Op::Reload,
        Op::Voice,
        Op::WaveInit,
        Op::Initialize,
    ] {
        let status = host.effect(op, 0, &paths.source, &paths.destination, error_path);
        if status != 0 {
            return status;
        }
    }
    let mut log = Log::new();
    log.add(b"Compiling phoneme data: ");
    log.add(string(&paths.input));
    log.add(b"\n");
    log.send(host);
    let status = host.effect(
        Op::SourceOpen,
        0,
        &paths.input,
        &paths.destination,
        error_path,
    );
    if status != 0 {
        return host.file_error(status, &paths.input);
    }
    let status = effect!(OutputOpen);
    if status != 0 {
        effect!(SourceClose);
        return host.file_error(status, error_path);
    }
    effect!(BaseTable);
    effect!(Compile);
    effect!(EndTable);
    effect!(WriteTables);
    let c = host.counts();
    let mut log = Log::new();
    let _ = write!(log, "\nRefs {},  Reused {}\n", c.references, c.reused);
    log.send(host);
    effect!(SourceClose);
    let status = effect!(OutputClose);
    if status != 0 {
        host.effect(
            Op::OutputError,
            status,
            &paths.source,
            &paths.destination,
            error_path,
        );
        effect!(CloseLog);
        return status;
    }
    effect!(Reload);
    effect!(WaveFini);
    let c = host.counts();
    let mut log = Log::new();
    let _ = writeln!(log, "Compiled phonemes: {} errors.", c.errors);
    log.send(host);
    effect!(CloseLog);
    let path = match crate::voice_storage::compat_path(string(&paths.manifest)) {
        Ok(p) => p,
        Err(e) => return host.file_error(errno(&e), &paths.manifest),
    };
    match manifest.read(&path, paths.width, mode, alpha, spaces) {
        Err(e) => host.file_error(errno(&e), &paths.manifest),
        Ok(0) => {
            if c.errors > 0 {
                COMPILE_ERROR
            } else {
                0
            }
        }
        Ok(status) => status,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_preserve_clipping_defaults_and_reject_unbounded_explicit_copies() {
        let p = Paths::new(b"/data", None, None, 16, false).unwrap();
        assert_eq!(string(&p.source), b"/data/../phsour");
        assert_eq!(string(&p.input), b"/data/../phsour");
        assert_eq!(string(&p.manifest), b"/data/phondata-");
        assert!(matches!(
            Paths::new(b"/data", Some(&[b'x'; 16]), None, 16, false),
            Err(code) if code==NAME_TOO_LONG
        ));
        assert!(matches!(
            Paths::new(b"/data", None, Some(b"x\0y"), 16, false),
            Err(22)
        ));
        assert_eq!(
            string(
                &Paths::new(b"C:", Some(b"s"), None, 260, true)
                    .unwrap()
                    .manifest
            ),
            b"C:\\phondata-manifest"
        );
    }
}
