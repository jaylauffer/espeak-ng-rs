//! CLI input framing and WAV ownership. Regular file contents use engine I/O;
//! inherited streams retain the C runtime primitives until the proactor offers
//! portable stream operations (positioned reads cannot read pipes or consoles).
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{engine_io, events::Event, voice_storage::compat_path};
use libc::FILE;
use std::{ffi::CString, io};

const HEADER: [u8; 44] = [
    b'R', b'I', b'F', b'F', 0x24, 0xf0, 0xff, 0x7f, b'W', b'A', b'V', b'E', b'f', b'm', b't', b' ',
    0x10, 0, 0, 0, 1, 0, 1, 0, 9, 0x3d, 0, 0, 0x12, 0x7a, 0, 0, 2, 0, 0x10, 0, b'd', b'a', b't',
    b'a', 0, 0xf0, 0xff, 0x7f,
];

/// The CLI owns one instance for its synchronous synthesis callback. Stream
/// addresses are borrowed for that complete invocation, never closed here.
pub struct WaveOutput {
    quiet: bool,
    path: Vec<u8>,
    extension: Vec<u8>,
    rate: i32,
    split_seconds: u32,
    split_samples: u32,
    samples: u32,
    count: u32,
    file: Option<engine_io::OutputFile>,
    buffer: Vec<u8>,
    stdout_open: bool,
    stdout: usize,
    stderr: usize,
    binary_stdout: unsafe extern "C" fn(*mut FILE),
}
impl WaveOutput {
    /// # Safety
    /// Both streams and the platform primitive must remain valid until finish
    /// and drop; the caller serializes all use of this output instance.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn new(
        quiet: bool,
        mut path: Vec<u8>,
        split_seconds: u32,
        rate: i32,
        stdout: *mut FILE,
        stderr: *mut FILE,
        binary_stdout: unsafe extern "C" fn(*mut FILE),
    ) -> Self {
        let mut extension = Vec::new();
        if split_seconds.wrapping_mul(rate as u32) > 0 {
            if let Some(dot) = path.iter().rposition(|&b| b == b'.') {
                if path.len() - dot <= 4 {
                    extension.extend_from_slice(&path[dot..]);
                    path.truncate(dot);
                }
            }
        }
        Self {
            quiet,
            path,
            extension,
            rate,
            split_seconds,
            split_samples: split_seconds.wrapping_mul(rate as u32),
            samples: 0,
            count: 0,
            file: None,
            buffer: Vec::new(),
            stdout_open: false,
            stdout: stdout as usize,
            stderr: stderr as usize,
            binary_stdout,
        }
    }
    fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        if self.stdout_open {
            // SAFETY: constructor's invocation-wide borrowed stdout contract.
            let n = unsafe {
                libc::fwrite(
                    bytes.as_ptr().cast(),
                    1,
                    bytes.len(),
                    self.stdout as *mut FILE,
                )
            };
            if n != bytes.len() {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        } else {
            self.file
                .as_mut()
                .ok_or(io::ErrorKind::NotConnected)?
                .write(bytes)
        }
    }
    fn open(&mut self) -> io::Result<()> {
        let mut name = self.path.clone();
        if self.split_samples > 0 {
            name.extend_from_slice(format!("_{:02}", self.count.wrapping_add(1) as i32).as_bytes());
            name.extend_from_slice(&self.extension);
        }
        let start = name
            .iter()
            .position(|&b| !b.is_ascii_whitespace())
            .unwrap_or(name.len());
        let name = &name[start..];
        let result = if name == b"stdout" {
            // SAFETY: supplied primitive and stdout are valid for this invocation.
            unsafe { (self.binary_stdout)(self.stdout as *mut FILE) };
            self.stdout_open = true;
            Ok(())
        } else {
            compat_path(name).and_then(|path| {
                engine_io::OutputFile::create(&path, std::mem::take(&mut self.buffer))
                    .map(|file| self.file = Some(file))
            })
        };
        if result.is_err() {
            let message = CString::new(name).unwrap_or_default();
            // SAFETY: name is NUL terminated, format and stream are valid.
            unsafe {
                libc::fprintf(
                    self.stderr as *mut FILE,
                    c"Can't write to: '%s'\n".as_ptr(),
                    message.as_ptr(),
                );
            }
        }
        result?;
        let mut header = HEADER;
        header[24..28].copy_from_slice(&self.rate.to_le_bytes());
        header[28..32].copy_from_slice(&self.rate.wrapping_mul(2).to_le_bytes());
        self.write(&header)
    }
    pub fn callback(&mut self, samples: &[i16], events: &[Event]) -> i32 {
        if self.quiet {
            return 0;
        }
        for event in events {
            if event.kind == 8 {
                self.rate = i32::from_ne_bytes(event.id[..4].try_into().unwrap());
                self.split_samples = self.split_seconds.wrapping_mul(self.rate as u32);
            } else if event.kind == 2 && self.split_samples > 0 && self.samples > self.split_samples
            {
                if self.finish().is_err() {
                    return 1;
                }
                self.samples = 0;
                self.count = self.count.wrapping_add(1);
            }
        }
        if self.file.is_none() && !self.stdout_open && self.open().is_err() {
            return 1;
        }
        self.samples = self.samples.wrapping_add(samples.len() as u32);
        // SAFETY: every initialized i16 can be viewed as its two native-endian
        // bytes, exactly like the original fwrite(short*, n*2). No new buffer.
        let bytes =
            unsafe { std::slice::from_raw_parts(samples.as_ptr().cast(), samples.len() * 2) };
        i32::from(self.write(bytes).is_err())
    }
    pub fn finish(&mut self) -> io::Result<()> {
        if let Some(mut file) = self.file.take() {
            let result = (|| {
                let position = file.position() as u32;
                file.seek(4)?;
                file.write(&position.wrapping_sub(8).to_le_bytes())?;
                file.seek(40)?;
                file.write(&position.wrapping_sub(44).to_le_bytes())?;
                file.flush()
            })();
            self.buffer = file.into_buffer();
            result
        } else {
            Ok(())
        }
    }
}

fn reserve(bytes: &mut Vec<u8>, extra: usize) -> Result<(), i32> {
    bytes.try_reserve(extra).map_err(|_| libc::ENOMEM)
}

/// Zero-pad `used` input bytes so that a whole zero unit follows the text's
/// last unit in every input encoding: bytes, 16-bit units or `wchar_t`. The
/// engine finds the end of wide input by scanning for a zero `wchar_t`, and
/// four bytes after unaligned text (13 bytes, say) do not make one. At least
/// four zero bytes, as before.
fn nul_terminate(bytes: &mut Vec<u8>, used: usize) -> Result<(), i32> {
    const WIDE: usize = std::mem::size_of::<libc::wchar_t>();
    let end = used
        .div_ceil(WIDE)
        .checked_add(1)
        .and_then(|units| units.checked_mul(WIDE))
        .ok_or(libc::ENOMEM)?
        .max(used + 4);
    bytes.truncate(used);
    reserve(bytes, end - used)?;
    bytes.resize(end, 0);
    Ok(())
}

/// Windows fopen("r") maps CRLF and stops at Ctrl-Z, including regular files
/// whose proactor reads are binary. The other platforms preserve every byte.
fn text_mode(bytes: &mut Vec<u8>, windows: bool) {
    if !windows {
        return;
    }
    let mut read = 0;
    let mut written = 0;
    while read < bytes.len() && bytes[read] != 0x1a {
        if bytes[read] == b'\r' && bytes.get(read + 1) == Some(&b'\n') {
            read += 1;
        }
        bytes[written] = bytes[read];
        read += 1;
        written += 1;
    }
    bytes.truncate(written);
}

/// Execute exactly the CLI's three framing modes: first argument, 999-byte
/// fgets records, or one entire file/--stdin input. Synth receives initialized
/// NUL padding as well as the legacy public API size argument.
/// Returns -1 after a diagnosed input failure, or ENOMEM for caller diagnosis.
/// # Safety
/// All three streams are live borrowed CRT streams during the call. Synthesis
/// must finish copying/consuming its input before this callback returns.
#[allow(clippy::too_many_arguments)]
pub unsafe fn run_input(
    filename: &[u8],
    stdin_mode: bool,
    positional: Option<&[u8]>,
    stdin: *mut FILE,
    stdout: *mut FILE,
    stderr: *mut FILE,
    mut synth: impl FnMut(&[u8], usize),
) -> Result<(), i32> {
    let mut bytes = Vec::new();
    if filename.is_empty() && !stdin_mode {
        if let Some(text) = positional {
            reserve(&mut bytes, text.len())?;
            bytes.extend_from_slice(text);
            nul_terminate(&mut bytes, text.len())?;
            synth(&bytes, text.len() + 1);
            return Ok(());
        }
    }
    let mut stream = stdin;
    let mut close = false;
    let mut lines = !stdin_mode;
    if !filename.is_empty() {
        let name = CString::new(filename).map_err(|_| -1)?;
        let path = compat_path(filename).map_err(|_| -1)?;
        let metadata = match std::fs::metadata(&path) {
            Ok(value) => value,
            Err(_) => {
                // SAFETY: caller supplies a live stream; name/format are C strings.
                unsafe {
                    libc::fprintf(
                        stderr,
                        c"Failed to stat() file '%s'\n".as_ptr(),
                        name.as_ptr(),
                    );
                }
                return Err(-1);
            }
        };
        #[cfg(unix)]
        let fifo = {
            use std::os::unix::fs::FileTypeExt;
            metadata.file_type().is_fifo()
        };
        #[cfg(not(unix))]
        let fifo = false;
        if stdin_mode && !fifo {
            // --stdin wins over a regular -f file: the original only opens the
            // file (failing as below), then reads stdin in bulk.
            // SAFETY: name/mode are NUL-terminated and remain alive for fopen.
            let file = unsafe { libc::fopen(name.as_ptr(), c"r".as_ptr()) };
            if file.is_null() {
                // SAFETY: caller supplies live stderr and valid C strings.
                unsafe {
                    libc::fprintf(
                        stderr,
                        c"Failed to read file '%s'\n".as_ptr(),
                        name.as_ptr(),
                    );
                }
                return Err(-1);
            }
            // SAFETY: the successful fopen result is closed exactly once.
            unsafe {
                libc::fclose(file);
            }
        } else if !fifo {
            if metadata.is_dir() {
                return Err(libc::ENOMEM);
            }
            let expected = usize::try_from(metadata.len()).map_err(|_| libc::ENOMEM)?;
            if expected > i32::MAX as usize {
                return Err(libc::ENOMEM);
            }
            // Existing reader owns registration/lifetime and reuses its bounded
            // completion buffer; no C fread or app-specific worker for this path.
            if let Err(error) = engine_io::read_file_into(&path, i32::MAX as usize, &mut bytes) {
                if error.kind() == io::ErrorKind::OutOfMemory {
                    return Err(libc::ENOMEM);
                }
                // SAFETY: caller supplies a live stream; name/format are C strings.
                unsafe {
                    libc::fprintf(
                        stderr,
                        c"Failed to read file '%s'\n".as_ptr(),
                        name.as_ptr(),
                    );
                }
                return Err(-1);
            }
            text_mode(&mut bytes, cfg!(windows));
            let used = bytes.len();
            if used != expected {
                // SAFETY: exact printf argument types; stream is live.
                unsafe {
                    libc::fprintf(
                        stderr,
                        c"Warning: expected %d bytes from input file, got %zu\n".as_ptr(),
                        expected as i32,
                        used,
                    );
                }
            }
            nul_terminate(&mut bytes, used)?;
            synth(&bytes, used + 1);
            return Ok(());
        }
        if fifo {
            // FIFO open and stream operations remain explicit platform primitives.
            // SAFETY: name/mode are NUL-terminated and remain alive for fopen.
            stream = unsafe { libc::fopen(name.as_ptr(), c"r".as_ptr()) };
            if stream.is_null() {
                // SAFETY: caller supplies live stderr and valid C strings.
                unsafe {
                    libc::fprintf(
                        stderr,
                        c"Failed to read file '%s'\n".as_ptr(),
                        name.as_ptr(),
                    );
                }
                return Err(-1);
            }
            close = true;
            lines = true;
        }
    }
    // This guard closes only a FIFO that we opened, including on allocation errors.
    struct Stream {
        ptr: *mut FILE,
        close: bool,
    }
    impl Drop for Stream {
        fn drop(&mut self) {
            if self.close {
                // SAFETY: this guard uniquely owns the successful fopen result.
                unsafe {
                    libc::fclose(self.ptr);
                }
            }
        }
    }
    let stream = Stream { ptr: stream, close };
    if lines {
        reserve(&mut bytes, 1004)?;
        bytes.resize(1004, 0);
        // SAFETY: the live stream and initialized writable 1004-byte buffer are
        // valid; fgets writes at most 1000 bytes including the terminator.
        while !unsafe { libc::fgets(bytes.as_mut_ptr().cast(), 1000, stream.ptr) }.is_null() {
            bytes[999] = 0;
            synth(&bytes, 1000);
            // SAFETY: stdout remains live throughout the invocation.
            unsafe {
                libc::fflush(stdout);
            }
        }
    } else {
        let mut chunk = [0u8; 8192];
        loop {
            // SAFETY: the live stream and writable chunk are valid. CRT fread
            // supplies the same text-mode bytes as getc, with less call overhead.
            let used =
                unsafe { libc::fread(chunk.as_mut_ptr().cast(), 1, chunk.len(), stream.ptr) };
            if used == 0 {
                break;
            }
            reserve(&mut bytes, used)?;
            bytes.extend_from_slice(&chunk[..used]);
        }
        if !bytes.is_empty() {
            let used = bytes.len();
            nul_terminate(&mut bytes, used)?;
            synth(&bytes, used);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    unsafe extern "C" fn binary(_: *mut FILE) {}
    struct Streams(*mut FILE, *mut FILE);
    impl Streams {
        fn new() -> Self {
            // SAFETY: tmpfile has no arguments and returns independent owned streams.
            let s = unsafe { Self(libc::tmpfile(), libc::tmpfile()) };
            assert!(!s.0.is_null() && !s.1.is_null());
            s
        }
    }
    impl Drop for Streams {
        fn drop(&mut self) {
            // SAFETY: streams remain live and are each closed once by their owner.
            unsafe {
                libc::fclose(self.0);
                libc::fclose(self.1);
            }
        }
    }
    fn sentence() -> Event {
        Event {
            kind: 2,
            ..Event::default()
        }
    }
    fn sample_rate(rate: i32) -> Event {
        let mut event = Event {
            kind: 8,
            ..Event::default()
        };
        event.id[..4].copy_from_slice(&rate.to_ne_bytes());
        event
    }
    #[test]
    fn wav_splits_strictly_after_threshold_and_reuses_buffer_with_rate_events() {
        let root = std::env::temp_dir().join(format!("espeak-cli-wave-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let streams = Streams::new();
        let name = root.join("speech.wav");
        // SAFETY: owned streams outlive output; binary is a no-op test primitive.
        let mut output = unsafe {
            WaveOutput::new(
                false,
                name.to_str().unwrap().as_bytes().to_vec(),
                1,
                2,
                streams.0,
                streams.1,
                binary,
            )
        };
        assert_eq!(output.callback(&[1, 2], &[]), 0);
        assert_eq!(output.callback(&[3], &[sentence()]), 0); // equal: no split
        let first_buffer = output.file.as_ref().unwrap().bytes_pointer();
        assert_eq!(output.callback(&[4], &[sentence(), sample_rate(4)]), 0);
        assert_eq!(output.file.as_ref().unwrap().bytes_pointer(), first_buffer);
        output.finish().unwrap();
        let first = std::fs::read(root.join("speech_01.wav")).unwrap();
        let second = std::fs::read(root.join("speech_02.wav")).unwrap();
        assert_eq!(first.len(), 50);
        assert_eq!(&first[4..8], &42u32.to_le_bytes());
        assert_eq!(&first[40..44], &6u32.to_le_bytes());
        assert_eq!(&first[24..28], &2u32.to_le_bytes());
        assert_eq!(&second[24..28], &4u32.to_le_bytes());
        assert_eq!(&second[40..44], &2u32.to_le_bytes());
        assert_eq!(&second[44..], &4i16.to_ne_bytes());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn stdout_wav_preserves_placeholder_lengths_and_quiet_does_not_open() {
        let streams = Streams::new();
        // SAFETY: owned streams outlive output; binary is a no-op test primitive.
        let mut output = unsafe {
            WaveOutput::new(
                false,
                b"  stdout".to_vec(),
                0,
                22050,
                streams.0,
                streams.1,
                binary,
            )
        };
        assert_eq!(output.callback(&[10, -10], &[]), 0);
        output.finish().unwrap();
        let mut data = [0u8; 48];
        // SAFETY: live owned stream, valid seek, and initialized output storage.
        unsafe {
            assert_eq!(libc::fseek(streams.0, 0, libc::SEEK_SET), 0);
            assert_eq!(
                libc::fread(data.as_mut_ptr().cast(), 1, data.len(), streams.0),
                48
            );
        }
        assert_eq!(&data[4..8], &HEADER[4..8]);
        assert_eq!(&data[40..44], &HEADER[40..44]);
        // SAFETY: streams stay valid until this quiet output is dropped.
        let mut quiet = unsafe {
            WaveOutput::new(
                true,
                b"invalid/no.wav".to_vec(),
                1,
                10,
                streams.0,
                streams.1,
                binary,
            )
        };
        assert_eq!(quiet.callback(&[1], &[sample_rate(123)]), 0);
        assert_eq!(quiet.rate, 10);
        assert!(quiet.file.is_none());
    }
    #[test]
    fn windows_regular_text_matches_crt_translation_without_changing_unix_bytes() {
        let input = b"a\r\nb\rc\n\r\n\x1alater\r\n";
        let mut text = input.to_vec();
        text_mode(&mut text, true);
        assert_eq!(text, b"a\nb\rc\n\n");
        let mut binary = input.to_vec();
        text_mode(&mut binary, false);
        assert_eq!(binary, input);
    }
    #[test]
    fn nul_padding_ends_every_input_length_with_a_whole_zero_wide_unit() {
        const WIDE: usize = std::mem::size_of::<libc::wchar_t>();
        for used in 0..=13 {
            let mut bytes = vec![0xffu8; used];
            nul_terminate(&mut bytes, used).unwrap();
            assert_eq!(&bytes[..used], &vec![0xffu8; used][..]);
            assert!(bytes.len() >= used + 4 && bytes.len() % WIDE == 0);
            let unit = used.div_ceil(WIDE) * WIDE;
            assert!(
                bytes[unit..unit + WIDE].iter().all(|&b| b == 0),
                "{used} bytes"
            );
        }
    }
    #[test]
    fn stdin_flag_reads_stdin_in_bulk_even_with_a_regular_input_file() {
        let dir = std::env::temp_dir().join(format!("espeak-cli-stdin-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("input.txt");
        std::fs::write(&path, b"Not the stdin bytes.\n").unwrap();
        let streams = Streams::new();
        let typed = b"Read stdin.\nSecond line.\n";
        // SAFETY: live owned stream, initialized input bytes and a valid seek.
        unsafe {
            assert_eq!(
                libc::fwrite(typed.as_ptr().cast(), 1, typed.len(), streams.0),
                typed.len()
            );
            assert_eq!(libc::fseek(streams.0, 0, libc::SEEK_SET), 0);
        }
        let mut spoken = Vec::new();
        // SAFETY: both owned streams stay live for the call.
        let result = unsafe {
            run_input(
                path.to_str().unwrap().as_bytes(),
                true,
                None,
                streams.0,
                streams.1,
                streams.1,
                |text, length| spoken.push(text[..length].to_vec()),
            )
        };
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(result, Ok(()));
        // One bulk call with all of stdin, as the original's fgetc loop makes.
        assert_eq!(spoken, [typed.to_vec()]);
    }
}
