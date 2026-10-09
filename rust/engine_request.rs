//! Synthesis request admission and synchronous engine preparation.
//!
//! Pointer payloads are opaque here. Hosts admit their readable lifetimes,
//! copy asynchronous inputs, and serialize the engine's remaining resources.
//! Queue success transfers both commands together; failure returns ownership.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::async_command::{Character, Key, Mark, Parameter, Text, Wide};

#[derive(Clone, Copy)]
pub enum Request {
    Text(Text),
    Mark(Mark),
    Key(Key),
    Character(Character),
    Parameter(Parameter),
    Punctuation(*const Wide),
}
pub trait AdmissionHost {
    type Command;
    fn asynchronous(&self) -> bool;
    fn mode(&self) -> i32;
    fn publish_identifier(&mut self, id: u32);
    fn synchronous(&mut self, request: Request) -> u32;
    fn create(&mut self, request: Request, size: usize) -> Option<(Self::Command, u32)>;
    fn terminated(&mut self, id: u32, user: *mut std::ffi::c_void) -> Option<Self::Command>;
    fn enqueue_pair(
        &mut self,
        first: Self::Command,
        second: Self::Command,
    ) -> Result<(), (u32, Self::Command, Self::Command)>;
    fn enqueue(
        &mut self,
        command: Option<Self::Command>,
    ) -> Result<(), (u32, Option<Self::Command>)>;
    fn delete(&mut self, command: Option<Self::Command>);
}
pub fn submit(host: &mut impl AdmissionHost, mut request: Request, size: usize) -> u32 {
    let user = match &mut request {
        Request::Text(args) => {
            args.id = 0;
            Some(args.user)
        }
        Request::Mark(args) => {
            args.id = 0;
            Some(args.user)
        }
        _ => None,
    };
    if user.is_some() {
        host.publish_identifier(0);
    }
    if host.mode() & 1 != 0 || !host.asynchronous() {
        return host.synchronous(request);
    }
    let created = host.create(request, if user.is_some() { size } else { 0 });
    if let Some(user) = user {
        let id = created.as_ref().map_or(0, |(_, id)| *id);
        if created.is_some() {
            host.publish_identifier(id);
        }
        // Preserve the API's notification creation even if text capture fails.
        let terminated = host.terminated(id, user);
        match (created, terminated) {
            (Some((first, _)), Some(second)) => match host.enqueue_pair(first, second) {
                Ok(()) => 0,
                Err((status, first, second)) => {
                    host.delete(Some(first));
                    host.delete(Some(second));
                    status
                }
            },
            (first, second) => {
                host.delete(first.map(|(command, _)| command));
                host.delete(second);
                12 // ENOMEM: one or both command allocations failed
            }
        }
    } else {
        // The legacy single-command API lets the queue reject a null command.
        match host.enqueue(created.map(|(command, _)| command)) {
            Ok(()) => 0,
            Err((status, command)) => {
                host.delete(command);
                status
            }
        }
    }
}

pub trait SynthesisHost {
    fn initialize_text(&mut self, flags: u32);
    fn identity(&mut self, id: u32, user: *mut std::ffi::c_void);
    fn save_parameters(&mut self);
    fn skip(&mut self, index: usize, position: u32);
    fn has_skips(&self) -> bool;
    fn skipping(&mut self);
    fn marker(&mut self, marker: *const std::ffi::c_char);
    fn end(&mut self, position: u32);
    fn synthesize(&mut self, id: u32, text: *const std::ffi::c_void, flags: u32) -> u32;
    fn playback(&self) -> bool;
    fn finish_audio(&mut self, flush: bool);
}
pub fn synthesize(host: &mut impl SynthesisHost, args: Text) -> u32 {
    host.initialize_text(args.flags);
    host.identity(args.id, args.user);
    host.save_parameters();
    if let Some(index) = match args.position_type {
        1 => Some(0), // POS_CHARACTER
        2 => Some(1), // POS_WORD
        3 => Some(2), // POS_SENTENCE
        _ => None,
    } {
        host.skip(index, args.position);
    }
    if host.has_skips() {
        host.skipping();
    }
    host.end(args.end);
    let status = host.synthesize(args.id, args.text, args.flags);
    if host.playback() {
        host.finish_audio(status == 0x10000eff);
    }
    status
}
pub fn synthesize_mark(host: &mut impl SynthesisHost, args: Mark) -> u32 {
    host.initialize_text(args.flags);
    host.identity(args.id, args.user);
    if !args.mark.is_null() {
        host.marker(args.mark);
        host.skipping();
    }
    host.end(args.end);
    host.synthesize(args.id, args.text, args.flags | 0x10)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;
    struct Prepared {
        skips: [u32; 3],
        skipped: bool,
        saved: bool,
        marker: bool,
        status: u32,
        audio: Vec<bool>,
        end: u32,
        flags: u32,
    }
    impl SynthesisHost for Prepared {
        fn initialize_text(&mut self, _: u32) {
            self.skips = [5, 0, 0];
        }
        fn identity(&mut self, _: u32, _: *mut std::ffi::c_void) {}
        fn save_parameters(&mut self) {
            self.saved = true;
        }
        fn skip(&mut self, index: usize, value: u32) {
            self.skips[index] = value;
        }
        fn has_skips(&self) -> bool {
            self.skips.iter().any(|&v| v != 0)
        }
        fn skipping(&mut self) {
            self.skipped = true;
        }
        fn marker(&mut self, _: *const std::ffi::c_char) {
            self.marker = true;
        }
        fn end(&mut self, position: u32) {
            self.end = position;
        }
        fn synthesize(&mut self, _: u32, _: *const std::ffi::c_void, flags: u32) -> u32 {
            self.flags = flags;
            self.status
        }
        fn playback(&self) -> bool {
            true
        }
        fn finish_audio(&mut self, flush: bool) {
            self.audio.push(flush);
        }
    }
    fn prepared() -> Prepared {
        Prepared {
            skips: [0; 3],
            skipped: false,
            saved: false,
            marker: false,
            status: 0,
            audio: Vec::new(),
            end: 0,
            flags: 0,
        }
    }
    #[test]
    fn preparation_uses_post_init_skips_and_flushes_only_stopped_text() {
        for status in [0, 0x10000eff, 0x100001ff] {
            let mut host = prepared();
            host.status = status;
            let Request::Text(mut args) = text() else {
                unreachable!()
            };
            args.position_type = 99; // Leave the skips set by initialization.
            assert_eq!(synthesize(&mut host, args), status);
            assert_eq!(host.skips, [5, 0, 0]);
            assert!(host.skipped && host.saved);
            assert_eq!(host.end, args.end);
            assert_eq!(host.audio, [status == 0x10000eff]);
        }
    }
    #[test]
    fn mark_preserves_parameters_and_enables_ssml_without_audio_finish() {
        for mark in [ptr::null(), c"".as_ptr()] {
            let mut host = prepared();
            host.status = 0x10000eff;
            let args = Mark {
                id: 8,
                text: ptr::null_mut(),
                mark,
                end: u32::MAX,
                flags: 3,
                user: ptr::null_mut(),
            };
            assert_eq!(synthesize_mark(&mut host, args), host.status);
            assert_eq!(host.marker, !mark.is_null());
            assert_eq!(host.skipped, !mark.is_null());
            assert!(!host.saved && host.audio.is_empty());
            assert_eq!(host.flags, 0x13);
            assert_eq!(host.end, u32::MAX);
        }
    }
    #[derive(Default)]
    struct Spy {
        async_mode: bool,
        mode: i32,
        fail: u32,
        status: u32,
        published: u32,
        calls: Vec<(u32, u32)>,
    }
    impl AdmissionHost for Spy {
        type Command = u32;
        fn asynchronous(&self) -> bool {
            self.async_mode
        }
        fn mode(&self) -> i32 {
            self.mode
        }
        fn publish_identifier(&mut self, id: u32) {
            self.published = id;
            self.calls.push((0, id));
        }
        fn synchronous(&mut self, request: Request) -> u32 {
            if let Request::Text(args) = request {
                assert_eq!(args.id, 0);
            }
            self.calls.push((1, self.published));
            self.status
        }
        fn create(&mut self, _: Request, _: usize) -> Option<(u32, u32)> {
            self.calls.push((2, self.published));
            (self.fail & 1 == 0).then_some((10, 42))
        }
        fn terminated(&mut self, id: u32, _: *mut std::ffi::c_void) -> Option<u32> {
            assert_eq!(id, self.published);
            self.calls.push((3, id));
            (self.fail & 2 == 0).then_some(11)
        }
        fn enqueue_pair(&mut self, a: u32, b: u32) -> Result<(), (u32, u32, u32)> {
            self.calls.push((4, self.published));
            if self.status == 0 {
                Ok(())
            } else {
                Err((self.status, a, b))
            }
        }
        fn enqueue(&mut self, a: Option<u32>) -> Result<(), (u32, Option<u32>)> {
            self.calls.push((5, a.unwrap_or(0)));
            if self.status == 0 {
                Ok(())
            } else {
                Err((self.status, a))
            }
        }
        fn delete(&mut self, a: Option<u32>) {
            self.calls.push((6, a.unwrap_or(0)));
        }
    }
    fn text() -> Request {
        Request::Text(Text {
            id: 99,
            text: ptr::null_mut(),
            position: 7,
            position_type: 2,
            end: 8,
            flags: 9,
            user: ptr::null_mut(),
        })
    }
    #[test]
    fn paired_admission_publishes_before_callbacks_and_reclaims_only_failure() {
        for fail in 0..4 {
            for status in [0, 0x100001ff] {
                let mut spy = Spy {
                    async_mode: true,
                    fail,
                    status,
                    published: 999,
                    ..Spy::default()
                };
                let result = submit(&mut spy, text(), 80);
                assert_eq!(result, if fail == 0 { status } else { 12 });
                assert_eq!(spy.published, if fail & 1 == 0 { 42 } else { 0 });
                let deleted: Vec<_> = spy
                    .calls
                    .iter()
                    .filter(|&&(op, _)| op == 6)
                    .copied()
                    .collect();
                if fail == 0 && status == 0 {
                    assert!(deleted.is_empty());
                } else {
                    assert_eq!(
                        deleted,
                        [
                            (6, if fail & 1 == 0 { 10 } else { 0 }),
                            (6, if fail & 2 == 0 { 11 } else { 0 })
                        ]
                    );
                }
            }
        }
    }
    #[test]
    fn synchronous_modes_bypass_capture_and_single_failure_returns_ownership() {
        for (async_mode, mode) in [(false, 0), (true, 1), (true, 3)] {
            let mut spy = Spy {
                async_mode,
                mode,
                status: 123,
                published: 999,
                ..Spy::default()
            };
            assert_eq!(submit(&mut spy, text(), 0), 123);
            assert_eq!(spy.calls, [(0, 0), (1, 0)]);
        }
        let mut spy = Spy {
            async_mode: true,
            fail: 1,
            status: 22,
            published: 777,
            ..Spy::default()
        };
        assert_eq!(
            submit(
                &mut spy,
                Request::Parameter(Parameter {
                    parameter: 3,
                    value: 5,
                    relative: 1
                }),
                0
            ),
            22
        );
        assert_eq!(spy.calls, [(2, 777), (5, 0), (6, 0)]);
        assert_eq!(spy.published, 777);
    }
}
