//! Native status conversion and byte-preserving diagnostics.
// SPDX-License-Identifier: GPL-3.0-or-later
pub const VERSION_MISMATCH: u32 = 0x1000_02ff;
pub const SPEECH_STOPPED: u32 = 0x1000_0eff;

pub fn legacy(status: u32) -> i32 {
    match status {
        0 | SPEECH_STOPPED => 0,
        0x1000_03ff => 1,
        0x1000_06ff..=0x1000_08ff if status & 0xff == 0xff => 2,
        _ => -1,
    }
}
pub enum Message {
    Builtin(&'static [u8]),
    Errno(u32),
    Other(u32),
}
pub fn message(status: u32) -> Message {
    let text: &[u8] = match status {
        0x1000_01ff => b"Compile error",
        VERSION_MISMATCH => b"Wrong version of espeak-ng-data",
        0x1000_03ff => b"The FIFO buffer is full",
        0x1000_04ff => b"The espeak-ng library has not been initialized",
        0x1000_05ff => b"Cannot initialize the audio device",
        0x1000_06ff => b"The specified espeak-ng voice does not exist",
        0x1000_07ff => b"Could not load the mbrola.dll file",
        0x1000_08ff => b"Could not load the specified mbrola voice file",
        0x1000_09ff => b"The event buffer is full",
        0x1000_0aff => b"The requested functionality has not been built into espeak-ng",
        0x1000_0bff => b"The phoneme file is not in a supported format",
        0x1000_0cff => b"The spectral file does not contain any frame data",
        0x1000_0dff => b"The phoneme manifest file does not contain any phonemes",
        0x1000_0fff => b"The phoneme feature is not recognised",
        0x1000_10ff => b"The text encoding is not supported",
        _ if status & 0x7000_0000 == 0 => return Message::Errno(status),
        _ => return Message::Other(status),
    };
    Message::Builtin(text)
}
pub fn hex(value: u32, bytes: &mut [u8; 8]) -> &[u8] {
    let mut value = value;
    let mut start = bytes.len();
    loop {
        start -= 1;
        bytes[start] = b"0123456789abcdef"[(value & 15) as usize];
        value >>= 4;
        if value == 0 {
            break;
        }
    }
    &bytes[start..]
}
pub fn other(status: u32, mut write: impl FnMut(&[u8])) {
    write(b"Unspecified error 0x");
    write(hex(status, &mut [0; 8]));
}
pub enum Context<'a> {
    File(&'a [u8]),
    Version {
        name: &'a [u8],
        expected: i32,
        actual: i32,
    },
    Unknown,
}
/// Emit bounded stack fragments, preserving arbitrary filename bytes.
/// A stdio adapter holds its stream lock for the entire diagnostic.
pub fn diagnostic(context: Option<Context<'_>>, message: &[u8], mut write: impl FnMut(&[u8])) {
    match context {
        None => {
            write(b"Error: ");
            write(message);
            write(b".\n");
        }
        Some(Context::File(name)) => {
            write(b"Error processing file '");
            write(name);
            write(b"': ");
            write(message);
            write(b".\n");
        }
        Some(Context::Version {
            name,
            expected,
            actual,
        }) => {
            write(b"Error: ");
            write(message);
            write(b" at '");
            write(name);
            write(b"' (expected 0x");
            write(hex(expected as u32, &mut [0; 8]));
            write(b", got 0x");
            write(hex(actual as u32, &mut [0; 8]));
            write(b").\n");
        }
        Some(Context::Unknown) => {}
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conversion_and_unknown_hex_preserve_exact_status_bits() {
        for (code, expected) in [
            (0, 0),
            (SPEECH_STOPPED, 0),
            (0x1000_03ff, 1),
            (0x1000_06ff, 2),
            (0x1000_07ff, 2),
            (0x1000_08ff, 2),
            (0x1000_0700, -1),
            (12, -1),
        ] {
            assert_eq!(legacy(code), expected);
        }
        let mut bytes = Vec::new();
        other(u32::MAX, |part| bytes.extend_from_slice(part));
        assert_eq!(bytes, b"Unspecified error 0xffffffff");
    }
    #[test]
    fn diagnostic_preserves_non_utf8_names_and_signed_version_bits() {
        let mut bytes = Vec::new();
        diagnostic(
            Some(Context::Version {
                name: b"\xff/path",
                expected: -1,
                actual: 0,
            }),
            b"wrong",
            |part| bytes.extend_from_slice(part),
        );
        assert_eq!(
            bytes,
            b"Error: wrong at '\xff/path' (expected 0xffffffff, got 0x0).\n"
        );
        bytes.clear();
        diagnostic(Some(Context::Unknown), b"wrong", |part| {
            bytes.extend_from_slice(part)
        });
        assert!(bytes.is_empty());
    }
}
