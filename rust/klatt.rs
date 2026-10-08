//! Owned Klatt cascade/parallel synthesizer. Work is bounded by the output
//! buffer and runs within the engine's proactor-driven synthesis step.
//! No threads, waits, or per-sample allocations belong to this DSP kernel.
// Copyright (C) 2008 Jonathan Duddington, 2013-2016 Reece H. Dunn;
// based on Jon Iles and Nick Ing-Simmons' Klatt implementation (1993-94).
// Rust adaptation (C) 2026. SPDX-License-Identifier: GPL-3.0-or-later
use crate::{formant::Frame, klatt_data::*, voice::Voice, wavegen::WgenData};
use std::f64::consts::PI;

/// The owner's serialized queue, resident bytes, random stream and PCM sink.
/// The sink includes echo feedback. `next_spectrum` inspects only the admitted
/// queue, stopping at the first waveform or pause; `byte` reads resident data.
pub trait Host {
    fn room(&self) -> isize;
    fn emit(&mut self, sample: i32);
    fn random(&mut self) -> i32;
    fn byte(&self, address: usize, offset: i32) -> u8;
    fn next_spectrum(&self) -> Option<Frame>;
    fn reset_speechplayer(&mut self);
}

#[derive(Clone, Copy, Debug, Default)]
struct Resonator {
    a: f64,
    b: f64,
    c: f64,
    p1: f64,
    p2: f64,
}
impl Resonator {
    fn filter(&mut self, input: f64, zero: bool) -> f64 {
        let result = self.a * input + self.b * self.p1 + self.c * self.p2;
        self.p2 = self.p1;
        self.p1 = if zero { input } else { result };
        result
    }
    fn coefficients(&mut self, frequency: i32, width: i32, minus_pi: f64, zero: bool) {
        let r = (minus_pi * f64::from(width)).exp();
        self.c = -(r * r);
        let f = if zero { -frequency } else { frequency };
        self.b = r * (-2.0 * minus_pi * f64::from(f)).cos() * 2.0;
        self.a = 1.0 - self.b - self.c;
        if zero && self.a != 0.0 {
            self.a = 1.0 / self.a;
            self.c *= -self.a;
            self.b *= -self.a;
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Peak {
    frequency: f64,
    bandwidth: f64,
    amplitude: f64,
    parallel_width: f64,
    frequency_inc: f64,
    bandwidth_inc: f64,
    amplitude_inc: f64,
    parallel_width_inc: f64,
}

/// All filter and excitation history belongs to this instance. `initialize`
/// keeps the legacy persistent source histories across engine initialization;
/// constructing a new instance starts those histories independently at zero.
#[derive(Clone, Debug)]
pub struct Klatt {
    resonators: [Resonator; 20],
    peaks: [Peak; 9],
    parameters: [f64; 10],
    increments: [f64; 10],
    frequency: [i32; 10],
    bandwidth: [i32; 10],
    parallel_amp: [i32; 10],
    parallel_bw: [i32; 10],
    previous: Frame,
    samples: i32,
    count: i32,
    end_wave: bool,
    source: usize,
    flutter: i32,
    flutter_time: i32,
    period: i32,
    nper: i32,
    nopen: i32,
    nmod: i32,
    sample_index: i32,
    f0: i32,
    original_f0: i32,
    voice_db: i32,
    skew: i32,
    frame_skew: i32,
    open: i32,
    tilt: i32,
    turbulence: i32,
    pulse_a: f64,
    pulse_b: f64,
    natural_wave: f64,
    noise_last: f64,
    noise: f64,
    random: i32,
    voice: f64,
    voice_last: f64,
    glottal_last: f64,
    decay: f64,
    onemd: f64,
    amp_voice: f64,
    amp_parallel: f64,
    amp_aspiration: f64,
    amp_frication: f64,
    amp_breath: f64,
    amp_bypass: f64,
    gain: f64,
    fadein: i32,
    fadeout: i32,
}

fn gain(db: i32) -> f64 {
    AMPLITUDES
        .get(db as usize)
        .map_or(0.0, |&v| f64::from(v) * 0.001)
}
const MINUS_PI: f64 = -PI / 22050.0;

impl Default for Klatt {
    fn default() -> Self {
        Self::new()
    }
}
impl Klatt {
    pub fn new() -> Self {
        let mut state = Self {
            resonators: [Resonator {
                a: 0.0,
                b: 0.0,
                c: 0.0,
                p1: 0.0,
                p2: 0.0,
            }; 20],
            peaks: [Peak {
                frequency: 0.0,
                bandwidth: 0.0,
                amplitude: 0.0,
                parallel_width: 0.0,
                frequency_inc: 0.0,
                bandwidth_inc: 0.0,
                amplitude_inc: 0.0,
                parallel_width_inc: 0.0,
            }; 9],
            parameters: [0.0; 10],
            increments: [0.0; 10],
            frequency: [0; 10],
            bandwidth: [0; 10],
            parallel_amp: [0; 10],
            parallel_bw: [0; 10],
            previous: Frame::ZERO,
            samples: 0,
            count: 0,
            end_wave: false,
            source: 1,
            flutter: 0,
            flutter_time: 0,
            period: 0,
            nper: 0,
            nopen: 0,
            nmod: 0,
            sample_index: 0,
            f0: 0,
            original_f0: 0,
            voice_db: 0,
            skew: 0,
            frame_skew: 0,
            open: 0,
            tilt: 0,
            turbulence: 0,
            pulse_a: 0.0,
            pulse_b: 0.0,
            natural_wave: 0.0,
            noise_last: 0.0,
            noise: 0.0,
            random: 0,
            voice: 0.0,
            voice_last: 0.0,
            glottal_last: 0.0,
            decay: 0.0,
            onemd: 0.0,
            amp_voice: 0.0,
            amp_parallel: 0.0,
            amp_aspiration: 0.0,
            amp_frication: 0.0,
            amp_breath: 0.0,
            amp_bypass: 0.0,
            gain: 0.0,
            fadein: 0,
            fadeout: 0,
        };
        state.initialize();
        state
    }

    pub fn initialize(&mut self) {
        self.count = 0;
        self.source = 1;
        self.flutter = 20;
        self.reset(2);
        self.frequency = [280, 688, 1064, 2806, 3260, 3700, 6500, 7000, 8000, 280];
        self.bandwidth = [89, 160, 70, 160, 200, 200, 500, 500, 500, 89];
        self.parallel_amp = [0, 59, 59, 59, 59, 59, 59, 0, 0, 0];
        self.parallel_bw = [59, 59, 89, 149, 200, 200, 500, 0, 0, 0];
        self.f0 = 1000;
        self.open = 40;
        self.voice_db = 52;
        self.frame_skew = 0;
        self.tilt = 0;
        self.turbulence = 0;
    }

    pub fn reset(&mut self, control: i32) {
        if control == 2 {
            self.resonators[18].coefficients(
                950 * 22050 / 10000,
                630 * 22050 / 10000,
                MINUS_PI,
                false,
            );
        }
        if control > 0 {
            self.nper = 0;
            self.period = 0;
            self.nopen = 0;
            self.nmod = 0;
            for r in &mut self.resonators[17..] {
                r.p1 = 0.0;
                r.p2 = 0.0;
            }
        }
        for r in &mut self.resonators[..=16] {
            r.p1 = 0.0;
            r.p2 = 0.0;
        }
    }

    fn setup(
        &mut self,
        host: &mut impl Host,
        length: i32,
        first: &Frame,
        last: &Frame,
        voice: &Voice,
    ) {
        if (1..=5).contains(&voice.klatt[0]) {
            self.source = voice.klatt[0] as usize;
        }
        self.flutter = voice.flutter / 32;
        self.end_wave = host
            .next_spectrum()
            .is_none_or(|next| next.frequencies[1..6] != last.frequencies[1..6]);
        if self.previous.frequencies[1..6] != first.frequencies[1..6] {
            host.reset_speechplayer();
            self.reset(0);
        }
        self.previous = *last;
        for i in 0..10 {
            if i >= 5 || first.flags & 1 == 0 {
                self.parameters[i] = 0.0;
                self.increments[i] = 0.0;
            } else {
                self.parameters[i] = f64::from(first.klatt[i]);
                self.increments[i] =
                    f64::from((i32::from(last.klatt[i]) - i32::from(first.klatt[i])) * 64)
                        / f64::from(length);
            }
        }
        self.samples = length;
        for i in 1..6 {
            let p = &mut self.peaks[i];
            p.frequency =
                f64::from(i32::from(first.frequencies[i]) * i32::from(voice.frequency[i])) / 256.0
                    + f64::from(voice.frequency_add[i]);
            let next = f64::from(i32::from(last.frequencies[i]) * i32::from(voice.frequency[i]))
                / 256.0
                + f64::from(voice.frequency_add[i]);
            p.frequency_inc = ((next - p.frequency) * 64.0) / f64::from(length);
            if i < 4 {
                p.bandwidth =
                    f64::from(first.bandwidths[i]) * 2.0 * (f64::from(voice.width[i]) / 256.0);
                p.bandwidth_inc = ((f64::from(last.bandwidths[i]) * 2.0 - p.bandwidth) * 64.0)
                    / f64::from(length);
            }
        }
        let nasal = |frame: &Frame| {
            if frame.klatt[1] == 0 {
                f64::from(self.frequency[9])
            } else {
                f64::from(frame.klatt[1]) * 2.0
            }
        };
        self.peaks[0].frequency = nasal(first);
        self.peaks[0].frequency_inc = ((nasal(last) - nasal(first)) * 64.0) / f64::from(length);
        self.peaks[0].bandwidth = 89.0;
        self.peaks[0].bandwidth_inc = 0.0;
        if first.flags & 1 != 0 {
            for i in 1..7 {
                let p = &mut self.peaks[i];
                p.parallel_width = f64::from(first.parallel_bandwidths[i]) * 4.0;
                p.parallel_width_inc =
                    ((f64::from(last.parallel_bandwidths[i]) * 4.0 - p.parallel_width) * 64.0)
                        / f64::from(length);
                p.amplitude = f64::from(first.parallel_amplitudes[i]);
                p.amplitude_inc = ((f64::from(last.parallel_amplitudes[i]) - p.amplitude) * 64.0)
                    / f64::from(length);
            }
        }
    }

    fn frame_init(&mut self) {
        self.original_f0 = self.f0 / 10;
        self.voice_db = (self.parameters[0] as i32 - 7).max(0);
        self.amp_aspiration = gain(self.parameters[3] as i32) * 0.05;
        self.amp_frication = gain(self.parameters[7] as i32) * 0.25;
        self.amp_parallel = gain(self.parameters[6] as i32);
        self.amp_bypass = gain(self.parameters[8] as i32) * 0.05;
        self.gain = gain(59) / [45, 38, 45, 45, 55, 45][self.source] as f64;
        self.open = self.parameters[5] as i32;
        self.tilt = self.parameters[2] as i32;
        self.turbulence = self.parameters[9] as i32;
        self.frame_skew = self.parameters[4] as i32;
        for i in 1..=9 {
            self.resonators[i].coefficients(self.frequency[i], self.bandwidth[i], MINUS_PI, false);
        }
        self.resonators[0].coefficients(self.frequency[0], self.bandwidth[0], MINUS_PI, true);
        for (i, factor) in [0.6, 0.4, 0.15, 0.06, 0.04, 0.022, 0.03]
            .into_iter()
            .enumerate()
        {
            self.resonators[10 + i].coefficients(
                self.frequency[i],
                self.parallel_bw[i],
                MINUS_PI,
                false,
            );
            self.resonators[10 + i].a *= gain(self.parallel_amp[i]) * factor;
        }
        self.resonators[19].coefficients(0, 22050 / 2, MINUS_PI, false);
    }

    fn pitch_reset(&mut self) {
        if self.f0 > 0 {
            self.period = 40 * 22050 / self.f0;
            self.amp_voice = gain(self.voice_db);
            self.nmod = self.period;
            if self.voice_db > 0 {
                self.nmod >>= 1;
            }
            self.amp_breath = gain(self.turbulence) * 0.1;
            self.nopen = 4 * self.open;
            if self.source == 1 && self.nopen > 263 {
                self.nopen = 263;
            }
            if self.nopen >= self.period - 1 {
                self.nopen = self.period - 2;
            }
            self.nopen = self.nopen.max(40);
            // The compiled parameter range has an open phase in 40..=263.
            // Keep malformed caller frames bounded rather than indexing past B0.
            self.pulse_b = B0
                .get((self.nopen - 40) as usize)
                .copied()
                .map_or(0.0, f64::from);
            self.pulse_a = (self.pulse_b * f64::from(self.nopen)) * 0.333;
            self.resonators[17].coefficients(0, 22050 / self.nopen, MINUS_PI, false);
            let scale = f64::from(self.nopen) * 0.00833;
            self.resonators[17].a *= scale * scale;
            self.frame_skew = self.frame_skew.min(self.period - self.nopen);
            self.skew = if self.skew >= 0 {
                self.frame_skew
            } else {
                -self.frame_skew
            };
            self.period += self.skew;
            self.skew = -self.skew;
        } else {
            self.period = 4;
            self.amp_voice = 0.0;
            self.nmod = self.period;
            self.amp_breath = 0.0;
            self.pulse_a = 0.0;
            self.pulse_b = 0.0;
        }
        if self.period != 4 || self.sample_index == 0 {
            self.decay = 0.033 * f64::from(self.tilt);
            self.onemd = if self.decay > 0.0 {
                1.0 - self.decay
            } else {
                1.0
            };
        }
    }

    fn excitation(&mut self) -> f64 {
        match self.source {
            1 => {
                let input = [0.0, 13000000.0, -13000000.0]
                    .get(self.nper as usize)
                    .copied()
                    .unwrap_or(0.0);
                self.resonators[17].filter(input, false)
            }
            2 => {
                if self.nper < self.nopen {
                    self.pulse_a -= self.pulse_b;
                    self.natural_wave += self.pulse_a;
                    self.natural_wave * 0.028
                } else {
                    self.natural_wave = 0.0;
                    0.0
                }
            }
            3 | 4 => {
                if self.period == 0 {
                    return 0.0;
                }
                let samples: &[i16] = if self.source == 3 {
                    &NATURAL_SAMPLES
                } else {
                    &NATURAL_SAMPLES2
                };
                let position =
                    (f64::from(self.nper) / f64::from(self.period)) * samples.len() as f64;
                let index = position as usize;
                let first = f64::from(samples[index % samples.len()]);
                let last = f64::from(samples[(index + 1) % samples.len()]);
                (first + (last - first) * (position - index as f64)) * 3.0
            }
            _ => self.voice,
        }
    }

    fn parwave(&mut self, host: &mut impl Host, data: &mut WgenData, count: i32) -> bool {
        let flutter = (f64::from(self.flutter) / 50.0)
            * (f64::from(self.original_f0) / 100.0)
            * ((PI * 12.7 * f64::from(self.flutter_time)).sin()
                + (PI * 7.1 * f64::from(self.flutter_time)).sin()
                + (PI * 4.7 * f64::from(self.flutter_time)).sin())
            * 10.0;
        self.f0 = self.f0.wrapping_add(flutter as i32);
        self.flutter_time = self.flutter_time.wrapping_add(1);
        for ns in 0..count {
            self.sample_index = ns;
            self.random = host.random();
            self.noise = f64::from(self.random) + 0.75 * self.noise_last;
            self.noise_last = self.noise;
            if self.nper > self.nmod {
                self.noise *= 0.5;
            }
            let frication = self.amp_frication * self.noise;
            for _ in 0..4 {
                self.voice = self.excitation();
                if self.nper >= self.period {
                    self.nper = 0;
                    self.pitch_reset();
                }
                self.voice = self.resonators[18].filter(self.voice, false);
                self.nper += 1;
            }
            if self.source == 5 {
                self.voice = ((f64::from(self.nper) / f64::from(self.period)) * 2.0 - 1.0) * 6000.0;
            }
            self.voice = self.voice * self.onemd + self.voice_last * self.decay;
            self.voice_last = self.voice;
            if self.nper < self.nopen {
                self.voice += self.amp_breath * f64::from(self.random);
            }
            let glottal = self.amp_voice * self.voice + self.amp_aspiration * self.noise;
            let parallel = self.amp_parallel * self.voice + self.amp_aspiration * self.noise;
            let mut out = self.resonators[0].filter(glottal, true);
            for i in (1..=9).rev() {
                out = self.resonators[i].filter(out, false);
            }
            out += self.resonators[11].filter(parallel, false);
            out += self.resonators[10].filter(parallel, false);
            let source = frication + parallel - self.glottal_last;
            self.glottal_last = parallel;
            for i in 12..=16 {
                out = self.resonators[i].filter(source, false) - out;
            }
            out = self.amp_bypass * source - out;
            out = self.resonators[19].filter(out, false);
            // Rust defines this cast for an unstable/malformed frame too:
            // saturation outside i32 and zero for NaN. C left it undefined.
            let mut sample = f64::from((out * f64::from(data.amplitude) * self.gain) as i32);
            if data.mix_wavefile_ix < data.n_mix_wavefile {
                let offset = data.mix_wavefile_ix.wrapping_add(data.mix_wavefile_offset);
                let mixed = if data.mix_wave_scale == 0 {
                    let lo = host.byte(data.mix_wavefile, offset);
                    let hi = host.byte(data.mix_wavefile, offset.wrapping_add(1));
                    data.mix_wavefile_ix += 2;
                    i32::from(i16::from_le_bytes([lo, hi]))
                } else {
                    data.mix_wavefile_ix += 1;
                    i32::from(host.byte(data.mix_wavefile, offset) as i8) * data.mix_wave_scale
                };
                let scaled = mixed.wrapping_mul(data.amplitude_v) / 1024;
                sample += f64::from(scaled.wrapping_mul(data.mix_wave_amp) / 40);
                if data.mix_wavefile_ix.wrapping_add(data.mix_wavefile_offset)
                    >= data.mix_wavefile_max
                {
                    data.mix_wavefile_offset -= (data.mix_wavefile_max * 3) / 4;
                }
            }
            if self.fadein < 64 {
                sample = sample * f64::from(self.fadein) / 64.0;
                self.fadein += 1;
            }
            if self.fadeout > 0 {
                self.fadeout -= 1;
                sample = sample * f64::from(self.fadeout) / 64.0;
                if self.fadeout == 0 {
                    self.fadein = 0;
                }
            }
            host.emit(sample as i32);
            self.count += 1;
            if host.room() < 2 {
                return true;
            }
        }
        false
    }

    /// Returns true when the buffer filled; the owner later calls with
    /// `resume=true`. `length` is the admitted command's sample count.
    /// Klatt retains C's 64-sample parameter advancement at each resume.
    /// A call without room for a sample leaves the command unadmitted; retry
    /// it with the same `resume` value after providing output capacity.
    #[allow(clippy::too_many_arguments)]
    pub fn fill(
        &mut self,
        host: &mut impl Host,
        length: i32,
        resume: bool,
        first: &Frame,
        last: &Frame,
        data: &mut WgenData,
        voice: &Voice,
    ) -> bool {
        if host.room() < 2 || length <= 0 {
            return host.room() < 2;
        }
        if !resume {
            self.setup(host, length, first, last, voice);
            self.count = 0;
        }
        while self.count < self.samples {
            self.f0 = data.pitch.wrapping_mul(10) / 4096;
            for i in 0..6 {
                self.frequency[i] = self.peaks[i].frequency as i32;
                if i < 4 {
                    self.bandwidth[i] = self.peaks[i].bandwidth as i32;
                }
            }
            for i in 1..7 {
                self.parallel_amp[i] = self.peaks[i].amplitude as i32;
            }
            // frame_init must see parameters before interpolation, as in C.
            self.frame_init();
            for p in &mut self.peaks {
                p.frequency += p.frequency_inc;
                p.bandwidth += p.bandwidth_inc;
                p.parallel_width += p.parallel_width_inc;
                p.amplitude += p.amplitude_inc;
            }
            for i in 0..10 {
                self.parameters[i] += self.increments[i];
            }
            data.pitch_ix = data.pitch_ix.wrapping_add(data.pitch_inc);
            let index = (data.pitch_ix >> 8).min(127);
            let pitch = i32::from(host.byte(data.pitch_env, index)).wrapping_mul(data.pitch_range);
            data.pitch = (pitch >> 8).wrapping_add(data.pitch_base);
            if self.parwave(host, data, (self.samples - self.count).min(64)) {
                return true;
            }
        }
        if self.end_wave {
            self.fadeout = 64;
            self.end_wave = false;
            self.count -= 64;
            if self.parwave(host, data, 64) {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Sink {
        samples: Vec<i32>,
        capacity: usize,
        resets: usize,
    }
    impl Host for Sink {
        fn room(&self) -> isize {
            (self.capacity - self.samples.len()) as isize * 2
        }
        fn emit(&mut self, sample: i32) {
            self.samples.push(sample);
        }
        fn random(&mut self) -> i32 {
            17
        }
        fn byte(&self, _: usize, _: i32) -> u8 {
            128
        }
        fn next_spectrum(&self) -> Option<Frame> {
            None
        }
        fn reset_speechplayer(&mut self) {
            self.resets += 1;
        }
    }
    fn fixtures() -> (Frame, Voice, WgenData) {
        let frame = Frame {
            flags: 1,
            frequencies: [280, 688, 1064, 2806, 3260, 3700, 6500],
            bandwidths: [89, 80, 35, 80],
            klatt: [59, 0, 0, 0, 0],
            ..Frame::ZERO
        };
        let voice = Voice {
            klatt: [1, 0, 0, 0, 0, 0, 0, 0],
            frequency: [256; 9],
            width: [256; 9],
            ..Voice::default()
        };
        let data = WgenData {
            pitch: 100 * 4096,
            pitch_base: 100 * 4096,
            amplitude: 60,
            ..WgenData::default()
        };
        (frame, voice, data)
    }
    #[test]
    fn independent_owners_keep_excitation_history_separate() {
        let (frame, voice, data) = fixtures();
        let mut first = Klatt::new();
        let mut other = Klatt::new();
        let mut sink = Sink {
            samples: Vec::with_capacity(200),
            capacity: 200,
            resets: 0,
        };
        let mut first_data = data;
        assert!(!first.fill(
            &mut sink,
            128,
            false,
            &frame,
            &frame,
            &mut first_data,
            &voice
        ));
        let expected = sink.samples.clone();
        assert!(expected.iter().any(|&s| s != 0));
        sink.samples.clear();
        first.fill(
            &mut sink,
            128,
            false,
            &frame,
            &frame,
            &mut first_data,
            &voice,
        );
        sink.samples.clear();
        let mut other_data = data;
        assert!(!other.fill(
            &mut sink,
            128,
            false,
            &frame,
            &frame,
            &mut other_data,
            &voice
        ));
        assert_eq!(sink.samples, expected);
        assert_eq!(other_data.pitch_ix, data.pitch_ix);
    }
    #[test]
    fn empty_output_preserves_command_and_owner_until_room_arrives() {
        let (frame, voice, mut data) = fixtures();
        let before = data;
        let mut state = Klatt::new();
        let mut sink = Sink {
            samples: Vec::new(),
            capacity: 0,
            resets: 0,
        };
        assert!(state.fill(&mut sink, 128, false, &frame, &frame, &mut data, &voice));
        assert_eq!(data, before);
        assert_eq!(sink.resets, 0);
        assert_eq!(state.count, 0);
        sink.capacity = 1;
        assert!(state.fill(&mut sink, 128, false, &frame, &frame, &mut data, &voice));
        assert_eq!(sink.samples.len(), 1);
        assert_eq!(state.count, 1);
    }
}
