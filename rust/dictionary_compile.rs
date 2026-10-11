//! Top-level dictionary compilation, source/output lifetime and header patching.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{engine_io::Writer, voice_storage};
use std::{fs::File, io};
pub const FAILED: i32 = 0x100001ff;
pub trait Host {
    fn lists(&mut self, prefix: &[u8], writer: &mut Writer) -> i32;
    fn rules(&mut self, path: &[u8], writer: &mut Writer) -> i32;
    fn log(&mut self, kind: i32, text: &[u8]);
    fn reload(&mut self, name: &[u8]);
    fn errors(&mut self) -> i32;
    fn error(&mut self, status: i32, path: &[u8]) -> i32;
}
pub struct Environment<'a> {
    pub source: &'a [u8],
    pub name: &'a [u8],
    pub home: &'a [u8],
    pub table: &'a [u8],
    pub path_width: usize,
    pub separator: u8,
}
fn path(width: usize, parts: &[&[u8]]) -> [u8; 4096] {
    let mut out = [0; 4096];
    let mut used = 0;
    for bytes in parts {
        let n = bytes.len().min(width - 1 - used);
        out[used..used + n].copy_from_slice(&bytes[..n]);
        used += n;
    }
    out
}
fn string(bytes: &[u8]) -> &[u8] {
    &bytes[..bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len())]
}
fn terminated(bytes: &[u8]) -> &[u8] {
    &bytes[..=string(bytes).len()]
}
fn errno(error: &io::Error) -> i32 {
    #[cfg(unix)]
    if let Some(code) = error.raw_os_error() {
        return code;
    }
    match error.kind() {
        io::ErrorKind::NotFound => 2,
        io::ErrorKind::PermissionDenied => 13,
        io::ErrorKind::AlreadyExists => 17,
        io::ErrorKind::IsADirectory => 21,
        io::ErrorKind::NotADirectory => 20,
        io::ErrorKind::OutOfMemory => 12,
        io::ErrorKind::InvalidInput | io::ErrorKind::InvalidData => 22,
        io::ErrorKind::StorageFull => 28,
        io::ErrorKind::WouldBlock => 11,
        _ => 5,
    }
}
fn output_error(host: &mut impl Host, error: io::Error, path: &[u8]) -> i32 {
    host.error(errno(&error), terminated(path))
}
pub fn compile(host: &mut impl Host, env: &Environment<'_>) -> i32 {
    if !(2..=4096).contains(&env.path_width)
        || env.name.len() > 4095
        || env.source.len() > 4095
        || env.home.len() > 4095
    {
        return FAILED;
    }
    let prefix = path(env.path_width, &[env.source, env.name, b"_"]);
    let mut input = path(env.path_width, &[string(&prefix), b"rules.txt"]);
    let probe = |bytes: &[u8]| -> io::Result<File> {
        File::open(voice_storage::compat_path(string(bytes))?)
    };
    let source = match probe(&input) {
        Ok(file) => file,
        Err(_) => {
            input = path(env.path_width, &[string(&prefix), b"rules"]);
            match probe(&input) {
                Ok(file) => file,
                Err(error) => return output_error(host, error, &input),
            }
        }
    };
    let output = path(
        env.path_width,
        &[env.home, &[env.separator], env.name, b"_dict"],
    );
    let destination = match voice_storage::compat_path(string(&output)) {
        Ok(path) => path,
        Err(error) => {
            drop(source);
            return output_error(host, error, &output);
        }
    };
    let mut writer = match Writer::create(&destination) {
        Ok(writer) => writer,
        Err(error) => {
            drop(source);
            return output_error(host, error, &output);
        }
    };
    if let Err(error) = writer
        .write(&1024u32.to_le_bytes())
        .and_then(|_| writer.write(&0u32.to_le_bytes()))
    {
        return output_error(host, error, &output);
    }
    // All path/name inputs were copied before any effect. The source probe is
    // retained through compilation, matching the legacy initial-open lifetime.
    let mut table = [0; 4096];
    let n = env.table.len().min(table.len() - 1);
    table[..n].copy_from_slice(&env.table[..n]);
    host.log(1, &table[..=n]);
    let status = host.lists(terminated(&prefix), &mut writer);
    if status != 0 {
        drop(source);
        let _ = writer.finish();
        return status;
    }
    let offset = writer.position() as u32;
    host.log(2, terminated(&input));
    let status = host.rules(terminated(&input), &mut writer);
    drop(source);
    let finished = writer
        .seek(4)
        .and_then(|_| writer.write(&offset.to_le_bytes()))
        .and_then(|_| writer.finish());
    host.log(3, b"\0");
    if status != 0 {
        return status;
    }
    if let Err(error) = finished {
        return output_error(host, error, &output);
    }
    let mut name = [0; 4096];
    name[..env.name.len()].copy_from_slice(env.name);
    host.reload(&name[..=env.name.len()]);
    if host.errors() > 0 {
        FAILED
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Effects {
        calls: Vec<i32>,
        list_status: i32,
        rule_status: i32,
        errors: i32,
        error_path: Vec<u8>,
    }
    impl Host for Effects {
        fn lists(&mut self, _: &[u8], writer: &mut Writer) -> i32 {
            self.calls.push(4);
            writer.write(&[0; 1024]).unwrap();
            self.list_status
        }
        fn rules(&mut self, _: &[u8], writer: &mut Writer) -> i32 {
            self.calls.push(5);
            writer.write(&[7, 0]).unwrap();
            self.rule_status
        }
        fn log(&mut self, kind: i32, _: &[u8]) {
            self.calls.push(kind);
        }
        fn reload(&mut self, _: &[u8]) {
            self.calls.push(6);
        }
        fn errors(&mut self) -> i32 {
            self.errors
        }
        fn error(&mut self, status: i32, path: &[u8]) -> i32 {
            self.error_path = path.to_vec();
            self.calls.push(7);
            status
        }
    }
    #[test]
    fn complete_header_effects_semantic_errors_and_partial_failure_files() {
        let _serial = crate::engine_io::writer_test_lock();
        let root =
            std::env::temp_dir().join(format!("espeak-dictionary-driver-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let source = format!("{}/", root.display()).into_bytes();
        let home = root.to_string_lossy().as_bytes().to_vec();
        let env = Environment {
            source: &source,
            name: b"en",
            home: &home,
            table: b"en",
            path_width: 4096,
            separator: b'/',
        };
        let mut host = Effects::default();
        assert_eq!(compile(&mut host, &env), 2);
        assert_eq!(host.calls, [7]);
        assert!(host.error_path.ends_with(b"en_rules\0"));
        assert!(!root.join("en_dict").exists());
        std::fs::write(root.join("en_rules"), b".group a\na a\n").unwrap();
        let mut host = Effects::default();
        assert_eq!(compile(&mut host, &env), 0);
        assert_eq!(host.calls, [1, 4, 2, 5, 3, 6]);
        let bytes = std::fs::read(root.join("en_dict")).unwrap();
        assert_eq!(&bytes[..8], &[0, 4, 0, 0, 8, 4, 0, 0]);
        assert_eq!(&bytes[1032..], &[7, 0]);
        let mut host = Effects {
            errors: 1,
            ..Effects::default()
        };
        assert_eq!(compile(&mut host, &env), FAILED);
        assert!(host.calls.contains(&6));
        let mut host = Effects {
            rule_status: FAILED,
            ..Effects::default()
        };
        assert_eq!(compile(&mut host, &env), FAILED);
        assert_eq!(host.calls, [1, 4, 2, 5, 3]);
        assert_eq!(std::fs::read(root.join("en_dict")).unwrap(), bytes);
        let mut host = Effects {
            list_status: FAILED,
            ..Effects::default()
        };
        assert_eq!(compile(&mut host, &env), FAILED);
        assert_eq!(host.calls, [1, 4]);
        assert_eq!(std::fs::read(root.join("en_dict")).unwrap().len(), 1032);
        std::fs::remove_file(root.join("en_dict")).unwrap();
        std::fs::create_dir(root.join("en_dict")).unwrap();
        let mut host = Effects::default();
        assert!(compile(&mut host, &env) > 0);
        assert_eq!(host.calls, [7]);
        assert!(host.error_path.ends_with(b"en_dict\0"));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn path_truncation_preserves_snprintf_prefix_and_separator_bytes() {
        assert_eq!(terminated(&path(8, &[b"abcdef", b"ghi"])), b"abcdefg\0");
        assert_eq!(
            terminated(&path(20, &[b"home", b"\\", b"en", b"_dict"])),
            b"home\\en_dict\0"
        );
    }
}
