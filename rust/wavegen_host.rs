//! Native wave-generator effects and Sonic output/stop policy.
// SPDX-License-Identifier: GPL-3.0-or-later
use std::mem::size_of;

#[derive(Clone, Copy, Default)]
pub struct Features {
    pub klatt: bool,
    pub mbrola: bool,
    pub sonic: bool,
    pub big_endian: bool,
}
#[derive(Clone, Copy, Default)]
pub struct Effect {
    pub op: i32,
    pub index: i32,
    pub a: i32,
    pub b: i32,
    pub c: i32,
    pub value: usize,
    pub value2: usize,
    pub data: usize,
    pub voice: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Invalid;

/// Every view is acquired afresh. No engine/storage loan spans a primitive.
/// Addresses refer to the serialized host's initialized bounded storage; the
/// host checks selected callback and cursor admission before dereferencing it.
pub trait Host {
    fn queue(&mut self, index: usize) -> [isize; 4];
    fn output(&mut self) -> (usize, usize);
    fn publish_output(&mut self, pointer: usize);
    fn hook(&mut self, kind: i32, sample: i16) -> Result<(), Invalid>;
    fn alignment(&mut self, pointer: usize, kind: i32) -> Result<(), Invalid>;
    fn marker(&mut self, kind: i32, position: u32, value: i32, value2: i32, pointer: usize);
    fn release(&mut self, pointer: usize);
    fn random(&mut self, minimum: i32, maximum: i32) -> i32;
    fn klatt_reset(&mut self);
    fn klatt(&mut self, effect: Effect) -> i32;
    fn mbrola(&mut self, length: i32, resume: i32, amplitude: i32) -> i32;
    fn fill(&mut self) -> i32;
    fn rescale(&mut self, before: i32, after: i32);
    fn stop_queue(&mut self);
    fn mbrola_active(&mut self) -> bool;
    fn mbrola_reset(&mut self);
    fn speed(&mut self) -> f64;
    fn set_speed(&mut self, speed: f64);
    fn stream(&mut self) -> usize;
    fn set_stream(&mut self, stream: usize);
    fn sonic_create(&mut self, rate: i32, channels: i32) -> usize;
    fn sonic_destroy(&mut self, stream: usize);
    fn sonic_flush(&mut self, stream: usize);
    fn sonic_speed(&mut self, stream: usize) -> f32;
    fn sonic_set_speed(&mut self, stream: usize, speed: f32);
    fn sonic_write(&mut self, stream: usize, pointer: usize, samples: i32);
    fn sonic_read(&mut self, stream: usize, pointer: usize, samples: i32) -> i32;
    fn transcode(&mut self, pointer: usize, samples: i32, to_native: bool);
}

fn extent(start: usize, end: usize) -> Result<i32, Invalid> {
    i32::try_from(end.checked_sub(start).ok_or(Invalid)?).map_err(|_| Invalid)
}
fn read(host: &mut impl Host, pointer: usize, samples: i32) -> Result<i32, Invalid> {
    let stream = host.stream();
    if stream == 0 || samples < 0 || pointer == 0 {
        return Err(Invalid);
    }
    let count = host.sonic_read(stream, pointer, samples);
    if !(0..=samples).contains(&count) {
        return Err(Invalid);
    }
    Ok(count)
}

pub fn effect(host: &mut impl Host, features: Features, effect: Effect) -> Result<i32, Invalid> {
    let index = if (0..170).contains(&effect.index) {
        effect.index as usize
    } else {
        0
    };
    match effect.op {
        1 => host.hook(effect.a, effect.b as i16)?,
        2 => {
            let queue = host.queue(index);
            // C views two native ints starting at q[2], spanning q[3] on
            // 32-bit hosts. Copy bytes rather than aliasing integer types.
            let width = size_of::<isize>();
            let mut packed = [0; 16];
            packed[..width].copy_from_slice(&queue[2].to_ne_bytes());
            packed[width..width * 2].copy_from_slice(&queue[3].to_ne_bytes());
            let value = i32::from_ne_bytes(packed[..4].try_into().expect("fixed extent"));
            let value2 = i32::from_ne_bytes(packed[4..8].try_into().expect("fixed extent"));
            let (pointer, _) = host.output();
            host.marker(
                (queue[0] >> 8) as i32,
                queue[1] as u32,
                value,
                value2,
                pointer,
            );
        }
        3 => {
            let queue = host.queue(index);
            let pointer = queue[1] as usize;
            host.alignment(pointer, queue[2] as i32)?;
            host.release(pointer);
        }
        4 => {
            let (pointer, _) = host.output();
            host.marker(8, 0, effect.a, 0, pointer);
        }
        5 if features.sonic => {
            let queue = host.queue(index);
            host.set_speed(queue[1] as f64 / 1024.0);
            let stream = host.stream();
            if stream != 0 && host.speed() <= 1.0 {
                host.sonic_flush(stream);
                let (pointer, end) = host.output();
                let samples = read(host, pointer, extent(pointer, end)? / 2)?;
                if features.big_endian {
                    // Preserve the preceding controller's selected half-count.
                    host.transcode(pointer, samples / 2, false);
                }
                let (fresh, end) = host.output();
                let written = samples as usize * 2;
                if written > extent(fresh, end)? as usize {
                    return Err(Invalid);
                }
                host.publish_output(fresh.checked_add(written).ok_or(Invalid)?);
            }
        }
        6 => return Ok(host.random(effect.a, effect.b)),
        7 => host.release(effect.value),
        8 if features.klatt => host.klatt_reset(),
        9 if features.klatt => return Ok(host.klatt(effect)),
        10 if features.mbrola => return Ok(host.mbrola(effect.a, effect.b, effect.c)),
        _ => {}
    }
    Ok(0)
}

fn speed_up(
    host: &mut impl Host,
    features: Features,
    pointer: usize,
    input: i32,
    capacity: i32,
    ended: i32,
) -> Result<i32, Invalid> {
    if input > 0 {
        if host.stream() == 0 {
            let stream = host.sonic_create(22050, 1);
            host.set_stream(stream);
            if stream == 0 {
                return Err(Invalid);
            }
        }
        let stream = host.stream();
        if f64::from(host.sonic_speed(stream)) != host.speed() {
            let speed = host.speed() as f32;
            let stream = host.stream();
            host.sonic_set_speed(stream, speed);
        }
        if features.big_endian {
            host.transcode(pointer, input, true);
        }
        let stream = host.stream();
        host.sonic_write(stream, pointer, input);
    }
    if host.stream() == 0 {
        return Ok(0);
    }
    if ended != 0 {
        let stream = host.stream();
        host.sonic_flush(stream);
    }
    let result = read(host, pointer, capacity)?;
    if features.big_endian {
        // The historical SpeedUp converts its complete output capacity,
        // including initialized unwritten tail samples.
        host.transcode(pointer, capacity, false);
    }
    Ok(result)
}

pub fn fill(host: &mut impl Host, features: Features) -> Result<i32, Invalid> {
    let (start, _) = host.output();
    let mut finished = host.fill();
    if features.sonic && host.speed() > 1.0 {
        let (pointer, end) = host.output();
        let capacity = extent(start, end)?;
        let input = extent(start, pointer)? / 2;
        if pointer > end {
            return Err(Invalid);
        }
        let samples = speed_up(host, features, start, input, capacity / 2, finished)?;
        let bytes = samples as usize * 2;
        host.publish_output(start.checked_add(bytes).ok_or(Invalid)?);
        host.rescale(input, samples);
        if bytes >= capacity as usize {
            finished = 0;
        }
    }
    Ok(finished)
}

pub fn stop(host: &mut impl Host, features: Features) {
    host.stop_queue();
    if features.sonic {
        let stream = host.stream();
        if stream != 0 {
            host.sonic_destroy(stream);
            host.set_stream(0);
        }
    }
    if features.mbrola && host.mbrola_active() {
        host.mbrola_reset();
    }
}
