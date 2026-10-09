//! Native audio dispatch and event admission on the host's completion path.
// SPDX-License-Identifier: GPL-3.0-or-later
pub const ASYNC: u32 = 1;
pub const AUDIO: u32 = 2;
pub const LATENCY: u32 = 4;
const AUDIO_ERROR: i32 = 0x100005ff;

pub trait Host {
    type Samples: Copy;
    type Event: Copy;
    fn capabilities(&self) -> u32;
    fn mode(&self) -> i32;
    fn command_enabled(&mut self) -> i32;
    fn voice_rate(&self) -> i32;
    fn output_rate(&self) -> i32;
    fn set_voice_rate(&mut self, rate: i32);
    fn set_output_rate(&mut self, rate: i32);
    fn set_error(&mut self, status: i32);
    fn close(&mut self);
    fn open(&mut self) -> i32;
    fn diagnostic(&mut self, operation: u32, error: i32);
    fn event_init(&mut self);
    fn has_samples(&self, samples: Self::Samples) -> bool;
    fn write(&mut self, samples: Self::Samples, bytes: usize) -> i32;
    fn callback(&mut self, samples: Self::Samples, length: i32, event: Option<Self::Event>);
    fn kind(&self, event: Self::Event) -> i32;
    fn event_length(&self, event: Self::Event) -> i32;
    fn event_rate(&self, event: Self::Event) -> i32;
    fn event_sample(&self, event: Self::Event) -> i32;
    fn has_audio(&self) -> bool;
    fn samples(&self) -> i128;
    fn latency(&mut self) -> i32;
    fn admit(&mut self, event: Self::Event, delay_ms: i32) -> u32;
    fn capacity(&self) -> usize;
    fn count(&self) -> i32;
    /// Fresh checked event projection; count zero dispatches one null event.
    fn event_at(&self, index: usize) -> Result<Option<Self::Event>, i32>;
}

/// Widen before subtraction/multiplication. Clamp the admitted host deadline;
/// invalid/nonpositive rates never divide or invent a long deferred wait.
pub fn delay_ms(queued: i32, after: i128, rate: i32) -> i32 {
    if rate <= 0 {
        return 0;
    }
    let played = if after > 0 {
        after.saturating_mul(1000) / i128::from(rate)
    } else {
        0
    };
    i128::from(queued)
        .saturating_sub(played)
        .clamp(0, i128::from(i32::MAX)) as i32
}

pub fn declare<H: Host>(host: &mut H, event: H::Event) -> u32 {
    let delay = if host.capabilities() & LATENCY != 0 && host.has_audio() && host.voice_rate() > 0 {
        let after = if host.kind(event) == 6 {
            0
        } else {
            host.samples()
                .saturating_sub(i128::from(host.event_sample(event)))
        };
        let latency = host.latency();
        delay_ms(latency, after, host.voice_rate())
    } else {
        0
    };
    host.admit(event, delay)
}

pub fn dispatch<H: Host>(
    host: &mut H,
    samples: H::Samples,
    length: i32,
    event: Option<H::Event>,
) -> i32 {
    if length < 0 {
        host.set_error(22);
        return -1;
    }
    let caps = host.capabilities();
    let enabled = if caps & ASYNC != 0 && host.mode() & 1 == 0 {
        host.command_enabled()
    } else {
        1
    };
    match host.mode() {
        2 | 3 => {
            if event.is_some_and(|event| host.kind(event) == 8) {
                host.set_voice_rate(host.event_rate(event.unwrap()));
                if host.output_rate() != host.voice_rate() {
                    if caps & AUDIO != 0 {
                        if host.output_rate() != 0 {
                            host.close();
                            host.set_output_rate(0);
                        }
                        let error = host.open();
                        if error != 0 {
                            host.diagnostic(1, error);
                            host.set_error(AUDIO_ERROR);
                            return -1;
                        }
                    }
                    host.set_output_rate(host.voice_rate());
                    if caps & ASYNC != 0 && host.mode() & 1 == 0 {
                        host.event_init();
                    }
                }
            }
            if caps & AUDIO != 0 {
                if host.output_rate() == 0 {
                    let error = host.open();
                    if error != 0 {
                        host.diagnostic(0, error);
                        host.set_error(AUDIO_ERROR);
                        return -1;
                    }
                    host.set_output_rate(host.voice_rate());
                }
                if host.has_samples(samples) && length != 0 && enabled != 0 {
                    let bytes = (length as usize)
                        .checked_mul(2)
                        .expect("positive i32 PCM extent fits usize");
                    let error = host.write(samples, bytes);
                    if error != 0 {
                        host.diagnostic(2, error);
                    }
                }
            }
            if caps & ASYNC != 0 && enabled != 0 {
                if let Some(event) = event {
                    if !(host.kind(event) == 1 && host.event_length(event) == 0)
                        && host.mode() & 1 == 0
                    {
                        let status = declare(host, event);
                        host.set_error(status as i32);
                    }
                }
            }
        }
        0 => host.callback(samples, length, event),
        _ => {}
    }
    i32::from(enabled == 0)
}

/// Send PCM once and visit the live admitted event prefix. The fixed admitted
/// capacity bounds callbacks that change the count; no unfilled tail is read.
pub fn create<H: Host>(host: &mut H, samples: H::Samples, mut length: i32) -> i32 {
    let bound = host.capacity();
    for index in 0..bound.max(1) {
        let event = match host.event_at(index) {
            Ok(event) => event,
            Err(status) => {
                host.set_error(status);
                return -1;
            }
        };
        let finished = dispatch(host, samples, length, event);
        length = 0;
        if finished != 0 {
            return finished;
        }
        let count = host.count();
        if count < 0 || count as usize > bound {
            host.set_error(22);
            return -1;
        }
        if index + 1 >= count as usize {
            return finished;
        }
    }
    host.set_error(22);
    -1
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Spy {
        capabilities: u32,
        mode: i32,
        rate: i32,
        output: i32,
        error: i32,
        count: i32,
        writes: Vec<usize>,
        admits: usize,
        grow: bool,
    }
    impl Host for Spy {
        type Samples = bool;
        type Event = usize;
        fn capabilities(&self) -> u32 {
            self.capabilities
        }
        fn mode(&self) -> i32 {
            self.mode
        }
        fn command_enabled(&mut self) -> i32 {
            1
        }
        fn voice_rate(&self) -> i32 {
            self.rate
        }
        fn output_rate(&self) -> i32 {
            self.output
        }
        fn set_voice_rate(&mut self, rate: i32) {
            self.rate = rate;
        }
        fn set_output_rate(&mut self, rate: i32) {
            self.output = rate;
        }
        fn set_error(&mut self, status: i32) {
            self.error = status;
        }
        fn close(&mut self) {}
        fn open(&mut self) -> i32 {
            0
        }
        fn diagnostic(&mut self, _: u32, _: i32) {}
        fn event_init(&mut self) {}
        fn has_samples(&self, samples: bool) -> bool {
            samples
        }
        fn write(&mut self, _: bool, bytes: usize) -> i32 {
            self.writes.push(bytes);
            -1
        }
        fn callback(&mut self, _: bool, _: i32, _: Option<usize>) {}
        fn kind(&self, _: usize) -> i32 {
            3
        }
        fn event_length(&self, _: usize) -> i32 {
            1
        }
        fn event_rate(&self, _: usize) -> i32 {
            22050
        }
        fn event_sample(&self, _: usize) -> i32 {
            0
        }
        fn has_audio(&self) -> bool {
            true
        }
        fn samples(&self) -> i128 {
            0
        }
        fn latency(&mut self) -> i32 {
            0
        }
        fn admit(&mut self, _: usize, _: i32) -> u32 {
            self.admits += 1;
            if self.grow {
                self.count += 1;
            }
            17
        }
        fn capacity(&self) -> usize {
            3
        }
        fn count(&self) -> i32 {
            self.count
        }
        fn event_at(&self, index: usize) -> Result<Option<usize>, i32> {
            if self.count > 3 {
                return Err(22);
            }
            Ok((self.count > 0).then_some(index))
        }
    }
    fn spy() -> Spy {
        Spy {
            capabilities: ASYNC | AUDIO,
            mode: 2,
            rate: 22050,
            output: 22050,
            error: 67,
            count: 3,
            writes: Vec::new(),
            admits: 0,
            grow: false,
        }
    }
    #[test]
    fn event_prefix_delivers_pcm_once_and_preserves_write_error_policy() {
        let mut host = spy();
        assert_eq!(create(&mut host, true, 8), 0);
        assert_eq!(host.writes, [16]);
        assert_eq!((host.admits, host.error), (3, 17));
        host.mode = 3;
        host.error = 67;
        assert_eq!(dispatch(&mut host, true, 8, Some(0)), 0);
        assert_eq!(host.error, 67); // Legacy write diagnostics do not replace status.
        for capabilities in [0, ASYNC, AUDIO] {
            let mut host = spy();
            host.capabilities = capabilities;
            assert_eq!(create(&mut host, true, 8), 0);
            assert_eq!(host.writes.len(), usize::from(capabilities & AUDIO != 0));
            assert_eq!(host.admits, if capabilities & ASYNC != 0 { 3 } else { 0 });
        }
    }
    #[test]
    fn callback_count_growth_cannot_visit_an_unfilled_tail() {
        let mut host = spy();
        host.count = 1;
        host.grow = true;
        assert_eq!(create(&mut host, true, 8), -1);
        assert_eq!(host.writes, [16]);
        assert_eq!((host.admits, host.error), (3, 22));
    }
    #[test]
    fn extreme_sample_positions_clamp_without_overflow_or_division() {
        assert_eq!(delay_ms(777, 2205, 22050), 677);
        assert_eq!(delay_ms(i32::MAX, i128::MAX, 1), 0);
        assert_eq!(delay_ms(i32::MIN, i128::MAX, 1), 0);
        assert_eq!(delay_ms(i32::MAX, i128::MIN, 1), i32::MAX);
        assert_eq!(delay_ms(500, 1, 0), 0);
        assert_eq!(delay_ms(500, 1, -1), 0);
        assert_eq!(delay_ms(-1, 0, 22050), 0);
    }
}
