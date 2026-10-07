//! Formant wave generator and command queue consumer (`wavegen.c`).
//!
//! [`Wavegen`] owns the generator's state: the voice copy, formant peaks and
//! their per-step increments, two harmonic spectra and the low-harmonic
//! interpolation, pitch flutter, automatic gain control, glottal and roughness
//! modulation, breath resonators, wave-cycle and segment counters, the
//! amplitude envelope, echo length and the queue consumer's resume state.
//!
//! The memory the other synthesizers (Klatt, speechPlayer, MBROLA) share with
//! it stays with the [`Host`]: the command queue, the echo ring, the output
//! buffer and the embedded command values, as do those synthesizers, events,
//! output hooks, sonic and the random generator. Queue words that hold
//! addresses (frames, envelopes, sample data, voices) are read through the
//! host, as C dereferenced them.
// Copyright (C) 2005 to 2013 Jonathan Duddington, (C) 2015-2016 Reece H. Dunn;
// Rust adaptation (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later
#[path = "wavegen_data.rs"]
mod data;

use crate::{formant::Frame, synthesis_parameters as parameters, voice::Voice};
use data::{
    EMBEDDED_DEFAULT, EMBEDDED_MAX, FLUTTER_TAB, MODULATION_TAB, PK_SHAPE1, PK_SHAPE2, SIN_TAB,
    WAVEMULT,
};

pub const N_PEAKS: usize = 9;
pub use crate::wave_memory::{N_ECHO_BUF, N_WCMDQ};
pub const N_EMBEDDED_VALUES: usize = 15;
const N_LOWHARM: usize = 30;
pub const MAX_HARMONIC: usize = 400;
const N_TONE_ADJUST: i32 = 1000;
const N_WAVEMULT: i32 = 128;
const N_FLUTTER: i32 = 0x170;
const N_ROUGHNESS: i32 = 8;
const STEPSIZE: i32 = 64;
const ENV_LEN: i32 = 128;
const MIN_PITCH: i32 = 102400; // 25 Hz << 12

const WCMD_KLATT: isize = 1;
const WCMD_KLATT2: isize = 2;
const WCMD_SPECT: isize = 3;
const WCMD_SPECT2: isize = 4;
const WCMD_PAUSE: isize = 5;
const WCMD_WAVE: isize = 6;
const WCMD_WAVE2: isize = 7;
const WCMD_AMPLITUDE: isize = 8;
const WCMD_PITCH: isize = 9;
const WCMD_MARKER: isize = 10;
const WCMD_VOICE: isize = 11;
const WCMD_EMBEDDED: isize = 12;
const WCMD_MBROLA_DATA: isize = 13;
const WCMD_FMT_AMPLITUDE: isize = 14;
const WCMD_SONIC_SPEED: isize = 15;
const WCMD_PHONEME_ALIGNMENT: isize = 16;

const EMBED_P: usize = 1;
const EMBED_A: usize = 3;
const EMBED_H: usize = 5;
const EMBED_T: usize = 6;
const EMBED_F: usize = 13;

/// The layout of `wavegen_peaks_t`: Hz<<16 frequencies and widths, height<<15.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Peak {
    pub freq: i32,
    pub height: i32,
    pub left: i32,
    pub right: i32,
    pub freq1: f64,
    pub height1: f64,
    pub left1: f64,
    pub right1: f64,
    pub freq_inc: f64,
    pub height_inc: f64,
    pub left_inc: f64,
    pub right_inc: f64,
}

/// The layout of `WGEN_DATA`, shared with the Klatt and speechPlayer
/// synthesizers. Address fields hold the queue's raw addresses.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WgenData {
    pub pitch_env: usize,
    pub pitch: i32,
    pub pitch_ix: i32,
    pub pitch_inc: i32,
    pub pitch_base: i32,
    pub pitch_range: i32,
    pub mix_wavefile: usize,
    pub n_mix_wavefile: i32,
    pub mix_wave_scale: i32,
    pub mix_wave_amp: i32,
    pub mix_wavefile_ix: i32,
    pub mix_wavefile_max: i32,
    pub mix_wavefile_offset: i32,
    pub amplitude: i32,
    pub amplitude_v: i32,
    pub amplitude_fmt: i32,
}

/// The layout of `RESONATOR`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Resonator {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub x1: f64,
    pub x2: f64,
}

impl Resonator {
    fn step(&mut self, input: f64) -> f64 {
        let x = self.a * input + self.b * self.x1 + self.c * self.x2;
        self.x2 = self.x1;
        self.x1 = x;
        x
    }
}

/// The spectrum being played, for the low-harmonic increments: a separate
/// table, or an offset into the one being written.
#[derive(Clone, Copy)]
enum Current<'a> {
    Separate(&'a [i32]),
    Within(usize),
}

/// Output hooks that see each sample.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Hook {
    Voiced,
    Silence,
    Unvoiced,
}

/// Compile-time synthesizer options and the current voice's roughness.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Options {
    pub klatt: bool,
    pub mbrola: bool,
    pub sonic: bool,
    /// The current voice's roughness (`voice->roughness`, not the copy).
    pub roughness: i32,
}

/// Shared memory and the other synthesizers.
pub trait Host {
    fn samplerate(&mut self) -> i32;
    fn set_samplerate(&mut self, rate: i32);
    fn embedded(&mut self, index: usize) -> i32;
    fn set_embedded(&mut self, index: usize, value: i32);

    /// Queue head, tail and entries.
    fn head(&mut self) -> i32;
    fn tail(&mut self) -> i32;
    fn command(&mut self, index: i32) -> [isize; 4];
    fn advance_head(&mut self);

    /// Output bytes left in the buffer (`out_end - out_ptr`).
    fn room(&mut self) -> isize;
    /// Writes a sample's two bytes and advances the output.
    fn write(&mut self, sample: i32);

    /// Takes the echo ring's tail sample and advances it.
    fn echo_take(&mut self) -> i32;
    /// Stores a sample at the echo ring's head and advances it.
    fn echo_put(&mut self, sample: i32);
    fn echo_amp(&mut self) -> i32;
    /// Clears the ring and sets its head, tail and amplitude.
    fn echo_reset(&mut self, head: i32, amp: i32);

    fn byte(&mut self, address: usize, offset: i32) -> u8;
    fn frame(&mut self, address: usize) -> Frame;
    /// A queued voice copy; `None` for a null address.
    fn voice(&mut self, address: usize) -> Option<Voice>;
    fn free_voice(&mut self, address: usize);

    fn hook(&mut self, hook: Hook, sample: i32);
    /// A queued marker or phoneme alignment, from its queue entry.
    fn marker(&mut self, index: i32);
    fn alignment(&mut self, index: i32);
    fn samplerate_event(&mut self, rate: i32);
    fn sonic_speed(&mut self, index: i32);
    fn random(&mut self, min: i32, max: i32) -> i32;

    fn klatt_reset(&mut self) {}
    /// Runs the Klatt synthesizer on the shared data; 1 when the buffer filled.
    #[allow(clippy::too_many_arguments)]
    fn klatt(
        &mut self,
        _length: i32,
        _resume: bool,
        _fr1: usize,
        _fr2: usize,
        _data: *mut WgenData,
        _voice: *mut Voice,
    ) -> i32 {
        0
    }
    fn mbrola(&mut self, _length: i32, _resume: bool, _amp: i32) -> i32 {
        0
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum Shape {
    Standard,
    #[default]
    Squarer,
}

/// The generator's state (the legacy file and function statics).
#[derive(Clone, Debug)]
pub struct Wavegen {
    voice: Option<Voice>,
    option_harmonic1: i32,
    flutter_amp: i32,
    general_amplitude: i32,
    consonant_amp: i32,
    phase_inc_factor: i32,
    peaks: [Peak; N_PEAKS],
    peak_harmonic: [i32; N_PEAKS],
    peak_height: [i32; N_PEAKS],
    echo_length: i32,
    voicing: i32,
    breath: [Resonator; N_PEAKS],
    harm_inc: [i32; N_LOWHARM],
    harmspect: usize,
    hswitch: usize,
    hspect: [[i32; MAX_HARMONIC]; 2],
    nsamples: i32,
    modulation_type: i32,
    glottal_flag: i32,
    glottal_reduce: i32,
    pub data: WgenData,
    amp_ix: i32,
    amp_inc: i32,
    amplitude_env: usize,
    samplecount: i32,
    samplecount_start: i32,
    end_wave: i32,
    wavephase: i32,
    phaseinc: i32,
    cycle_samples: i32,
    cbytes: i32,
    hf_factor: i32,
    minus_pi_t: f64,
    two_pi_t: f64,
    const_f0: i32,
    flutter_inc: i32,
    wavemult_offset: i32,
    wavemult_max: i32,
    wavemult: [u8; 128],
    shape: Shape,
    // function statics
    flutter_ix: i32,
    maxh: i32,
    maxh2: i32,
    agc: i32,
    h_switch_sign: i32,
    cycle_count: i32,
    amplitude2: i32,
    silence_samples: i32,
    wave_samples: i32,
    wave_ix: i32,
    resume: bool,
    echo_complete: i32,
}

impl Default for Wavegen {
    fn default() -> Self {
        Self {
            voice: None,
            option_harmonic1: 10,
            flutter_amp: 64,
            general_amplitude: 60,
            consonant_amp: 26,
            phase_inc_factor: 0,
            peaks: [Peak::default(); N_PEAKS],
            peak_harmonic: [0; N_PEAKS],
            peak_height: [0; N_PEAKS],
            echo_length: 0,
            voicing: 0,
            breath: [Resonator::default(); N_PEAKS],
            harm_inc: [0; N_LOWHARM],
            harmspect: 0,
            hswitch: 0,
            hspect: [[0; MAX_HARMONIC]; 2],
            nsamples: 0,
            modulation_type: 0,
            glottal_flag: 0,
            glottal_reduce: 0,
            data: WgenData::default(),
            amp_ix: 0,
            amp_inc: 0,
            amplitude_env: 0,
            samplecount: 0,
            samplecount_start: 0,
            end_wave: 0,
            wavephase: 0,
            phaseinc: 0,
            cycle_samples: 0,
            cbytes: 0,
            hf_factor: 0,
            minus_pi_t: 0.0,
            two_pi_t: 0.0,
            const_f0: 0,
            flutter_inc: 0,
            wavemult_offset: 0,
            wavemult_max: 0,
            wavemult: WAVEMULT,
            shape: Shape::Squarer,
            flutter_ix: 0,
            maxh: 0,
            maxh2: 0,
            agc: 256,
            h_switch_sign: 0,
            cycle_count: 0,
            amplitude2: 0,
            silence_samples: 0,
            wave_samples: 0,
            wave_ix: 0,
            resume: false,
            echo_complete: 0,
        }
    }
}

fn set_resonator(
    r: &mut Resonator,
    minus_pi_t: f64,
    two_pi_t: f64,
    freq: i32,
    bwidth: i32,
    init: bool,
) {
    if init {
        r.x1 = 0.0;
        r.x2 = 0.0;
    }
    let x = (minus_pi_t * f64::from(bwidth)).exp();
    r.c = -(x * x);
    r.b = x * (two_pi_t * f64::from(freq)).cos() * 2.0;
    r.a = 1.0 - r.b - r.c;
}

fn with_range0(value: i32, max: i32) -> i32 {
    if value < 0 {
        0
    } else if value > max {
        max
    } else {
        value
    }
}

/// C's `(int)` of a double as the compiled C does it: x86 converts an
/// out-of-range value or NaN to `INT_MIN`; other targets saturate.
fn to_int(x: f64) -> i32 {
    if cfg!(any(target_arch = "x86", target_arch = "x86_64"))
        && !(x > -2147483649.0 && x < 2147483648.0)
    {
        i32::MIN
    } else {
        x as i32
    }
}

/// Integer division; where C would trap on a zero divisor, 0.
fn div(a: i32, b: i32) -> i32 {
    if b == 0 {
        0
    } else {
        a.wrapping_div(b)
    }
}

impl Wavegen {
    pub fn voice(&self) -> Option<&Voice> {
        self.voice.as_ref()
    }

    /// Pointers to the shared data and voice copy, for the Klatt synthesizer.
    pub fn klatt_parts(&mut self) -> (*mut WgenData, *mut Voice) {
        let voice = match self.voice.as_mut() {
            Some(voice) => voice as *mut Voice,
            None => std::ptr::null_mut(),
        };
        (&mut self.data, voice)
    }

    pub fn set_const_f0(&mut self, f0: i32) {
        self.const_f0 = f0;
    }

    /// `WavegenInit`, apart from the Klatt synthesizer's own setup.
    pub fn init(&mut self, host: &mut impl Host, rate: i32, wavemult_fact: i32) {
        let wavemult_fact = if wavemult_fact == 0 {
            60
        } else {
            wavemult_fact
        };
        self.voice = None;
        host.set_samplerate(rate);
        self.phase_inc_factor = div(0x8000000, rate); // pitch is Hz*32
        self.flutter_inc = div(64i32.wrapping_mul(rate), rate);
        self.samplecount = 0;
        self.nsamples = 0;
        self.wavephase = 0x7fffffff;
        self.data.amplitude = 32;
        self.data.amplitude_fmt = 100;
        for (ix, &value) in EMBEDDED_DEFAULT.iter().enumerate() {
            host.set_embedded(ix, value);
        }
        // a window spreading harmonics from a single HF peak
        self.wavemult_max = ((rate.wrapping_mul(wavemult_fact)) / (256 * 50)).min(N_WAVEMULT);
        self.wavemult_offset = self.wavemult_max / 2;
        if rate != 22050 {
            // the presets are for 22050 Hz
            for ix in 0..self.wavemult_max.max(0) {
                let x = 127.0
                    * (1.0
                        - ((std::f64::consts::PI * 2.0) * f64::from(ix)
                            / f64::from(self.wavemult_max))
                        .cos());
                self.wavemult[ix as usize] = to_int(x) as u8;
            }
        }
        self.shape = Shape::Squarer;
    }

    /// `GetAmplitude`.
    pub fn amplitude(&mut self, host: &mut impl Host) -> i32 {
        let amplitude = host.embedded(EMBED_A);
        let emphasis = host.embedded(EMBED_F);
        if let Ok(value) = usize::try_from(emphasis)
            .map_err(|_| ())
            .and_then(|emphasis| parameters::general_amplitude(amplitude, emphasis).map_err(|_| ()))
        {
            self.general_amplitude = value;
        }
        self.general_amplitude
    }

    fn set_echo(&mut self, host: &mut impl Host) {
        let Some(voice) = self.voice.as_ref() else {
            return;
        };
        self.voicing = voice.voicing;
        let mut delay = voice.echo_delay.min(N_ECHO_BUF - 1);
        let mut amp = voice.echo_amplitude.min(100);
        let embedded = host.embedded(EMBED_H);
        if embedded > 0 {
            // echo from an embedded command
            amp = embedded;
            delay = 130;
        }
        if delay == 0 {
            amp = 0;
        }
        let head = delay.wrapping_mul(host.samplerate()) / 1000;
        host.echo_reset(head, amp);
        // to ensure completion of echo at the end of speech
        self.echo_length = if amp == 0 {
            0
        } else if amp > 20 {
            head.wrapping_mul(2) // two periods for a loud echo
        } else {
            head
        };
        // compensate partly for the echo's added amplitude
        self.general_amplitude = self.amplitude(host);
        self.general_amplitude = self.general_amplitude.wrapping_mul(500 - amp) / 500;
    }

    fn set_pitch_formants(&mut self, host: &mut impl Host) {
        let (pitch, tone) = (host.embedded(EMBED_P), host.embedded(EMBED_T));
        if let Some(voice) = self.voice.as_mut() {
            let _ = parameters::pitch_formants(voice, pitch, tone);
        }
    }

    /// `SetEmbedded`: an embedded command in the text.
    pub fn set_embedded(&mut self, host: &mut impl Host, control: i32, value: i32) {
        let command = (control & 0x1f) as usize;
        let sign = match control & 0x60 {
            0x60 => -1,
            0x40 => 1,
            _ => 0,
        };
        if command < N_EMBEDDED_VALUES {
            let current = if sign == 0 {
                value
            } else {
                host.embedded(command)
                    .wrapping_add(value.wrapping_mul(sign))
            };
            host.set_embedded(command, with_range0(current, EMBEDDED_MAX[command]));
        }
        match command {
            EMBED_T => {
                self.set_echo(host);
                self.set_pitch_formants(host);
            }
            EMBED_P => self.set_pitch_formants(host),
            EMBED_A | EMBED_F => self.general_amplitude = self.amplitude(host),
            EMBED_H => self.set_echo(host),
            _ => {}
        }
    }

    /// `WavegenSetVoice`: copies the voice.
    pub fn set_voice(&mut self, host: &mut impl Host, voice: &Voice) {
        self.voice = Some(*voice);
        self.shape = if voice.peak_shape == 0 {
            Shape::Standard
        } else {
            Shape::Squarer
        };
        self.consonant_amp = voice.consonant_amplitude.wrapping_mul(26) / 100;
        if host.samplerate() <= 11000 {
            self.consonant_amp = self.consonant_amp.wrapping_mul(2); // emphasize at low rates
            self.option_harmonic1 = 6;
        }
        self.set_echo(host);
        self.set_pitch_formants(host);
        host.samplerate_event(voice.sample_rate);
    }

    /// `InitBreath`.
    pub fn init_breath(&mut self, samplerate: i32) {
        self.minus_pi_t = -std::f64::consts::PI / f64::from(samplerate);
        self.two_pi_t = -2.0 * self.minus_pi_t;
        for r in &mut self.breath {
            set_resonator(r, self.minus_pi_t, self.two_pi_t, 2000, 200, true);
        }
    }

    fn set_breath(&mut self) {
        let Some(voice) = self.voice.as_ref() else {
            return;
        };
        if voice.breath[0] == 0 {
            return;
        }
        for pk in 1..N_PEAKS {
            if voice.breath[pk] != 0 {
                // the current formant's frequency, the voice's width
                set_resonator(
                    &mut self.breath[pk],
                    self.minus_pi_t,
                    self.two_pi_t,
                    self.peaks[pk].freq >> 16,
                    voice.breath_width[pk],
                    false,
                );
            }
        }
    }

    fn apply_breath(&mut self, host: &mut impl Host) -> i32 {
        let Some(voice) = self.voice.as_ref() else {
            return 0;
        };
        let noise = host.random(-0x2000, 0x1fff);
        let mut value = 0i32;
        for ix in 1..N_PEAKS {
            let amp = voice.breath[ix];
            if amp != 0 {
                let amp = amp.wrapping_mul(self.peaks[ix].height >> 14);
                let out = to_int(self.breath[ix].step(f64::from(noise)));
                value = value.wrapping_add(out.wrapping_mul(amp));
            }
        }
        value
    }

    /// `PeaksToHarmspect`: harmonic amplitudes from the formant peaks into
    /// `htab` (`MAX_HARMONIC` entries); control bit 0 also sets the
    /// low-harmonic increments from the spectrum being played.
    pub fn peaks_to_harmspect(
        &mut self,
        samplerate: i32,
        peaks: &[Peak; N_PEAKS],
        pitch: i32,
        htab: &mut [i32],
        control: i32,
    ) -> i32 {
        let current = self.hspect[self.harmspect];
        self.harmonics(
            samplerate,
            peaks,
            pitch,
            htab,
            control,
            Current::Separate(&current),
        )
    }

    /// Harmonic amplitudes from the formant peaks into
    /// `htab`; `current` is the spectrum being played, for the low-harmonic
    /// increments (control bit 0). Returns the highest harmonic, or 1 with no
    /// voice.
    fn harmonics(
        &mut self,
        samplerate: i32,
        peaks: &[Peak; N_PEAKS],
        pitch: i32,
        htab: &mut [i32],
        control: i32,
        current: Current<'_>,
    ) -> i32 {
        let Some(voice) = self.voice.as_ref() else {
            return 1;
        };
        if pitch <= 0 {
            return 0; // C divides by zero or runs through memory
        }
        let n_peaks = voice.harmonic_peaks;
        let shape: &[u8] = match self.shape {
            Shape::Standard => &PK_SHAPE1,
            Shape::Squarer => &PK_SHAPE2,
        };
        let peak = |ix: i32| peaks.get(ix.max(0) as usize).copied().unwrap_or_default();

        let top = peak(n_peaks);
        let mut hmax = top.freq.wrapping_add(top.right) / pitch;
        if hmax >= MAX_HARMONIC as i32 {
            hmax = MAX_HARMONIC as i32 - 1;
        }
        // only up to 95% of the Nyquist frequency
        let hmax_samplerate = ((samplerate.wrapping_mul(19) / 40).wrapping_shl(16)) / pitch;
        hmax = hmax.min(hmax_samplerate);
        for slot in htab.iter_mut().take((hmax.max(-1) + 1) as usize) {
            *slot = 0;
        }
        let add = |htab: &mut [i32], h: i32, value: i32| {
            if let Some(slot) = usize::try_from(h).ok().and_then(|h| htab.get_mut(h)) {
                *slot = slot.wrapping_add(value);
            }
        };

        let mut pk = 0;
        while pk <= n_peaks {
            let p = peak(pk);
            pk += 1;
            let fp = p.freq;
            if p.height == 0 || fp == 0 {
                continue;
            }
            let fhi = p.freq.wrapping_add(p.right);
            let mut h = (p.freq.wrapping_sub(p.left) / pitch).wrapping_add(1).max(1);
            let mut f = pitch.wrapping_mul(h);
            while f < fp {
                if let Some(width) = fp.wrapping_sub(f).checked_div(p.left >> 8) {
                    let s = shape.get(width as usize).copied().unwrap_or(0);
                    add(htab, h, i32::from(s).wrapping_mul(p.height));
                }
                h += 1;
                f = f.wrapping_add(pitch);
            }
            while f < fhi {
                if let Some(width) = f.wrapping_sub(fp).checked_div(p.right >> 8) {
                    let s = shape.get(width as usize).copied().unwrap_or(0);
                    add(htab, h, i32::from(s).wrapping_mul(p.height));
                }
                h += 1;
                f = f.wrapping_add(pitch);
            }
        }

        // increase bass, in 1/256ths, decreasing until 1000 Hz
        let mut y = peaks[1].height.wrapping_mul(10);
        let h2 = (1000 << 16) / pitch;
        if h2 > 0 {
            let x = y / h2;
            let mut h = 1;
            while y > 0 {
                add(htab, h, y);
                h += 1;
                y -= x;
                if x <= 0 {
                    break; // C never ends here
                }
            }
        }

        // the nearest harmonic for HF peaks, without shape
        let mut pk = pk.max(0) as usize;
        while pk < N_PEAKS {
            let x = peaks[pk].height >> 14;
            self.peak_height[pk] = x.wrapping_mul(x).wrapping_mul(5) / 2;
            if control == 0 {
                // initially; later changes only at a quiet point
                self.peak_harmonic[pk] = peaks[pk].freq / pitch;
            }
            if self.peak_harmonic[pk] >= hmax_samplerate {
                self.peak_height[pk] = 0; // only up to half the sample rate
            }
            pk += 1;
        }

        // from the square-rooted values
        let mut f = 0i32;
        for h in 0..=hmax.max(-1) {
            let Some(slot) = htab.get_mut(h as usize) else {
                break;
            };
            let x = *slot >> 15;
            *slot = x.wrapping_mul(x) >> 8;
            let ix = f >> 19; // tone_adjust in steps of 8 Hz
            if ix < N_TONE_ADJUST {
                if let Some(&tone) = voice.tone.get(ix.max(0) as usize) {
                    *slot = slot.wrapping_mul(i32::from(tone)) >> 13;
                }
            }
            f = f.wrapping_add(pitch);
        }

        // the first harmonic's amplitude affects tonal quality
        if let Some(first) = htab.get_mut(1) {
            *first = first.wrapping_mul(self.option_harmonic1) / 8;
        }

        if control & 1 != 0 {
            // intermediate increments of the low harmonics
            for h in 1..N_LOWHARM {
                let now = match current {
                    Current::Separate(current) => current.get(h),
                    Current::Within(offset) => htab.get(offset + h),
                };
                let next = htab.get(h).copied().unwrap_or(0);
                self.harm_inc[h] = next.wrapping_sub(now.copied().unwrap_or(0)) >> 3;
            }
        }
        hmax
    }

    fn harmonics_into(&mut self, samplerate: i32, target: usize, control: i32) -> i32 {
        let peaks = self.peaks;
        let pitch = self.data.pitch.wrapping_shl(4);
        // C's two copies are contiguous: writes past the first run on into
        // the second (past the second they are dropped)
        let mut flat = [0; 2 * MAX_HARMONIC];
        flat.copy_from_slice(self.hspect.as_flattened());
        let (base, now) = (target * MAX_HARMONIC, self.harmspect * MAX_HARMONIC);
        let current_row = self.hspect[self.harmspect];
        let current = match now.checked_sub(base) {
            Some(offset) => Current::Within(offset),
            None => Current::Separate(&current_row),
        };
        let hmax = self.harmonics(
            samplerate,
            &peaks,
            pitch,
            &mut flat[base..],
            control,
            current,
        );
        for (row, values) in self.hspect.iter_mut().zip(flat.chunks_exact(MAX_HARMONIC)) {
            row.copy_from_slice(values);
        }
        hmax
    }

    /// Every 64 samples: pitch, flutter and formant steps.
    fn advance(&mut self, host: &mut impl Host) {
        let Some(voice) = self.voice.as_ref() else {
            return;
        };
        let n_peaks = voice.harmonic_peaks;
        self.data.pitch_ix = self.data.pitch_ix.wrapping_add(self.data.pitch_inc);
        let ix = (self.data.pitch_ix >> 8).min(127);
        let mut x = 0;
        if self.data.pitch_env != 0 {
            x = i32::from(host.byte(self.data.pitch_env, ix)).wrapping_mul(self.data.pitch_range);
        }
        self.data.pitch = (x >> 8).wrapping_add(self.data.pitch_base);
        self.amp_ix = self.amp_ix.wrapping_add(self.amp_inc);

        // pitch flutter
        if self.flutter_ix >= N_FLUTTER * 64 {
            self.flutter_ix = 0;
        }
        let x = (i32::from(FLUTTER_TAB[(self.flutter_ix >> 6).clamp(0, N_FLUTTER - 1) as usize])
            - 0x80)
            .wrapping_mul(self.flutter_amp);
        self.flutter_ix = self.flutter_ix.wrapping_add(self.flutter_inc);
        self.data.pitch = self.data.pitch.wrapping_add(x);
        if self.const_f0 != 0 {
            self.data.pitch = self.const_f0.wrapping_shl(12);
        }
        self.data.pitch = self.data.pitch.max(MIN_PITCH);

        if self.samplecount == self.samplecount_start {
            return;
        }
        let mut ix = 0usize;
        while (ix as i32) <= n_peaks && ix < N_PEAKS {
            let p = &mut self.peaks[ix];
            p.freq1 += p.freq_inc;
            p.freq = to_int(p.freq1);
            p.height1 += p.height_inc;
            p.height = to_int(p.height1).max(0);
            p.left1 += p.left_inc;
            p.left = to_int(p.left1);
            if ix < 3 {
                p.right1 += p.right_inc;
                p.right = to_int(p.right1);
            } else {
                p.right = p.left;
            }
            ix += 1;
        }
        // formants 6, 7, 8 have no width
        while ix < 8 {
            let p = &mut self.peaks[ix];
            if ix < 7 {
                p.freq1 += p.freq_inc;
                p.freq = to_int(p.freq1);
            }
            p.height1 += p.height_inc;
            p.height = to_int(p.height1).max(0);
            ix += 1;
        }
    }

    /// `SetPitch`: length in samples.
    fn set_pitch(
        &mut self,
        length: i32,
        env: usize,
        fall: usize,
        pitch1: i32,
        pitch2: i32,
        host: &mut impl Host,
    ) {
        let Some(voice) = self.voice.as_ref() else {
            return;
        };
        self.data.pitch_env = if env == 0 { fall } else { env };
        self.data.pitch_ix = 0;
        self.data.pitch_inc = div(256 * ENV_LEN * STEPSIZE, length);
        let embedded = parameters::Embedded {
            pitch: host.embedded(EMBED_P),
            tone: host.embedded(EMBED_T),
            range: host.embedded(4),
        };
        if let Ok(pitch) = parameters::pitch(voice, pitch1, pitch2, embedded) {
            self.data.pitch_base = pitch.base;
            self.data.pitch_range = pitch.range;
        }
        // the initial pitch, Hz << 12
        let first = i32::from(host.byte(self.data.pitch_env, 0));
        self.data.pitch =
            (first.wrapping_mul(self.data.pitch_range) >> 8).wrapping_add(self.data.pitch_base);
        self.flutter_amp = voice.flutter;
    }

    fn set_amplitude(&mut self, length: i32, env: usize, value: i32) {
        let Some(voice) = self.voice.as_ref() else {
            return;
        };
        if let Ok(amplitude) = parameters::amplitude(
            length,
            value,
            self.general_amplitude,
            voice.voiced_consonant_amplitude,
        ) {
            self.amp_ix = 0;
            self.amp_inc = amplitude.increment;
            self.data.amplitude = amplitude.value;
            self.data.amplitude_v = amplitude.voiced;
            self.amplitude_env = env;
        }
    }

    /// `SetSynth`: a spectrum segment between two frames.
    fn set_synth(&mut self, host: &mut impl Host, length: i32, modn: i32, fr1: usize, fr2: usize) {
        let Some(v) = self.voice else {
            return;
        };
        const GLOTTAL_BEFORE: [i32; 4] = [0x30, 0x30, 0x40, 0x50]; // vowel before [?]
        const GLOTTAL_AFTER: [i32; 4] = [0x90, 0xa0, 0xb0, 0xc0]; // vowel after [?]
        self.end_wave = 1;
        self.modulation_type = modn & 0xff;
        self.glottal_flag = 0;
        if modn & 0x400 != 0 {
            self.glottal_flag = 3; // before a glottal stop
            self.glottal_reduce = GLOTTAL_BEFORE[((modn >> 8) & 3) as usize];
        }
        if modn & 0x800 != 0 {
            self.glottal_flag = 4; // after a glottal stop
            self.glottal_reduce = GLOTTAL_AFTER[((modn >> 8) & 3) as usize];
        }

        let tail = host.tail();
        let mut qix = host.head() + 1;
        loop {
            if qix >= N_WCMDQ {
                qix = 0;
            }
            if qix == tail {
                break;
            }
            let cmd = host.command(qix)[0] as i32 as isize;
            if cmd == WCMD_SPECT {
                self.end_wave = 0; // the next wave comes from another spectrum
                break;
            }
            if cmd == WCMD_WAVE || cmd == WCMD_PAUSE {
                break; // not a spectrum: continue to the cycle's end
            }
            qix += 1;
        }

        // a multiple of the step size
        let mut length2 = length.wrapping_add(STEPSIZE / 2) & !0x3f;
        if length2 == 0 {
            length2 = STEPSIZE;
        }
        // add to any left over from the previous segment
        self.samplecount_start = self.samplecount;
        self.nsamples = self.nsamples.wrapping_add(length2);
        let length4 = length2 / 4;

        let (f1, f2) = (host.frame(fr1), host.frame(fr2));
        let freq = |k: usize| i32::from(v.frequency[k]);
        let add = |k: usize| i32::from(v.frequency_add[k]).wrapping_mul(256);
        self.peaks[7].freq = (7800i32.wrapping_mul(freq(7)).wrapping_add(add(7))).wrapping_shl(8);
        self.peaks[8].freq = (9000i32.wrapping_mul(freq(8)).wrapping_add(add(8))).wrapping_shl(8);
        for ix in 0..8 {
            let p = &mut self.peaks[ix];
            if ix < 7 {
                p.freq1 = f64::from(
                    (i32::from(f1.frequencies[ix])
                        .wrapping_mul(freq(ix))
                        .wrapping_add(add(ix)))
                    .wrapping_shl(8),
                );
                p.freq = to_int(p.freq1);
                let next = f64::from(
                    (i32::from(f2.frequencies[ix])
                        .wrapping_mul(freq(ix))
                        .wrapping_add(add(ix)))
                    .wrapping_shl(8),
                );
                // lower headroom for fixed point
                p.freq_inc = ((next - p.freq1) * f64::from(STEPSIZE / 4)) / f64::from(length4);
            }
            let height = i32::from(v.height[ix]);
            p.height1 = f64::from((i32::from(f1.heights[ix]).wrapping_mul(height)).wrapping_shl(6));
            p.height = to_int(p.height1);
            let next = f64::from((i32::from(f2.heights[ix]).wrapping_mul(height)).wrapping_shl(6));
            p.height_inc = ((next - p.height1) * f64::from(STEPSIZE)) / f64::from(length2);

            if ix <= 5 && ix as i32 <= v.harmonic_peaks {
                let width = i32::from(v.width[ix]);
                p.left1 =
                    f64::from((i32::from(f1.widths[ix]).wrapping_mul(width)).wrapping_shl(10));
                p.left = to_int(p.left1);
                let next =
                    f64::from((i32::from(f2.widths[ix]).wrapping_mul(width)).wrapping_shl(10));
                p.left_inc = ((next - p.left1) * f64::from(STEPSIZE)) / f64::from(length2);
                if ix < 3 {
                    p.right1 =
                        f64::from((i32::from(f1.right[ix]).wrapping_mul(width)).wrapping_shl(10));
                    p.right = to_int(p.right1);
                    let next =
                        f64::from((i32::from(f2.right[ix]).wrapping_mul(width)).wrapping_shl(10));
                    p.right_inc = ((next - p.right1) * f64::from(STEPSIZE)) / f64::from(length2);
                } else {
                    p.right = p.left;
                }
            }
        }
    }

    /// `Wavegen`: formant synthesis until the segment or buffer ends.
    /// Returns 1 when the buffer filled first.
    #[allow(clippy::too_many_arguments)]
    fn wavegen(
        &mut self,
        host: &mut impl Host,
        options: &Options,
        length: i32,
        modulation: i32,
        resume: bool,
        fr1: usize,
        fr2: usize,
    ) -> i32 {
        if !resume {
            self.set_synth(host, length, modulation, fr1, fr2);
        }
        let Some(voice) = self.voice else {
            return 0;
        };
        let samplerate = host.samplerate();
        let n_peaks = voice.harmonic_peaks;
        let first_hf = (n_peaks + 1).max(0) as usize;

        loop {
            if self.end_wave == 0 && self.samplecount == self.nsamples {
                return 0;
            }
            if self.samplecount & 0x3f == 0 {
                // every 64 samples, adjust the parameters
                if self.samplecount == 0 {
                    self.hswitch = 0;
                    self.harmspect = 0;
                    self.maxh2 = self.harmonics_into(samplerate, 0, 0);
                    // fewer harmonics at higher pitch
                    self.amplitude2 = (self
                        .data
                        .amplitude
                        .wrapping_mul(self.data.pitch >> 8)
                        .wrapping_mul(self.data.amplitude_fmt))
                        / (10000 << 3);
                    // switch the sign of harmonics above about 900 Hz to reduce peaks
                    self.h_switch_sign = div(890, self.data.pitch >> 12);
                } else {
                    self.advance(host);
                }
                // pitch is Hz << 12
                self.phaseinc = (self.data.pitch >> 7).wrapping_mul(self.phase_inc_factor);
                self.cycle_samples = div(samplerate, self.data.pitch >> 12);
                self.hf_factor = self.data.pitch >> 11;
                self.maxh = self.maxh2;
                self.harmspect = self.hswitch;
                self.hswitch ^= 1;
                self.maxh2 = self.harmonics_into(samplerate, self.hswitch, 1);
                self.set_breath();
            } else if self.samplecount & 0x07 == 0 {
                let current = self.harmspect;
                let mut h = 1;
                while h < N_LOWHARM && (h as i32) <= self.maxh2 && (h as i32) <= self.maxh {
                    self.hspect[current][h] =
                        self.hspect[current][h].wrapping_add(self.harm_inc[h]);
                    h += 1;
                }
                // automatic gain control back towards unity
                if self.agc < 256 {
                    self.agc += 1;
                }
            }

            self.samplecount = self.samplecount.wrapping_add(1);

            if self.wavephase > 0 {
                self.wavephase = self.wavephase.wrapping_add(self.phaseinc);
                if self.wavephase < 0 {
                    // the sign changed: a quiet point in the waveform
                    self.cbytes = self.wavemult_offset - self.cycle_samples / 2;
                    if self.samplecount > self.nsamples {
                        return 0;
                    }
                    self.cycle_count = self.cycle_count.wrapping_add(1);
                    for pk in first_hf..N_PEAKS {
                        // the nearest harmonic for HF peaks
                        self.peak_harmonic[pk] =
                            div(self.peaks[pk].freq, self.data.pitch.wrapping_mul(8))
                                .wrapping_add(1)
                                / 2;
                    }
                    self.amplitude2 = (self
                        .data
                        .amplitude
                        .wrapping_mul(self.data.pitch >> 8)
                        .wrapping_mul(self.data.amplitude_fmt))
                        / (10000 << 3);
                    if self.glottal_flag > 0 {
                        if self.glottal_flag == 3 {
                            if self.nsamples.wrapping_sub(self.samplecount)
                                < self.cycle_samples.wrapping_mul(2)
                            {
                                // the penultimate cycle before a glottal stop
                                self.glottal_flag = 2;
                                self.amplitude2 =
                                    self.amplitude2.wrapping_mul(self.glottal_reduce) / 256;
                            }
                        } else if self.glottal_flag == 4 {
                            // the second cycle after a glottal stop
                            self.glottal_flag = 2;
                            self.amplitude2 =
                                self.amplitude2.wrapping_mul(self.glottal_reduce) / 256;
                        } else {
                            self.glottal_flag -= 1;
                        }
                    }
                    if self.amplitude_env != 0 {
                        // creaky voice on certain vowels or tones
                        let ix = (self.amp_ix >> 8).min(127);
                        let amp = i32::from(host.byte(self.amplitude_env, ix));
                        self.amplitude2 = self.amplitude2.wrapping_mul(amp) / 128;
                    }
                    // roughness: reduce the amplitude of some cycles
                    let mut modn_period = 0;
                    let mut modn_amp = 1;
                    if options.roughness < N_ROUGHNESS {
                        // C reads rows past the end for a high modulation type
                        let at = options
                            .roughness
                            .wrapping_mul(8)
                            .wrapping_add(self.modulation_type);
                        let entry = usize::try_from(at)
                            .ok()
                            .and_then(|at| MODULATION_TAB.as_flattened().get(at))
                            .copied()
                            .unwrap_or(0);
                        modn_period = i32::from(entry);
                        modn_amp = modn_period & 0xf;
                        modn_period >>= 4;
                    }
                    if modn_period != 0 {
                        if modn_period == 0xf {
                            // just once
                            self.amplitude2 = self.amplitude2.wrapping_mul(modn_amp) / 16;
                            self.modulation_type = 0;
                        } else if self.cycle_count.wrapping_rem(modn_period) == 0 {
                            self.amplitude2 = self.amplitude2.wrapping_mul(modn_amp) / 16;
                        }
                    }
                }
            } else {
                self.wavephase = self.wavephase.wrapping_add(self.phaseinc);
            }
            let waveph = (self.wavephase >> 16) as u16;
            let mut total: i32 = 0;

            // HF peaks (formants 6, 7, 8): one harmonic each, spread by a window
            self.cbytes = self.cbytes.wrapping_add(1);
            if self.cbytes >= 0 && self.cbytes < self.wavemult_max {
                for pk in first_hf..N_PEAKS {
                    let theta = (self.peak_harmonic[pk].wrapping_mul(i32::from(waveph))) as u16;
                    let product = i64::from(SIN_TAB[usize::from(theta >> 5)])
                        * i64::from(self.peak_height[pk]);
                    total = (i64::from(total) + product) as i32;
                }
                let spread = i64::from(div(total, self.hf_factor))
                    * i64::from(self.wavemult[self.cbytes as usize]);
                total = spread as i32;
            }

            // main peaks, formants 0 to 5
            // C's pointer runs on into the second copy
            let spectrum = &self.hspect.as_flattened()[self.harmspect * MAX_HARMONIC..];
            let mut theta = waveph;
            let mut h = 1;
            while h <= self.h_switch_sign {
                let value = spectrum.get(h as usize).copied().unwrap_or(0);
                total = total
                    .wrapping_add(i32::from(SIN_TAB[usize::from(theta >> 5)]).wrapping_mul(value));
                theta = theta.wrapping_add(waveph);
                h += 1;
            }
            while h <= self.maxh {
                let value = spectrum.get(h as usize).copied().unwrap_or(0);
                total = total
                    .wrapping_sub(i32::from(SIN_TAB[usize::from(theta >> 5)]).wrapping_mul(value));
                theta = theta.wrapping_add(waveph);
                h += 1;
            }

            if self.voicing != 64 {
                total = (total >> 6).wrapping_mul(self.voicing);
            }
            if voice.breath[0] != 0 {
                total = total.wrapping_add(self.apply_breath(host));
            }

            // mix with a sampled wave
            let mut z2 = 0i32;
            if self.data.mix_wavefile_ix < self.data.n_mix_wavefile {
                let d = &mut self.data;
                let at = d.mix_wavefile_ix.wrapping_add(d.mix_wavefile_offset);
                let sample = if d.mix_wave_scale == 0 {
                    // 16 bit
                    let high = i32::from(host.byte(d.mix_wavefile, at.wrapping_add(1)) as i8);
                    d.mix_wavefile_ix = d.mix_wavefile_ix.wrapping_add(2);
                    i32::from(host.byte(d.mix_wavefile, at)).wrapping_add(high.wrapping_mul(256))
                } else {
                    // 8 bit, scaled
                    d.mix_wavefile_ix = d.mix_wavefile_ix.wrapping_add(1);
                    i32::from(host.byte(d.mix_wavefile, at) as i8).wrapping_mul(d.mix_wave_scale)
                };
                z2 = sample.wrapping_mul(d.amplitude_v) >> 10;
                z2 = z2.wrapping_mul(d.mix_wave_amp) / 32;
                if d.mix_wavefile_ix.wrapping_add(d.mix_wavefile_offset) >= d.mix_wavefile_max {
                    // the end of the available wave data
                    d.mix_wavefile_offset = d
                        .mix_wavefile_offset
                        .wrapping_sub(d.mix_wavefile_max.wrapping_mul(3) / 4);
                }
            }

            let mut z1 = z2.wrapping_add(((total >> 8).wrapping_mul(self.amplitude2)) >> 13);
            let echo = host.echo_take().wrapping_mul(host.echo_amp());
            z1 = z1.wrapping_add(echo >> 8);

            let mut z = z1.wrapping_mul(self.agc) >> 8;
            // 16 bit overflow: reduce by the gain control
            if z >= 32768 {
                let ov = 8388608 / z1 - 1;
                self.agc = self.agc.min(ov);
                z = z1.wrapping_mul(self.agc) >> 8;
            } else if z <= -32768 {
                let ov = -8388608 / z1 - 1;
                self.agc = self.agc.min(ov);
                z = z1.wrapping_mul(self.agc) >> 8;
            }
            host.write(z);
            host.hook(Hook::Voiced, z);
            host.echo_put(z);
            if host.room() < 2 {
                return 1;
            }
        }
    }

    fn play_silence(&mut self, host: &mut impl Host, length: i32, resume: bool) -> i32 {
        self.nsamples = 0;
        self.samplecount = 0;
        self.wavephase = 0x7fffffff;
        if length == 0 {
            return 0;
        }
        if !resume {
            self.silence_samples = length;
        }
        loop {
            let left = self.silence_samples;
            self.silence_samples = left.wrapping_sub(1);
            if left <= 0 {
                return 0;
            }
            let value = host.echo_take().wrapping_mul(host.echo_amp()) >> 8;
            host.write(value);
            host.hook(Hook::Silence, value);
            host.echo_put(value);
            if host.room() < 2 {
                return 1;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn play_wave(
        &mut self,
        host: &mut impl Host,
        length: i32,
        resume: bool,
        wave: usize,
        scale: i32,
        amp: i32,
    ) -> i32 {
        if !resume {
            self.wave_samples = length;
            self.wave_ix = 0;
        }
        self.nsamples = 0;
        self.samplecount = 0;
        loop {
            let left = self.wave_samples;
            self.wave_samples = left.wrapping_sub(1);
            if left <= 0 {
                return 0;
            }
            let mut value = if scale == 0 {
                // 16 bit
                let high = i32::from(host.byte(wave, self.wave_ix.wrapping_add(1)) as i8);
                let low = i32::from(host.byte(wave, self.wave_ix));
                self.wave_ix = self.wave_ix.wrapping_add(2);
                low.wrapping_add(high.wrapping_mul(256))
            } else {
                // 8 bit, shifted by the scale factor
                let value = i32::from(host.byte(wave, self.wave_ix) as i8).wrapping_mul(scale);
                self.wave_ix = self.wave_ix.wrapping_add(1);
                value
            };
            // a consonant's strength
            value = value.wrapping_mul(self.consonant_amp.wrapping_mul(self.general_amplitude));
            value >>= 10;
            value = value.wrapping_mul(amp) / 32;
            value = value.wrapping_add(host.echo_take().wrapping_mul(host.echo_amp()) >> 8);
            value = value.clamp(-32768, 32767);
            host.write(value);
            host.hook(Hook::Unvoiced, value);
            host.echo_put((value * 3) / 4);
            if host.room() < 2 {
                return 1;
            }
        }
    }

    /// `WavegenFill2`: runs queued commands until the output buffer is full
    /// (0) or the queue is empty (1). `fall` is the default pitch envelope.
    pub fn fill(&mut self, host: &mut impl Host, options: &Options, fall: usize) -> i32 {
        if self.data.pitch < MIN_PITCH {
            self.data.pitch = MIN_PITCH;
        }
        while host.room() > 0 {
            let head = host.head();
            let tail = host.tail();
            let mut free = head.wrapping_sub(tail);
            if free <= 0 {
                free += N_WCMDQ;
            }
            if N_WCMDQ - free <= 0 {
                if self.echo_complete > 0 {
                    // silence until the echo completes
                    self.resume = self.play_silence(host, self.echo_complete, self.resume) != 0;
                    if self.resume {
                        return 0;
                    }
                }
                return 1; // queue empty
            }

            let q = host.command(head);
            let length = q[1] as i32;
            let mut result = 0;
            let resume = self.resume;
            match q[0] & 0xff {
                WCMD_PITCH => self.set_pitch(
                    length,
                    q[2] as usize,
                    fall,
                    (q[3] >> 16) as i32,
                    (q[3] & 0xffff) as i32,
                    host,
                ),
                WCMD_PHONEME_ALIGNMENT => host.alignment(head),
                WCMD_PAUSE => {
                    if !resume {
                        self.echo_complete = self.echo_complete.wrapping_sub(length);
                    }
                    self.data.n_mix_wavefile = 0;
                    self.data.amplitude_fmt = 100;
                    if options.klatt {
                        host.klatt_reset();
                    }
                    result = self.play_silence(host, length, resume);
                }
                WCMD_WAVE => {
                    self.echo_complete = self.echo_length;
                    self.data.n_mix_wavefile = 0;
                    if options.klatt {
                        host.klatt_reset();
                    }
                    result = self.play_wave(
                        host,
                        length,
                        resume,
                        q[2] as usize,
                        (q[3] & 0xff) as i32,
                        (q[3] >> 8) as i32,
                    );
                }
                WCMD_WAVE2 => {
                    // a wave played along with synthesis
                    let d = &mut self.data;
                    d.mix_wave_amp = (q[3] >> 8) as i32;
                    d.mix_wave_scale = (q[3] & 0xff) as i32;
                    d.n_mix_wavefile = length & 0xffff;
                    d.mix_wavefile_max = (length >> 16) & 0xffff;
                    if d.mix_wave_scale == 0 {
                        d.n_mix_wavefile *= 2;
                        d.mix_wavefile_max *= 2;
                    }
                    d.mix_wavefile_ix = 0;
                    d.mix_wavefile_offset = 0;
                    d.mix_wavefile = q[2] as usize;
                }
                WCMD_SPECT | WCMD_SPECT2 => {
                    if q[0] & 0xff == WCMD_SPECT2 {
                        self.data.n_mix_wavefile = 0; // stop a concurrent wave
                    }
                    self.echo_complete = self.echo_length;
                    result = self.wavegen(
                        host,
                        options,
                        length & 0xffff,
                        (q[1] >> 16) as i32,
                        resume,
                        q[2] as usize,
                        q[3] as usize,
                    );
                }
                WCMD_KLATT | WCMD_KLATT2 if options.klatt => {
                    if q[0] & 0xff == WCMD_KLATT2 {
                        self.data.n_mix_wavefile = 0;
                    }
                    self.echo_complete = self.echo_length;
                    let (data, voice) = self.klatt_parts();
                    result = host.klatt(
                        length & 0xffff,
                        resume,
                        q[2] as usize,
                        q[3] as usize,
                        data,
                        voice,
                    );
                }
                WCMD_MARKER => host.marker(head),
                WCMD_AMPLITUDE => self.set_amplitude(length, q[2] as usize, q[3] as i32),
                WCMD_VOICE => {
                    if let Some(voice) = host.voice(q[2] as usize) {
                        self.set_voice(host, &voice);
                    }
                    host.free_voice(q[2] as usize);
                }
                WCMD_EMBEDDED => self.set_embedded(host, q[1] as i32, q[2] as i32),
                WCMD_MBROLA_DATA if options.mbrola => {
                    if let Some(voice) = self.voice.as_ref() {
                        let amp = self.general_amplitude.wrapping_mul(voice.voicing) / 64;
                        result = host.mbrola(length, resume, amp);
                    }
                }
                WCMD_FMT_AMPLITUDE => {
                    self.data.amplitude_fmt = q[1] as i32;
                    if self.data.amplitude_fmt == 0 {
                        self.data.amplitude_fmt = 100; // percentage; 0 means 100%
                    }
                }
                WCMD_SONIC_SPEED if options.sonic => host.sonic_speed(head),
                _ => {}
            }
            if result == 0 {
                host.advance_head();
                self.resume = false;
            } else {
                self.resume = true;
            }
        }
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Memory {
        samplerate: i32,
        embedded: [i32; N_EMBEDDED_VALUES],
        queue: Vec<[isize; 4]>,
        head: i32,
        out: Vec<i16>,
        room: isize,
        echo: Vec<i32>,
        echo_head: i32,
        echo_tail: i32,
        echo_amp: i32,
        bytes: Vec<u8>,
        frames: Vec<Frame>,
        events: Vec<(Hook, i32)>,
        markers: Vec<i32>,
    }
    impl Memory {
        fn new() -> Self {
            Self {
                samplerate: 0,
                embedded: [0; N_EMBEDDED_VALUES],
                queue: Vec::new(),
                head: 0,
                out: Vec::new(),
                room: 0,
                echo: vec![0; N_ECHO_BUF as usize],
                echo_head: 0,
                echo_tail: 0,
                echo_amp: 0,
                bytes: vec![128; 256],
                frames: Vec::new(),
                events: Vec::new(),
                markers: Vec::new(),
            }
        }
    }
    impl Host for Memory {
        fn samplerate(&mut self) -> i32 {
            self.samplerate
        }
        fn set_samplerate(&mut self, rate: i32) {
            self.samplerate = rate;
        }
        fn embedded(&mut self, index: usize) -> i32 {
            self.embedded[index]
        }
        fn set_embedded(&mut self, index: usize, value: i32) {
            self.embedded[index] = value;
        }
        fn head(&mut self) -> i32 {
            self.head
        }
        fn tail(&mut self) -> i32 {
            self.queue.len() as i32
        }
        fn command(&mut self, index: i32) -> [isize; 4] {
            self.queue[index as usize]
        }
        fn advance_head(&mut self) {
            self.head += 1;
        }
        fn room(&mut self) -> isize {
            self.room
        }
        fn write(&mut self, sample: i32) {
            self.out.push(sample as i16);
            self.room -= 2;
        }
        fn echo_take(&mut self) -> i32 {
            let value = self.echo[self.echo_tail as usize];
            self.echo_tail = (self.echo_tail + 1) % N_ECHO_BUF;
            value
        }
        fn echo_put(&mut self, sample: i32) {
            self.echo[self.echo_head as usize] = i32::from(sample as i16);
            self.echo_head = (self.echo_head + 1) % N_ECHO_BUF;
        }
        fn echo_amp(&mut self) -> i32 {
            self.echo_amp
        }
        fn echo_reset(&mut self, head: i32, amp: i32) {
            self.echo.fill(0);
            (self.echo_head, self.echo_tail, self.echo_amp) = (head, 0, amp);
        }
        fn byte(&mut self, address: usize, offset: i32) -> u8 {
            self.bytes[address - 1 + offset as usize]
        }
        fn frame(&mut self, address: usize) -> Frame {
            self.frames[address - 1]
        }
        fn voice(&mut self, _address: usize) -> Option<Voice> {
            None
        }
        fn free_voice(&mut self, _address: usize) {}
        fn hook(&mut self, hook: Hook, sample: i32) {
            self.events.push((hook, sample));
        }
        fn marker(&mut self, index: i32) {
            self.markers.push(index);
        }
        fn alignment(&mut self, _index: i32) {}
        fn samplerate_event(&mut self, rate: i32) {
            self.markers.push(-rate);
        }
        fn sonic_speed(&mut self, _index: i32) {}
        fn random(&mut self, min: i32, _max: i32) -> i32 {
            min
        }
    }

    fn voice() -> Voice {
        Voice {
            pitch_base: 0x47000,
            pitch_range: 4104,
            flutter: 64,
            harmonic_peaks: 5,
            voicing: 64,
            consonant_amplitude: 90,
            voiced_consonant_amplitude: 100,
            sample_rate: 22050,
            frequency: [256; 9],
            height: [256; 9],
            width: [256; 9],
            base_frequency: [256; 9],
            base_height: [256; 9],
            tone: [128; 1000],
            ..Voice::default()
        }
    }

    fn generator(memory: &mut Memory) -> Wavegen {
        let mut wavegen = Wavegen::default();
        wavegen.init(memory, 22050, 0);
        wavegen.init_breath(22050);
        wavegen.set_voice(memory, &voice());
        wavegen
    }

    #[test]
    fn integer_helpers_follow_c() {
        assert_eq!(with_range0(-3, 10), 0);
        assert_eq!(with_range0(11, 10), 10);
        assert_eq!(with_range0(7, 10), 7);
        assert_eq!(div(7, 0), 0);
        assert_eq!(div(i32::MIN, -1), i32::MIN);
        assert_eq!(div(-7, 2), -3);
        assert_eq!(to_int(-2.9), -2);
        if cfg!(target_arch = "x86_64") {
            assert_eq!(to_int(3e9), i32::MIN);
            assert_eq!(to_int(f64::NAN), i32::MIN);
        } else {
            assert_eq!(to_int(3e9), i32::MAX);
        }
    }

    #[test]
    fn init_sets_defaults_and_rate_table() {
        let mut memory = Memory::new();
        let mut wavegen = Wavegen::default();
        wavegen.init(&mut memory, 16000, 0);
        assert_eq!(memory.samplerate, 16000);
        assert_eq!(memory.embedded, EMBEDDED_DEFAULT);
        assert_eq!(wavegen.phase_inc_factor, 0x8000000 / 16000);
        assert_eq!(wavegen.wavemult_max, 16000 * 60 / (256 * 50));
        assert_eq!(wavegen.wavemult[0], 0);
        assert!(wavegen.wavemult[wavegen.wavemult_offset as usize] > 250);
        // 22050 Hz keeps the preset table, including entries past the window
        let mut preset = Wavegen::default();
        preset.init(&mut memory, 22050, 0);
        assert_eq!(preset.wavemult, WAVEMULT);
    }

    #[test]
    fn embedded_values_clamp_and_adjust() {
        let mut memory = Memory::new();
        let mut wavegen = generator(&mut memory);
        // relative +, relative -, then absolute past the maximum
        wavegen.set_embedded(&mut memory, 0x40 | EMBED_A as i32, 20);
        assert_eq!(memory.embedded[EMBED_A], 120);
        wavegen.set_embedded(&mut memory, 0x60 | EMBED_A as i32, 500);
        assert_eq!(memory.embedded[EMBED_A], 0);
        assert_eq!(wavegen.general_amplitude, 0);
        wavegen.set_embedded(&mut memory, EMBED_H as i32, 1000);
        assert_eq!(memory.embedded[EMBED_H], 99);
        assert_eq!(
            (memory.echo_amp, memory.echo_head),
            (99, 130 * 22050 / 1000)
        );
        assert_eq!(wavegen.echo_length, memory.echo_head * 2);
        // a command past the values changes nothing
        let before = memory.embedded;
        wavegen.set_embedded(&mut memory, 0x1f, 5);
        assert_eq!(memory.embedded, before);
    }

    #[test]
    fn harmonics_guard_c_traps() {
        let mut memory = Memory::new();
        let mut wavegen = Wavegen::default();
        let peaks = [Peak::default(); N_PEAKS];
        let mut htab = [7; MAX_HARMONIC];
        // without a voice: 1 and an untouched table
        assert_eq!(
            wavegen.peaks_to_harmspect(22050, &peaks, 100 << 16, &mut htab, 0),
            1
        );
        assert_eq!(htab, [7; MAX_HARMONIC]);
        wavegen = generator(&mut memory);
        assert_eq!(
            wavegen.peaks_to_harmspect(22050, &peaks, 0, &mut htab, 0),
            0
        );
        // a zero-width peak skips its shape instead of dividing by zero
        let mut peaks = [Peak::default(); N_PEAKS];
        peaks[1] = Peak {
            freq: 500 << 16,
            height: 1 << 20,
            ..Peak::default()
        };
        let hmax = wavegen.peaks_to_harmspect(22050, &peaks, 100 << 16, &mut htab, 0);
        assert_eq!(hmax, 0);
        // a low bass height ends its ramp instead of looping
        peaks[1].left = 50 << 16;
        peaks[1].right = 50 << 16;
        peaks[1].height = 1;
        peaks[5].freq = 3000 << 16;
        assert!(wavegen.peaks_to_harmspect(22050, &peaks, 100 << 16, &mut htab, 0) > 0);
    }

    #[test]
    fn empty_queue_and_pause_with_echo() {
        let mut memory = Memory::new();
        let mut wavegen = generator(&mut memory);
        memory.room = 100;
        assert_eq!(wavegen.fill(&mut memory, &Options::default(), 1), 1);
        assert!(memory.out.is_empty());
        // a pause plays the echo ring, scaled, and feeds it back
        memory.echo_amp = 128;
        memory.echo[0] = 1000;
        memory.echo_head = 3;
        memory.queue.push([WCMD_PAUSE, 4, 0, 0]);
        assert_eq!(wavegen.fill(&mut memory, &Options::default(), 1), 1);
        assert_eq!(memory.out, [500, 0, 0, 250]);
        assert_eq!(memory.head, 1);
        assert_eq!(memory.events.len(), 4);
        assert!(memory.events.iter().all(|&(hook, _)| hook == Hook::Silence));
    }

    #[test]
    fn full_buffer_resumes_where_it_stopped() {
        let mut memory = Memory::new();
        let mut wavegen = generator(&mut memory);
        memory.bytes = (0..=255).collect();
        // 8-bit wave, scale 1, amplitude 32
        memory.queue.push([WCMD_WAVE, 6, 1, 1 | (32 << 8)]);
        memory.queue.push([WCMD_MARKER | (3 << 8), 0, 0, 0]);
        memory.room = 6;
        assert_eq!(wavegen.fill(&mut memory, &Options::default(), 1), 0);
        assert_eq!((memory.out.len(), memory.head), (3, 0));
        memory.room = 100;
        assert_eq!(wavegen.fill(&mut memory, &Options::default(), 1), 1);
        assert_eq!((memory.out.len(), memory.head), (6, 2));
        assert_eq!(memory.markers, [-22050, 1]); // the voice's sample rate, then the marker
        let consonant = wavegen.consonant_amp * wavegen.general_amplitude;
        let expected: Vec<i16> = (0..6)
            .map(|ix| (((ix * consonant) >> 10) * 32 / 32) as i16)
            .collect();
        assert_eq!(memory.out, expected);
    }

    #[test]
    fn spectrum_segment_synthesizes_its_length() {
        let mut memory = Memory::new();
        let mut wavegen = generator(&mut memory);
        let frame = Frame {
            frequencies: [700, 1200, 2500, 3500, 4000, 4500, 5000],
            heights: [40; 8],
            widths: [100; 6],
            right: [100; 3],
            ..Frame::default()
        };
        memory.frames = vec![frame, frame];
        memory.queue.push([WCMD_PITCH, 2000, 0, (40 << 16) | 60]);
        memory.queue.push([WCMD_AMPLITUDE, 2000, 0, 60]);
        memory.queue.push([WCMD_SPECT, 640, 1, 2]);
        memory.room = 1 << 20;
        assert_eq!(wavegen.fill(&mut memory, &Options::default(), 1), 1);
        // the segment runs to the end of the cycle after its 640 samples
        assert!(memory.out.len() >= 640);
        assert!(memory.out.iter().any(|&sample| sample != 0));
        assert!(memory.events.iter().all(|&(hook, _)| hook == Hook::Voiced));
        assert_eq!(wavegen.data.pitch_env, 1);
    }
}
