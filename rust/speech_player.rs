//! Owned speechPlayer frame queue and complete cascade/parallel DSP.
// Derived from NV Speech Player, Copyright 2014 NV Access Limited.
// SPDX-License-Identifier: GPL-3.0-or-later

pub const PARAMETERS: usize = 47;
pub const QUEUE_CAPACITY: usize = 256;
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub values: [f64; PARAMETERS],
}
impl Default for Frame {
    fn default() -> Self {
        Self {
            values: [0.0; PARAMETERS],
        }
    }
}
fn fade(old: f64, new: f64, ratio: f64) -> f64 {
    if new.is_nan() {
        old
    } else {
        old + ((new - old) * ratio)
    }
}
#[derive(Clone, Copy)]
struct Request {
    minimum: u32,
    fade: u32,
    null: bool,
    frame: Frame,
    pitch_increment: f64,
    index: i32,
}
impl Request {
    const EMPTY: Self = Self {
        minimum: 0,
        fade: 0,
        null: true,
        frame: Frame {
            values: [0.0; PARAMETERS],
        },
        pitch_increment: 0.0,
        index: 0,
    };
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Invalid,
    Full,
}

pub struct Frames {
    requests: [Request; QUEUE_CAPACITY],
    head: usize,
    count: usize,
    old: Request,
    new: Option<Request>,
    current: Frame,
    null: bool,
    counter: u32,
    last_index: i32,
}
impl Default for Frames {
    fn default() -> Self {
        Self {
            requests: [Request::EMPTY; QUEUE_CAPACITY],
            head: 0,
            count: 0,
            old: Request::EMPTY,
            new: None,
            current: Frame::default(),
            null: true,
            counter: 0,
            last_index: -1,
        }
    }
}
impl Frames {
    pub fn clear(&mut self) {
        self.head = 0;
        self.count = 0;
        self.old = Request::EMPTY;
        self.new = None;
        self.current = Frame::default();
        self.null = true;
        self.counter = 0;
        self.last_index = -1;
    }
    pub fn last_index(&self) -> i32 {
        self.last_index
    }
    pub fn pending(&self) -> usize {
        self.count
    }
    pub fn queue(
        &mut self,
        frame: Option<Frame>,
        minimum: u32,
        duration: u32,
        index: i32,
        purge: bool,
    ) -> Result<(), Error> {
        // A non-null zero-duration frame produces infinite pitch increments in
        // the old backend and undefined integer conversion in synthesis.
        if frame.is_some() && minimum == 0 {
            return Err(Error::Invalid);
        }
        if !purge && self.count == QUEUE_CAPACITY {
            return Err(Error::Full);
        }
        let request = Request {
            minimum,
            fade: duration.max(1),
            null: frame.is_none(),
            frame: frame.unwrap_or_default(),
            pitch_increment: frame
                .map_or(0.0, |f| (f.values[46] - f.values[0]) / f64::from(minimum)),
            index,
        };
        if purge {
            self.head = 0;
            self.count = 0;
            self.counter = self.old.minimum;
            if let Some(next) = self.new.take() {
                self.old.null = next.null;
                self.old.frame = self.current;
            }
        }
        self.requests[(self.head + self.count) % QUEUE_CAPACITY] = request;
        self.count += 1;
        Ok(())
    }
    pub fn advance(&mut self) -> Option<&Frame> {
        self.counter = self.counter.wrapping_add(1);
        if let Some(next) = self.new {
            if self.counter > next.fade {
                self.old = next;
                self.new = None;
            } else {
                let ratio = f64::from(self.counter) / f64::from(next.fade);
                for i in 0..PARAMETERS {
                    self.current.values[i] =
                        fade(self.old.frame.values[i], next.frame.values[i], ratio);
                }
            }
        } else if self.counter > self.old.minimum {
            if self.count > 0 {
                self.null = false;
                let mut next = self.requests[self.head];
                self.head = (self.head + 1) % QUEUE_CAPACITY;
                self.count -= 1;
                if next.null {
                    next.frame = self.old.frame;
                    next.frame.values[44] = 0.0;
                    next.frame.values[0] = self.current.values[0];
                    next.pitch_increment = 0.0;
                } else if self.old.null {
                    self.old.frame = next.frame;
                    self.old.frame.values[44] = 0.0;
                }
                if next.index != -1 {
                    self.last_index = next.index;
                }
                self.counter = 0;
                next.frame.values[0] += next.pitch_increment * f64::from(next.fade);
                self.new = Some(next);
            } else {
                self.null = true;
            }
        } else {
            self.current.values[0] += self.old.pitch_increment;
            self.old.frame.values[0] = self.current.values[0];
        }
        if self.null {
            None
        } else {
            Some(&self.current)
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Resonator {
    frequency: f64,
    bandwidth: f64,
    set: bool,
    a: f64,
    b: f64,
    c: f64,
    p1: f64,
    p2: f64,
}
impl Resonator {
    fn step(&mut self, rate: f64, anti: bool, input: f64, frequency: f64, bandwidth: f64) -> f64 {
        if !self.set || frequency != self.frequency || bandwidth != self.bandwidth {
            self.frequency = frequency;
            self.bandwidth = bandwidth;
            let r = (-std::f64::consts::PI / rate * bandwidth).exp();
            self.c = -(r * r);
            self.b = r * (std::f64::consts::TAU / rate * -frequency).cos() * 2.0;
            self.a = 1.0 - self.b - self.c;
            if anti && frequency != 0.0 {
                self.a = 1.0 / self.a;
                self.c *= -self.a;
                self.b *= -self.a;
            }
            self.set = true;
        }
        let output = self.a * input + self.b * self.p1 + self.c * self.p2;
        self.p2 = self.p1;
        self.p1 = if anti { input } else { output };
        output
    }
}
pub struct Generator {
    rate: f64,
    pitch: f64,
    vibrato: f64,
    aspiration: f64,
    frication: f64,
    cascade: [Resonator; 8],
    parallel: [Resonator; 6],
}
impl Generator {
    pub fn new(rate: i32) -> Result<Self, Error> {
        if rate <= 0 {
            return Err(Error::Invalid);
        }
        Ok(Self {
            rate: f64::from(rate),
            pitch: 0.0,
            vibrato: 0.0,
            aspiration: 0.0,
            frication: 0.0,
            cascade: [Resonator::default(); 8],
            parallel: [Resonator::default(); 6],
        })
    }
    fn sample(&mut self, frame: &Frame, random: &mut impl FnMut() -> f64) -> i16 {
        let f = &frame.values;
        self.vibrato = ((f[2] / self.rate) + self.vibrato) % 1.0;
        let vibrato = (self.vibrato * std::f64::consts::TAU).sin() * 0.06 * f[1] + 1.0;
        self.pitch = ((f[0] * vibrato / self.rate) + self.pitch) % 1.0;
        self.aspiration = random() + 0.75 * self.aspiration;
        let aspiration = self.aspiration * 0.2;
        let mut turbulence = aspiration * f[3];
        let glottis_open = self.pitch >= f[4];
        if !glottis_open {
            turbulence *= 0.01;
        }
        let mut voice = (self.pitch * 2.0) - 1.0;
        voice += turbulence;
        voice *= f[5];
        let input = (aspiration * f[6] + voice) * f[44] / 2.0;
        let nasal = self.cascade[6].step(self.rate, true, input, f[13], f[21]);
        let pole = self.cascade[7].step(self.rate, false, nasal, f[14], f[22]);
        let mut output = fade(input, pole, f[23]);
        for i in (0..6).rev() {
            output = self.cascade[i].step(self.rate, false, output, f[7 + i], f[15 + i]);
        }
        self.frication = random() + 0.75 * self.frication;
        let frication = self.frication * 0.3 * f[24] * f[44] / 2.0;
        let mut parallel = 0.0;
        for i in 0..6 {
            parallel += (self.parallel[i].step(self.rate, false, frication, f[25 + i], f[31 + i])
                - frication)
                * f[37 + i];
        }
        parallel = fade(parallel, frication, f[43]);
        // The old helpers take int, so truncation precedes their 32000 clamp.
        (((output + parallel) * f[45] * 4000.0) as i32).clamp(-32000, 32000) as i16
    }
}
pub struct Player {
    pub frames: Frames,
    generator: Generator,
}
impl Player {
    pub fn reset(&mut self, rate: i32) -> Result<(), Error> {
        self.generator = Generator::new(rate)?;
        self.frames.clear();
        Ok(())
    }
    pub fn new(rate: i32) -> Result<Self, Error> {
        Ok(Self {
            frames: Frames::default(),
            generator: Generator::new(rate)?,
        })
    }
    pub fn synthesize(&mut self, output: &mut [i16], random: &mut impl FnMut() -> f64) -> usize {
        for (i, sample) in output.iter_mut().enumerate() {
            let Some(frame) = self.frames.advance() else {
                return i;
            };
            *sample = self.generator.sample(frame, random);
        }
        output.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queue_capacity_refuses_without_mutation_and_purge_reuses_slots() {
        let mut frames = Frames::default();
        let storage = frames.requests.as_ptr();
        for i in 0..QUEUE_CAPACITY {
            frames.queue(None, 1, 0, i as i32, false).unwrap();
        }
        assert_eq!(frames.queue(None, 1, 0, 9, false), Err(Error::Full));
        assert_eq!(frames.pending(), QUEUE_CAPACITY);
        assert_eq!(frames.last_index(), -1);
        frames.queue(None, 1, 0, 41, true).unwrap();
        assert_eq!(frames.pending(), 1);
        assert!(frames.advance().is_some());
        assert_eq!(frames.last_index(), 41);
        assert_eq!(frames.requests.as_ptr(), storage);
    }
    #[test]
    fn interpolation_preserves_nan_fields_and_initial_zero_frame() {
        let mut frames = Frames::default();
        let mut frame = Frame::default();
        frame.values[0] = 100.0;
        frame.values[46] = 120.0;
        frame.values[44] = 1.0;
        frames.queue(Some(frame), 4, 2, 7, false).unwrap();
        assert_eq!(frames.advance().unwrap().values[44], 0.0);
        assert_eq!(frames.advance().unwrap().values[44], 0.5);
        let mut next = frame;
        next.values[5] = f64::NAN;
        frames.queue(Some(next), 3, 0, 8, true).unwrap();
        for _ in 0..6 {
            if let Some(f) = frames.advance() {
                assert!(!f.values[5].is_nan());
            }
        }
        assert_eq!(frames.last_index(), 8);
    }
    #[test]
    fn idle_does_not_draw_random_values_or_overwrite_output_tail() {
        let mut player = Player::new(22050).unwrap();
        let mut samples = [71; 12];
        let mut calls = 0;
        let mut random = || {
            calls += 1;
            0.5
        };
        assert_eq!(player.synthesize(&mut samples, &mut random), 0);
        assert_eq!(calls, 0);
        assert_eq!(samples, [71; 12]);
        player.frames.queue(None, 1, 1, 1, false).unwrap();
        let n = player.synthesize(&mut samples, &mut || 0.5);
        assert!(n > 0 && n < samples.len());
        assert!(samples[n..].iter().all(|s| *s == 71));
    }
}
