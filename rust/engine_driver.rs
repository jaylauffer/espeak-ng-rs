//! Native synthesis startup and bounded buffer/clause passes.
//!
//! The host owns resource projections and the completion runner. A refused
//! runner returns stopped speech without replaying any pass in a local loop.
// SPDX-License-Identifier: GPL-3.0-or-later
use std::ffi::c_void;
pub const NOT_INITIALIZED: u32 = 0x100004ff;
pub const AUDIO_ERROR: u32 = 0x100005ff;
pub const STOPPED: u32 = 0x10000eff;
#[derive(Debug, PartialEq, Eq)]
pub enum Pass {
    Continue,
    Done(u32),
}
pub trait Host {
    fn buffers_ready(&self) -> bool;
    fn configure(&mut self, flags: i32);
    fn has_translator(&self) -> bool;
    fn default_voice(&mut self) -> u32;
    fn has_decoder(&self) -> bool;
    fn create_decoder(&mut self);
    fn decode(&mut self, text: *const c_void, flags: i32) -> u32;
    fn clause(&mut self, control: i32) -> i32;
    fn run(&mut self, id: u32) -> Result<u32, u32>;
    fn begin_buffer(&mut self);
    fn fill_buffer(&mut self);
    fn collect_buffer(&mut self, id: u32) -> Result<i32, u32>;
    fn playback(&self) -> bool;
    fn play(&mut self, length: i32, end: bool) -> i32;
    fn callback(&mut self, length: i32, end: bool) -> i32;
    fn generate(&mut self) -> i32;
    fn queued(&self) -> i32;
    fn terminate_current_events(&mut self);
}
pub fn synthesize(host: &mut impl Host, id: u32, text: *const c_void, flags: i32) -> u32 {
    if !host.buffers_ready() {
        return NOT_INITIALIZED;
    }
    host.configure(flags);
    if !host.has_translator() {
        let status = host.default_voice();
        if status != 0 {
            return status;
        }
    }
    if !host.has_decoder() {
        host.create_decoder();
    }
    let status = host.decode(text, flags);
    if status != 0 {
        return status;
    }
    host.clause(0);
    match host.run(id) {
        Ok(status) => status,
        Err(_) => {
            host.clause(2);
            STOPPED
        }
    }
}
pub fn step(host: &mut impl Host, id: u32) -> Pass {
    host.begin_buffer();
    host.fill_buffer();
    let length = match host.collect_buffer(id) {
        Ok(length) => length,
        Err(status) => {
            host.clause(2);
            return Pass::Done(status);
        }
    };
    let finished = if host.playback() {
        let result = host.play(length, false);
        if result < 0 {
            return Pass::Done(AUDIO_ERROR);
        }
        result
    } else {
        host.callback(length, false)
    };
    if finished != 0 {
        host.clause(2);
        return Pass::Done(STOPPED);
    }
    if host.generate() == 0 && host.queued() == 0 {
        // A clause boundary must be an output-buffer boundary, including
        // <audio>. Publish current identity before asking for the next clause.
        host.terminate_current_events();
        if host.clause(1) == 0 {
            let finished = if host.playback() {
                if host.play(0, true) < 0 {
                    return Pass::Done(AUDIO_ERROR);
                }
                0 // Legacy end playback ignores a positive dispatch result.
            } else {
                host.callback(0, true)
            };
            if finished != 0 {
                host.clause(2);
                return Pass::Done(STOPPED);
            }
            return Pass::Done(0);
        }
    }
    Pass::Continue
}
#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Spy {
        ready: bool,
        translator: bool,
        decoder: bool,
        voice_error: u32,
        decode_error: u32,
        run_error: bool,
        generated: i32,
        queued: i32,
        audio: bool,
        next: i32,
        delivery: i32,
        end_delivery: i32,
        stops: usize,
        boundaries: usize,
        ended: usize,
        passes: usize,
    }
    impl Host for Spy {
        fn buffers_ready(&self) -> bool {
            self.ready
        }
        fn configure(&mut self, _: i32) {}
        fn has_translator(&self) -> bool {
            self.translator
        }
        fn default_voice(&mut self) -> u32 {
            self.translator = true;
            self.voice_error
        }
        fn has_decoder(&self) -> bool {
            self.decoder
        }
        fn create_decoder(&mut self) {
            self.decoder = true;
        }
        fn decode(&mut self, _: *const c_void, _: i32) -> u32 {
            self.decode_error
        }
        fn clause(&mut self, control: i32) -> i32 {
            if control == 2 {
                self.stops += 1;
            }
            if control == 1 {
                self.boundaries += 1;
            }
            self.next
        }
        fn run(&mut self, _: u32) -> Result<u32, u32> {
            if self.run_error {
                Err(22)
            } else {
                Ok(123)
            }
        }
        fn begin_buffer(&mut self) {
            self.passes += 1;
        }
        fn fill_buffer(&mut self) {}
        fn collect_buffer(&mut self, _: u32) -> Result<i32, u32> {
            Ok(4)
        }
        fn playback(&self) -> bool {
            self.audio
        }
        fn play(&mut self, _: i32, end: bool) -> i32 {
            self.callback(0, end)
        }
        fn callback(&mut self, _: i32, end: bool) -> i32 {
            if end {
                self.ended += 1;
                self.end_delivery
            } else {
                self.delivery
            }
        }
        fn generate(&mut self) -> i32 {
            self.generated
        }
        fn queued(&self) -> i32 {
            self.queued
        }
        fn terminate_current_events(&mut self) {}
    }
    #[test]
    fn initialization_and_decode_failures_do_not_start_or_replay_passes() {
        let mut host = Spy::default();
        assert_eq!(
            synthesize(&mut host, 7, std::ptr::null(), 0),
            NOT_INITIALIZED
        );
        assert!(!host.translator && !host.decoder);
        host.ready = true;
        host.voice_error = 99;
        assert_eq!(synthesize(&mut host, 7, std::ptr::null(), 0), 99);
        assert!(!host.decoder);
        host.voice_error = 0;
        host.decode_error = 88;
        assert_eq!(synthesize(&mut host, 7, std::ptr::null(), 0), 88);
        host.decode_error = 0;
        host.run_error = true;
        assert_eq!(synthesize(&mut host, 7, std::ptr::null(), 0), STOPPED);
        assert_eq!((host.stops, host.passes), (1, 0));
    }
    #[test]
    fn clauses_wait_for_both_generation_and_output_queue() {
        for (generated, queued) in [(1, 0), (0, 1), (-1, 0)] {
            let mut host = Spy {
                generated,
                queued,
                ..Spy::default()
            };
            assert_eq!(step(&mut host, 7), Pass::Continue);
            assert_eq!((host.boundaries, host.ended), (0, 0));
        }
        let mut host = Spy {
            next: 1,
            ..Spy::default()
        };
        assert_eq!(step(&mut host, 7), Pass::Continue);
        assert_eq!((host.boundaries, host.ended), (1, 0));
    }
    #[test]
    fn audio_failures_and_retrieval_abort_keep_distinct_status_and_cleanup() {
        let mut host = Spy {
            audio: true,
            delivery: -1,
            ..Spy::default()
        };
        assert_eq!(step(&mut host, 7), Pass::Done(AUDIO_ERROR));
        assert_eq!(host.stops, 0);
        host.audio = false;
        assert_eq!(step(&mut host, 7), Pass::Done(STOPPED));
        assert_eq!(host.stops, 1);
        host.delivery = 0;
        host.audio = true;
        host.end_delivery = 1;
        assert_eq!(step(&mut host, 7), Pass::Done(0));
        assert_eq!(host.stops, 1);
        host.audio = false;
        assert_eq!(step(&mut host, 7), Pass::Done(STOPPED));
        assert_eq!(host.stops, 2);
    }
}
