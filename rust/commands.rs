//! Synthesis command writers: pauses, pitch and amplitude envelopes, sampled
//! sounds and formant spectrum sequences (`DoPause`, `DoPitch`, `DoAmplitude`,
//! `EndPitch`, `EndAmplitude`, `StartSyllable`, `DoSample2`/`DoSample3`,
//! `DoSpect2`).
//!
//! [`State`] owns what these writers share: the pending pitch and amplitude
//! commands and their accumulated lengths, the last spectrum frame and command,
//! the syllable marks used for smoothing, the format amplitude and whether a
//! wave was mixed. The command queue, spectrum lookup, smoothing and frame
//! storage stay with the [`Host`]. Frames, envelopes and wave data travel as
//! the addresses the queue holds, so commands keep their legacy words.
// Copyright (C) 2005 to 2014 Jonathan Duddington, (C) 2015-2017 Reece H. Dunn;
// Rust adaptation (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{generate::FmtParams, phoneme_program::PhonemeData};

/// `N_SEQ_FRAMES`: frames in a spectrum sequence.
pub const N_SEQ_FRAMES: usize = 25;

pub const WCMD_KLATT: isize = 1;
pub const WCMD_KLATT2: isize = 2;
pub const WCMD_SPECT: isize = 3;
pub const WCMD_SPECT2: isize = 4;
pub const WCMD_PAUSE: isize = 5;
pub const WCMD_WAVE: isize = 6;
pub const WCMD_WAVE2: isize = 7;
pub const WCMD_AMPLITUDE: isize = 8;
pub const WCMD_PITCH: isize = 9;
pub const WCMD_FMT_AMPLITUDE: isize = 14;

const PH_VOWEL: u8 = 2;
const PH_LIQUID: u8 = 3;
const PH_NASAL: u8 = 8;
const LONG: u32 = 1 << 21;
const LENGTHEN: u16 = 0x08;

const FRFLAG_VOWEL_CENTRE: i16 = 0x02;
const FRFLAG_LEN_MOD: i16 = 0x04;
const FRFLAG_BREAK_LF: i16 = 0x08;
const FRFLAG_BREAK: i16 = 0x10;
const FRFLAG_MODULATE: i16 = 0x40;
const FRFLAG_DEFER_WAV: i16 = 0x80;
const FRFLAG_LEN_MOD2: i16 = 0x4000;

const DONT_LENGTHEN: i32 = 0x4;
const WAV: usize = 1;
const LENGTH_MOD: usize = 10;

/// State shared by the writers, kept across calls (the legacy statics).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct State {
    /// The last queued spectrum frame (`frame_t *`), or 0.
    pub last_frame: usize,
    pub last_pitch_cmd: i32,
    pub last_amp_cmd: i32,
    /// The last queued spectrum or wave command, or -1.
    pub last_wcmdq: i32,
    pub pitch_length: i32,
    pub amp_length: i32,
    pub syllable_start: i32,
    pub syllable_end: i32,
    pub syllable_centre: i32,
    pub fmt_amplitude: i32,
    /// A wave was mixed into the previous spectrum (`DoSpect2`'s static).
    pub wave_flag: i32,
}

/// Engine settings for one call.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Settings {
    pub samplerate: i32,
    pub pause_factor: i32,
    pub clause_pause_factor: i32,
    pub min_pause: u32,
    pub wav_factor: i32,
    pub lenmod_factor: i32,
    pub lenmod2_factor: i32,
    pub min_sample_len: i32,
    /// The voice uses the Klatt synthesizer.
    pub klatt: i32,
    /// `param[LOPT_LONG_VOWEL_THRESHOLD]`.
    pub long_vowel_threshold: i32,
    /// `param[LOPT_SONORANT_MIN]`.
    pub sonorant_min: i32,
    /// Address of the default falling envelope (`envelope_data[PITCHfall]`).
    pub fall_envelope: usize,
}

/// The layout of `frameref_t`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FrameRef {
    pub length: i16,
    pub flags: i16,
    pub frame: usize,
}

/// A frame's own length and flags, read when the legacy code reads them.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FrameInfo {
    pub length: u8,
    pub flags: i16,
}

/// The result of a spectrum lookup. Formant transitions inside it can request
/// pauses, which C issued during the lookup; they are applied right after it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Lookup {
    pub found: bool,
    pub count: usize,
    /// Modulation flags from transitions (`modn_flags`).
    pub modulation: i32,
    pub pauses: [i32; 4],
    pub n_pauses: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// A sample header past the wave data, or a sample too short to split.
    Wave,
    /// More frames or transition pauses than a lookup can return.
    Lookup,
}

/// The command queue, spectrum lookup, smoothing and frames.
pub trait Host {
    /// Writes `words[..count]` at the queue tail and advances it; returns the
    /// index written.
    fn push(&mut self, words: [isize; 4], count: usize) -> i32;
    fn tail(&mut self) -> i32;
    fn word(&mut self, index: i32, slot: usize) -> isize;
    fn patch(&mut self, index: i32, slot: usize, value: isize);
    /// Smooths the syllable's queued spectra; returns the new syllable start.
    fn smooth(&mut self, start: i32, end: i32, centre: i32) -> i32;
    /// Looks up the current phoneme's spectrum sequence into `frames`.
    fn lookup(
        &mut self,
        which: i32,
        fmt: &mut FmtParams,
        frames: &mut [FrameRef; N_SEQ_FRAMES],
    ) -> Lookup;
    fn frame(&mut self, frame: usize) -> FrameInfo;
    /// Copies `frame`, keeping the formants above F3 of `high` (a frame
    /// flagged to keep its high peaks); returns the copy.
    fn copy_high(&mut self, frame: usize, high: usize) -> usize;
    /// Resets the sequence length adjustment (`seq_len_adjust = 0`).
    fn clear_length_adjust(&mut self);
}

/// The phoneme and list fields `DoSpect2` reads.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SpectPhoneme {
    /// `this_ph->type`, `std_length` and `phflags`.
    pub kind: u8,
    pub std_length: u8,
    pub flags: u32,
    /// `plist->synthflags` and `length`.
    pub synthflags: u16,
    pub length: u32,
    /// `plist[-1].type`, read for a vowel start.
    pub prev_kind: u8,
}

/// The writers over one state, with their settings, wave data and host.
pub struct Commands<'a, H> {
    pub state: &'a mut State,
    pub settings: &'a Settings,
    /// Phoneme sound data (`wavefile_data`); sample commands hold addresses into it.
    pub wave: &'a [u8],
    pub host: &'a mut H,
}

/// `PauseLength`: a pause in mS after speed scaling.
pub fn pause_length(s: &Settings, pause: i32, control: i32) -> i32 {
    let factor = if control != 0 {
        s.wav_factor
    } else if pause >= 200 {
        s.clause_pause_factor
    } else {
        s.pause_factor
    };
    // limit how far pauses can be shortened
    ((pause.wrapping_mul(factor) / 256) as u32).max(s.min_pause) as i32
}

impl<H: Host> Commands<'_, H> {
    fn smooth(&mut self) {
        let s = &mut *self.state;
        s.syllable_start = self
            .host
            .smooth(s.syllable_start, s.syllable_end, s.syllable_centre);
    }

    /// Fills in the pending amplitude command's length.
    pub fn end_amplitude(&mut self) {
        if self.state.amp_length > 0 {
            if self.host.word(self.state.last_amp_cmd, 1) == 0 {
                self.host
                    .patch(self.state.last_amp_cmd, 1, self.state.amp_length as isize);
            }
            self.state.amp_length = 0;
        }
    }

    /// Fills in the pending pitch command's length; a voice break also ends
    /// the syllable and smooths it.
    pub fn end_pitch(&mut self, voice_break: bool) {
        if self.state.pitch_length > 0 && self.state.last_pitch_cmd >= 0 {
            if self.host.word(self.state.last_pitch_cmd, 1) == 0 {
                self.host.patch(
                    self.state.last_pitch_cmd,
                    1,
                    self.state.pitch_length as isize,
                );
            }
            self.state.pitch_length = 0;
        }
        if voice_break {
            self.state.last_wcmdq = -1;
            self.state.last_frame = 0;
            self.state.syllable_end = self.host.tail();
            self.smooth();
            self.state.syllable_centre = -1;
        }
    }

    pub fn amplitude(&mut self, amp: i32, envelope: usize) {
        self.state.amp_length = 0; // the vowel length with this envelope
        let index = self
            .host
            .push([WCMD_AMPLITUDE, 0, envelope as isize, amp as isize], 4);
        self.state.last_amp_cmd = index;
    }

    pub fn pitch(&mut self, envelope: usize, pitch1: i32, pitch2: i32) {
        self.end_pitch(false);
        let (envelope, pitch1, pitch2) = if pitch1 == 255 {
            (self.settings.fall_envelope, 55, 76) // pitch was not set
        } else {
            (envelope, pitch1, pitch2)
        };
        self.state.pitch_length = 0; // the spectrum length with this envelope
        let words = [
            WCMD_PITCH,
            0,
            envelope as isize,
            (pitch1.wrapping_shl(16)).wrapping_add(pitch2.max(0)) as isize,
        ];
        self.state.last_pitch_cmd = self.host.push(words, 4);
    }

    /// `length` in nominal mS; `control` 1 shortens less at fast speeds.
    pub fn pause(&mut self, length: i32, control: i32) {
        let s = self.settings;
        let len = if length == 0 {
            0
        } else {
            let len = pause_length(s, length, control) as u32;
            if len < 90000 {
                len.wrapping_mul(s.samplerate as u32) / 1000 // mS to samples
            } else {
                len.wrapping_mul((s.samplerate / 25) as u32) / 40 // avoid overflow
            }
        };
        self.end_pitch(true);
        self.host.push([WCMD_PAUSE, len as isize, 0, 0], 2);
        self.state.last_frame = 0;
        if self.state.fmt_amplitude != 0 {
            self.state.fmt_amplitude = 0;
            self.host.push([WCMD_FMT_AMPLITUDE, 0, 0, 0], 2);
        }
    }

    /// Starts a syllable, if not already started.
    pub fn start_syllable(&mut self) {
        if self.state.syllable_end == self.state.syllable_start {
            self.state.syllable_end = self.host.tail();
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn sample2(
        &mut self,
        index: i32,
        which: i32,
        std_length: i32,
        control: i32,
        length_mod: i32,
        amp: i32,
    ) -> Result<i32, Error> {
        let s = self.settings;
        let index = (index & 0x7fffff) as usize;
        let header = self.wave.get(index..index + 3).ok_or(Error::Wave)?;
        let wav_scale = i32::from(header[2]);
        let mut wav_length = i32::from(header[1]) * 256 + i32::from(header[0]); // bytes
        if wav_length == 0 {
            return Ok(0);
        }
        let mut min_length = s.min_sample_len;
        if wav_scale == 0 {
            min_length = min_length.wrapping_mul(2); // 16 bit samples
        }
        let mut std_length = std_length;
        if std_length > 0 {
            std_length = std_length.wrapping_mul(s.samplerate) / 1000;
            if wav_scale == 0 {
                std_length = std_length.wrapping_mul(2);
            }
            let x = min_length.wrapping_mul(std_length) / wav_length;
            min_length = min_length.max(x);
        } else {
            std_length = wav_length; // the stored sound's length
        }
        if length_mod > 0 {
            std_length = std_length.wrapping_mul(length_mod) / 256;
        }
        let mut length = std_length.wrapping_mul(s.wav_factor) / 256;
        if control & DONT_LENGTHEN != 0 && length > std_length {
            length = std_length; // short noise bursts of stops keep their length
        }
        length = length.max(min_length);
        if wav_scale == 0 {
            length /= 2; // 16 bit samples
            wav_length /= 2;
        }
        if amp < 0 {
            return Ok(length);
        }

        let len4 = wav_length / 4;
        let index = index as isize + 4;
        let base = self.wave.as_ptr() as isize;
        let at = |offset: i32| base.wrapping_add(index).wrapping_add(offset as isize);
        let amp_word = wav_scale.wrapping_add(amp.wrapping_shl(8)) as isize;

        if which & 0x100 != 0 {
            // mix with the synthesized wave
            let words = [
                WCMD_WAVE2,
                (length | wav_length.wrapping_shl(16)) as isize,
                at(0),
                amp_word,
            ];
            self.state.last_wcmdq = self.host.push(words, 4);
            return Ok(length);
        }

        let first;
        if length > wav_length {
            first = len4 * 3;
            length -= first;
        } else {
            first = length;
            length = 0;
        }
        self.state.last_wcmdq = self
            .host
            .push([WCMD_WAVE, first as isize, at(0), amp_word], 4);
        while length > len4 * 3 {
            // C never ends here for a sample of fewer than four units
            if len4 == 0 {
                return Err(Error::Wave);
            }
            let x = if wav_scale == 0 { len4 * 2 } else { len4 };
            self.state.last_wcmdq = self
                .host
                .push([WCMD_WAVE, (len4 * 2) as isize, at(x), amp_word], 4);
            length -= len4 * 2;
        }
        if length > 0 {
            let mut x = wav_length - length;
            if wav_scale == 0 {
                x *= 2;
            }
            self.state.last_wcmdq = self
                .host
                .push([WCMD_WAVE, length as isize, at(x), amp_word], 4);
        }
        Ok(length)
    }

    /// Queues a phoneme program's sampled sound; an `amp` of -1 only measures
    /// it. Returns the length in samples.
    pub fn sample(&mut self, data: &PhonemeData, length_mod: i32, amp: i32) -> Result<i32, Error> {
        self.end_pitch(true);
        let amp = if amp == -1 {
            amp
        } else {
            let amp = match data.sound_parameters[WAV] {
                0 => 100,
                amp => amp,
            };
            amp.wrapping_mul(32) / 100
        };
        self.host.clear_length_adjust();
        let len = if data.sound_addresses[WAV] == 0 {
            0
        } else {
            let std_length = data.parameters[LENGTH_MOD].wrapping_mul(2);
            self.sample2(
                data.sound_addresses[WAV],
                2,
                std_length,
                data.control,
                length_mod,
                amp,
            )?
        };
        self.state.last_frame = 0;
        Ok(len)
    }

    /// Queues a phoneme's spectrum sequence. `which` is 0 not a vowel, 1 a
    /// vowel's start, 2 its body and end; a `modulation` of -1 only measures.
    /// Returns the total length in samples.
    pub fn spect(
        &mut self,
        ph: &SpectPhoneme,
        which: i32,
        fmt: &mut FmtParams,
        mut modulation: i32,
    ) -> Result<i32, Error> {
        let s = self.settings;
        if fmt.fmt_addr == 0 {
            return Ok(0);
        }
        let mut length_mod = match ph.length as i32 {
            0 => 256,
            length => length,
        };
        let mut length_min = s.samplerate / 70; // more than one cycle at low pitch
        if which == 2
            && s.long_vowel_threshold > 0
            && (i32::from(ph.std_length) >= s.long_vowel_threshold
                || ph.synthflags & LENGTHEN != 0
                || ph.flags & LONG != 0)
        {
            length_min *= 2; // long vowels are longer
        }
        if which == 1
            && (ph.kind == PH_LIQUID || ph.prev_kind == PH_LIQUID || ph.prev_kind == PH_NASAL)
        {
            // limit shortening of sonorants before shortened vowels
            length_mod = length_mod.max(s.sonorant_min);
        }

        let mut frames = [FrameRef::default(); N_SEQ_FRAMES];
        let lookup = self.host.lookup(which, fmt, &mut frames);
        let pauses = lookup.pauses.get(..lookup.n_pauses).ok_or(Error::Lookup)?;
        for &pause in pauses {
            self.pause(pause, 0);
        }
        if !lookup.found {
            return Ok(0);
        }
        let n_frames = lookup.count;
        if n_frames > N_SEQ_FRAMES {
            return Err(Error::Lookup);
        }

        if fmt.fmt_amp != self.state.fmt_amplitude {
            // an amplitude adjustment for this sequence
            self.state.fmt_amplitude = fmt.fmt_amp;
            self.host
                .push([WCMD_FMT_AMPLITUDE, fmt.fmt_amp as isize, 0, 0], 2);
        }

        let mut frame1 = frames[0].frame;
        let klatt = s.klatt != 0;
        let mut wcmd_spect = if klatt { WCMD_KLATT } else { WCMD_SPECT };
        if fmt.wav_addr == 0 && self.state.wave_flag != 0 {
            // cancel a wave playing previously
            wcmd_spect = if klatt { WCMD_KLATT2 } else { WCMD_SPECT2 };
            self.state.wave_flag = 0;
        }

        if self.state.last_frame != 0 {
            let last = self.host.frame(self.state.last_frame);
            if (last.length < 2 || last.flags & FRFLAG_VOWEL_CENTRE != 0)
                && last.flags & FRFLAG_BREAK == 0
            {
                // the previous sequence ended zero-length: replace with this one's first
                self.host.patch(self.state.last_wcmdq, 3, frame1 as isize);
                if last.flags & FRFLAG_BREAK_LF != 0 {
                    // but keep its high peaks
                    let copy = self.host.copy_high(frame1, self.state.last_frame);
                    self.host.patch(self.state.last_wcmdq, 3, copy as isize);
                }
            }
        }

        if ph.kind == PH_VOWEL && which == 2 {
            self.smooth(); // the previous syllable
            self.state.syllable_centre = self.host.tail(); // the vowel's centre
        }

        let mut lengths = [0i32; N_SEQ_FRAMES];
        let mut length_sum = 0i32;
        for ix in 1..n_frames {
            let before = frames[ix - 1];
            let factor = if before.flags & FRFLAG_LEN_MOD != 0 {
                // reduce the effect of the length modifier
                (length_mod.wrapping_mul(256 - s.lenmod_factor) + 256 * s.lenmod_factor) / 256
            } else if before.flags & FRFLAG_LEN_MOD2 != 0 {
                // reduce it at the start of a vowel
                (length_mod.wrapping_mul(256 - s.lenmod2_factor) + 256 * s.lenmod2_factor) / 256
            } else {
                length_mod
            };
            let len = i32::from(before.length).wrapping_mul(s.samplerate) / 1000;
            let len = len.wrapping_mul(factor) / 256;
            length_sum = length_sum.wrapping_add(len);
            lengths[ix] = len;
        }
        if length_sum > 0 && length_sum < length_min {
            // more than one cycle at low pitch
            for len in &mut lengths[1..n_frames.max(1)] {
                *len = len.wrapping_mul(length_min) / length_sum;
            }
        }

        let mut total = 0;
        for (ix, &len) in lengths.iter().enumerate().take(n_frames).skip(1) {
            let frame2 = frames[ix].frame;
            if fmt.wav_addr != 0 && self.host.frame(frame1).flags & FRFLAG_DEFER_WAV == 0 {
                // a wave to play along with this synthesis
                self.host.clear_length_adjust();
                let amp = if fmt.wav_amp == 0 {
                    32
                } else {
                    fmt.wav_amp.wrapping_mul(32) / 100
                };
                self.sample2(fmt.wav_addr, which + 0x100, 0, fmt.fmt_control, 0, amp)?;
                self.state.wave_flag = 1;
                fmt.wav_addr = 0;
            }
            if modulation >= 0 {
                if self.host.frame(frame1).flags & FRFLAG_MODULATE != 0 {
                    modulation = 6;
                }
                if ix == n_frames - 1 && lookup.modulation & 0xf00 != 0 {
                    modulation |= lookup.modulation; // before or after a glottal stop
                }
            }
            self.state.pitch_length = self.state.pitch_length.wrapping_add(len);
            self.state.amp_length = self.state.amp_length.wrapping_add(len);
            if len == 0 {
                self.state.last_frame = 0;
                frame1 = frame2;
            } else {
                self.state.last_wcmdq = self.host.tail();
                if modulation >= 0 {
                    let words = [
                        wcmd_spect,
                        len.wrapping_add(modulation.wrapping_shl(16)) as isize,
                        frame1 as isize,
                        frame2 as isize,
                    ];
                    self.host.push(words, 4);
                }
                self.state.last_frame = frame2;
                frame1 = frame2;
                total += len;
            }
        }

        if which != 1 && self.state.fmt_amplitude != 0 {
            self.state.fmt_amplitude = 0;
            self.host.push([WCMD_FMT_AMPLITUDE, 0, 0, 0], 2);
        }
        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Queue {
        words: Vec<[isize; 4]>,
        smoothed: Vec<(i32, i32, i32)>,
    }
    impl Host for Queue {
        fn push(&mut self, words: [isize; 4], count: usize) -> i32 {
            let mut entry = [0; 4];
            entry[..count].copy_from_slice(&words[..count]);
            self.words.push(entry);
            self.words.len() as i32 - 1
        }
        fn tail(&mut self) -> i32 {
            self.words.len() as i32
        }
        fn word(&mut self, index: i32, slot: usize) -> isize {
            self.words[index as usize][slot]
        }
        fn patch(&mut self, index: i32, slot: usize, value: isize) {
            self.words[index as usize][slot] = value;
        }
        fn smooth(&mut self, start: i32, end: i32, centre: i32) -> i32 {
            self.smoothed.push((start, end, centre));
            end
        }
        fn lookup(
            &mut self,
            _: i32,
            _: &mut FmtParams,
            frames: &mut [FrameRef; N_SEQ_FRAMES],
        ) -> Lookup {
            for (i, frame) in frames.iter_mut().take(3).enumerate() {
                *frame = FrameRef {
                    length: 20,
                    flags: 0,
                    frame: 0x1000 + i * 64,
                };
            }
            Lookup {
                found: true,
                count: 3,
                pauses: [10, 0, 0, 0],
                n_pauses: 1,
                ..Lookup::default()
            }
        }
        fn frame(&mut self, _: usize) -> FrameInfo {
            FrameInfo {
                length: 20,
                flags: 0,
            }
        }
        fn copy_high(&mut self, frame: usize, _: usize) -> usize {
            frame
        }
        fn clear_length_adjust(&mut self) {}
    }

    fn settings() -> Settings {
        Settings {
            samplerate: 22050,
            pause_factor: 256,
            clause_pause_factor: 256,
            wav_factor: 256,
            fall_envelope: 0xfa11,
            ..Settings::default()
        }
    }

    #[test]
    fn pitch_spectrum_and_lengths() {
        let mut state = State {
            last_pitch_cmd: -1,
            last_wcmdq: -1,
            syllable_centre: -1,
            ..State::default()
        };
        let mut queue = Queue::default();
        let settings = settings();
        let mut commands = Commands {
            state: &mut state,
            settings: &settings,
            wave: &[],
            host: &mut queue,
        };
        assert_eq!(pause_length(&settings, 300, 0), 300);
        commands.pitch(0x2222, 255, 9); // unset pitch: the default fall
        let vowel = SpectPhoneme {
            kind: PH_VOWEL,
            length: 256,
            ..SpectPhoneme::default()
        };
        let mut fmt = FmtParams {
            fmt_addr: 1,
            ..FmtParams::default()
        };
        // two 20 mS frames at 22050 Hz, after the transition's 10 mS pause
        assert_eq!(commands.spect(&vowel, 2, &mut fmt, 3), Ok(882));
        commands.end_pitch(false);
        // end_pitch filled in the spectra's length
        assert_eq!(queue.words[0], [WCMD_PITCH, 882, 0xfa11, (55 << 16) + 76]);
        assert_eq!(queue.words[1], [WCMD_PAUSE, 220, 0, 0]);
        assert_eq!(
            queue.words[2],
            [WCMD_SPECT, 441 + (3 << 16), 0x1000, 0x1040]
        );
        assert_eq!(
            queue.words[3],
            [WCMD_SPECT, 441 + (3 << 16), 0x1040, 0x1080]
        );
        assert_eq!(state.last_frame, 0x1080);
        assert_eq!(state.syllable_centre, 2);
        assert_eq!(state.pitch_length, 0);
    }

    #[test]
    fn short_samples_are_rejected() {
        let mut wave = [0u8; 16];
        wave[..3].copy_from_slice(&[2, 0, 1]); // two 8-bit units
        let settings = Settings {
            min_sample_len: 100,
            ..settings()
        };
        let mut state = State::default();
        let mut queue = Queue::default();
        let mut commands = Commands {
            state: &mut state,
            settings: &settings,
            wave: &wave,
            host: &mut queue,
        };
        let mut data = PhonemeData::default();
        data.sound_addresses[WAV] = 0;
        assert_eq!(commands.sample(&data, 0, 0), Ok(0)); // no sound
        data.sound_addresses[WAV] = 14;
        assert_eq!(commands.sample(&data, 0, 0), Err(Error::Wave)); // header past the data
        data.sound_addresses[WAV] = 0x800000; // masked to 0: two units, never split
        assert_eq!(commands.sample(&data, 0, 0), Err(Error::Wave));
    }
}
