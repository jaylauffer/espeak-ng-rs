//! Legacy API initialization and compiler control, over the owned engine host.
// SPDX-License-Identifier: GPL-3.0-or-later
pub trait Host {
    fn path(&mut self);
    fn initialize(&mut self) -> u32;
    fn diagnose(&mut self, status: u32);
    fn clear(&mut self);
    fn output(&mut self, mode: i32, length: i32);
    fn events(&mut self, flags: i32);
    fn rate(&mut self) -> i32;
    fn compile(&mut self) -> u32;
}
/// Exit request is explicit for native hosts; the C adapter preserves exit(1).
pub fn initialize(
    host: &mut impl Host,
    output: i32,
    length: i32,
    options: i32,
) -> Result<i32, i32> {
    host.path();
    let status = host.initialize();
    if status != 0 {
        host.diagnose(status);
        host.clear();
        if options & 0x8000 == 0 {
            return Err(1);
        }
    }
    if let Some(mode) = [2, 0, 1, 3].get(usize::try_from(output).unwrap_or(usize::MAX)) {
        // Legacy initialization ignores output errors; preserve that API.
        host.output(*mode, length);
    }
    host.events(options & 3);
    Ok(host.rate())
}
pub fn compile(host: &mut impl Host) {
    let status = host.compile();
    if status != 0 {
        host.diagnose(status);
        host.clear();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Spy {
        status: u32,
        calls: Vec<i32>,
    }
    impl Host for Spy {
        fn path(&mut self) {
            self.calls.push(10);
        }
        fn initialize(&mut self) -> u32 {
            self.calls.push(11);
            self.status
        }
        fn diagnose(&mut self, _: u32) {
            self.calls.push(12);
        }
        fn clear(&mut self) {
            self.calls.push(13);
        }
        fn output(&mut self, m: i32, _: i32) {
            self.calls.push(20 + m);
        }
        fn events(&mut self, f: i32) {
            self.calls.push(30 + f);
        }
        fn rate(&mut self) -> i32 {
            self.calls.push(40);
            22050
        }
        fn compile(&mut self) -> u32 {
            self.calls.push(50);
            self.status
        }
    }
    #[test]
    fn failed_initialization_clears_before_exit_and_dont_exit_keeps_legacy_flow() {
        let mut host = Spy {
            status: 12,
            calls: Vec::new(),
        };
        assert_eq!(initialize(&mut host, 0, 80, 0), Err(1));
        assert_eq!(host.calls, [10, 11, 12, 13]);
        host.calls.clear();
        assert_eq!(initialize(&mut host, 3, 80, 0x8003), Ok(22050));
        assert_eq!(host.calls, [10, 11, 12, 13, 23, 33, 40]);
    }
    #[test]
    fn modes_unknown_output_and_compiler_cleanup_match_the_api() {
        let mut host = Spy {
            status: 0,
            calls: Vec::new(),
        };
        for (out, mode) in [(0, 2), (1, 0), (2, 1), (3, 3)] {
            host.calls.clear();
            initialize(&mut host, out, 0, 0).unwrap();
            assert_eq!(host.calls, [10, 11, 20 + mode, 30, 40]);
        }
        host.calls.clear();
        initialize(&mut host, -1, 0, 2).unwrap();
        assert_eq!(host.calls, [10, 11, 32, 40]);
        host.calls.clear();
        compile(&mut host);
        assert_eq!(host.calls, [50]);
        host.calls.clear();
        host.status = 12;
        compile(&mut host);
        assert_eq!(host.calls, [50, 12, 13]);
    }
}
