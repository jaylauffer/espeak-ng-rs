//! Bounded synthesis-ring smoothing over reusable frame storage.
// Copyright (C) 2005-2014 Jonathan Duddington, 2015-2017 Reece H. Dunn;
// Rust adaptation (C) 2026. SPDX-License-Identifier: GPL-3.0-or-later

use crate::{
    formant::{copy_frame, Frame, Storage, MAX_POOL_FRAMES},
    phoneme_data::InvalidPhonemeData as Error,
};

/// Same four-word layout as the compatibility synthesis queue. Only spectrum
/// commands (kind <= 4) carry frame handles; other payloads remain untouched.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Command<T> {
    pub kind: isize,
    pub length: isize,
    pub start: T,
    pub end: T,
}
#[derive(Clone, Copy, Debug)]
pub struct Syllable {
    pub start: usize,
    pub end: usize,
    pub centre: Option<usize>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Virtual<T> {
    Original(T),
    Planned(usize),
}
#[derive(Clone, Copy)]
struct Edit<T> {
    original: Option<T>,
    frame: Frame,
}
/// Initialize once per engine. No allocation, I/O or scheduling during smoothing.
pub struct Workspace<T> {
    queue: [Command<Virtual<T>>; MAX_POOL_FRAMES],
    edits: [Edit<T>; MAX_POOL_FRAMES],
    handles: [Option<T>; MAX_POOL_FRAMES],
}
impl<T: Copy> Workspace<T> {
    pub fn new(seed: T) -> Self {
        Self {
            queue: [Command {
                kind: 0,
                length: 0,
                start: Virtual::Original(seed),
                end: Virtual::Original(seed),
            }; MAX_POOL_FRAMES],
            edits: [Edit {
                original: None,
                frame: Frame::default(),
            }; MAX_POOL_FRAMES],
            handles: [None; MAX_POOL_FRAMES],
        }
    }
}
struct Plan<'a, T, S> {
    source: &'a S,
    edits: &'a mut [Edit<T>],
    count: usize,
}
impl<T: Copy + Eq, S: Storage<T>> Plan<'_, T, S> {
    fn original(&self, handle: T) -> Option<usize> {
        self.edits[..self.count]
            .iter()
            .position(|edit| edit.original == Some(handle))
    }
    fn append(&mut self, original: Option<T>, frame: Frame) -> Result<usize, Error> {
        let index = self.count;
        let slot = self
            .edits
            .get_mut(index)
            .ok_or(Error("smoothing edit capacity exceeded"))?;
        *slot = Edit { original, frame };
        self.count += 1;
        Ok(index)
    }
}
impl<T: Copy + Eq, S: Storage<T>> Storage<Virtual<T>> for Plan<'_, T, S> {
    fn read(&self, handle: Virtual<T>) -> Result<Frame, Error> {
        match handle {
            Virtual::Original(handle) => match self.original(handle) {
                Some(index) => Ok(self.edits[index].frame),
                None => self.source.read(handle),
            },
            Virtual::Planned(index) => self
                .edits
                .get(index)
                .filter(|_| index < self.count)
                .map(|edit| edit.frame)
                .ok_or(Error("invalid planned frame")),
        }
    }
    fn writable(&self, handle: Virtual<T>) -> bool {
        match handle {
            Virtual::Original(handle) => self.source.writable(handle),
            Virtual::Planned(index) => index < self.count,
        }
    }
    fn write(&mut self, handle: Virtual<T>, frame: Frame) -> Result<(), Error> {
        let index = match handle {
            Virtual::Original(handle) => match self.original(handle) {
                Some(index) => index,
                None => self.append(Some(handle), frame)?,
            },
            Virtual::Planned(index) if index < self.count => index,
            _ => return Err(Error("invalid planned frame")),
        };
        self.edits[index].frame = frame;
        Ok(())
    }
    fn allocate(&mut self, frame: Frame) -> Result<Virtual<T>, Error> {
        self.append(None, frame).map(Virtual::Planned)
    }
}

/// Plan against immutable snapshots, reserve actual new copies, then commit.
/// Invalid rings, arithmetic and admission failures leave queue/storage intact.
/// After successful admission, storage allocate/write must succeed; otherwise
/// the owner must discard the partially committed syllable and drain its handles.
pub fn smooth<T: Copy + Eq, S: Storage<T>>(
    storage: &mut S,
    queue: &mut [Command<T>],
    syllable: &mut Syllable,
    rates: &[i32; 6],
    workspace: &mut Workspace<T>,
) -> Result<(), Error> {
    let n = queue.len();
    if n == 0
        || n > MAX_POOL_FRAMES
        || syllable.start >= n
        || syllable.end >= n
        || syllable.centre.is_some_and(|centre| centre >= n)
    {
        return Err(Error("invalid smoothing ring bounds"));
    }
    if syllable.start == syllable.end {
        return Ok(());
    }
    let Some(centre) = syllable.centre.filter(|centre| *centre != syllable.start) else {
        syllable.start = syllable.end;
        return Ok(());
    };
    let span = (syllable.end + n - syllable.start) % n;
    if (centre + n - syllable.start) % n >= span {
        return Err(Error("smoothing centre outside syllable"));
    }
    for (output, input) in workspace.queue[..n].iter_mut().zip(queue.iter()) {
        *output = Command {
            kind: input.kind,
            length: input.length,
            start: Virtual::Original(input.start),
            end: Virtual::Original(input.end),
        };
    }
    let mut plan = Plan {
        source: &*storage,
        edits: &mut workspace.edits,
        count: 0,
    };
    run(&mut plan, &mut workspace.queue[..n], *syllable, rates)?;
    let count = plan.count;
    let needed = plan.edits[..count]
        .iter()
        .filter(|edit| edit.original.is_none())
        .count();
    storage.reserve(needed)?;
    for (index, edit) in workspace.edits[..count].iter().enumerate() {
        workspace.handles[index] = Some(match edit.original {
            Some(handle) => {
                storage.write(handle, edit.frame)?;
                handle
            }
            None => storage.allocate(edit.frame)?,
        });
    }
    let resolve = |handle| match handle {
        Virtual::Original(handle) => handle,
        Virtual::Planned(index) => workspace.handles[index].expect("committed smoothing frame"),
    };
    for (output, input) in queue.iter_mut().zip(workspace.queue[..n].iter()) {
        output.start = resolve(input.start);
        output.end = resolve(input.end);
    }
    syllable.start = syllable.end;
    Ok(())
}

fn limit<T: Copy, S: Storage<T>>(
    storage: &mut S,
    previous: T,
    target: T,
    length: i32,
    rates: &[i32; 6],
    low_break: bool,
) -> Result<T, Error> {
    let mut changed = target;
    let mut modified = false;
    for (peak, rate) in rates.iter().enumerate() {
        let frame = storage.read(target)?;
        if low_break && frame.flags & 8 != 0 && peak < 3 {
            continue;
        }
        let first = i32::from(storage.read(previous)?.frequencies[peak]);
        let second = i32::from(frame.frequencies[peak]);
        let difference = second - first;
        let frequency = if difference > 0 {
            first * 2 + second
        } else {
            first + second * 2
        };
        let allowed = frequency
            .checked_mul(*rate)
            .ok_or(Error("smoothing rate overflow"))?
            / 3000;
        let allowed = allowed
            .checked_mul(length)
            .ok_or(Error("smoothing duration overflow"))?
            / 256;
        let value = if difference > allowed {
            Some(first.checked_add(allowed))
        } else if difference < -allowed {
            Some(first.checked_sub(allowed))
        } else {
            None
        };
        if let Some(value) = value {
            let value = value.ok_or(Error("smoothing frequency overflow"))?;
            if !modified {
                changed = copy_frame(storage, target, false)?;
                modified = true;
            }
            let mut output = storage.read(changed)?;
            output.frequencies[peak] = value as i16;
            storage.write(changed, output)?;
        }
    }
    Ok(changed)
}
fn run<T: Copy + Eq, S: Storage<T>>(
    storage: &mut S,
    queue: &mut [Command<T>],
    syllable: Syllable,
    rates: &[i32; 6],
) -> Result<(), Error> {
    let n = queue.len();
    let centre = syllable.centre.ok_or(Error("missing smoothing centre"))?;
    let mut frame = queue[centre].start;
    let mut replacement = frame;
    let mut index = (centre + n - 1) % n;
    for _ in 0..n {
        let command = &mut queue[index];
        if command.kind == 5 || command.kind == 6 {
            break;
        }
        if command.kind <= 4 {
            if command.end != frame {
                break;
            }
            command.end = replacement;
            frame = command.start;
            replacement = frame;
            let flags = storage.read(frame)?.flags;
            if flags & 16 != 0 {
                break;
            }
            let mut length = (command.length & 0xffff) as i32;
            if flags & 32 != 0 {
                length = length * 12 / 10;
            }
            replacement = limit(storage, command.end, frame, length, rates, true)?;
            command.start = replacement;
        }
        if index == syllable.start {
            break;
        }
        index = (index + n - 1) % n;
    }
    index = centre;
    let mut previous = None;
    for _ in 0..n {
        let command = &mut queue[index];
        if command.kind == 5 || command.kind == 6 {
            break;
        }
        if command.kind <= 4 {
            if let Some(frame) = previous {
                if command.start != frame {
                    break;
                }
                command.start = replacement;
            }
            frame = command.end;
            let flags = storage.read(command.start)?.flags;
            if flags & 16 != 0 {
                break;
            }
            let mut length = (command.length & 0xffff) as i32;
            if flags & 32 != 0 {
                length = length * 6 / 5;
            }
            replacement = limit(storage, command.start, frame, length, rates, false)?;
            command.end = replacement;
            previous = Some(frame);
        }
        index = (index + 1) % n;
        if index == syllable.end {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formant::{Handle, Pool, ResidentPool, Settings};
    fn fixture() -> (Vec<u8>, [Command<Handle>; 5]) {
        let mut bytes = vec![0; 8];
        for frequency in [5000_i16, 5000, 1000, 5000, 5000] {
            let mut frame = [0; 44];
            for peak in 0..7 {
                frame[2 + peak * 2..4 + peak * 2].copy_from_slice(&frequency.to_le_bytes());
            }
            bytes.extend_from_slice(&frame);
        }
        let queue = std::array::from_fn(|i| Command {
            kind: 3,
            length: 256,
            start: Handle::Resident(8 + i * 44),
            end: Handle::Resident(8 + (i + 1).min(4) * 44),
        });
        (bytes, queue)
    }
    #[test]
    fn smoothing_reserves_actual_copies_and_preserves_links_until_consumed() {
        let (bytes, mut queue) = fixture();
        let mut pool = Pool::new(4).unwrap();
        let mut store = ResidentPool {
            bytes: &bytes,
            pool: &mut pool,
            settings: Settings::default(),
            effects: Default::default(),
        };
        let mut work = Workspace::new(Handle::default());
        let mut syllable = Syllable {
            start: 0,
            end: 4,
            centre: Some(2),
        };
        smooth(&mut store, &mut queue, &mut syllable, &[0; 6], &mut work).unwrap();
        assert_eq!(syllable.start, 4);
        assert_eq!(store.pool.available(), 0);
        for index in 0..4 {
            assert_eq!(
                store.read(queue[index].start).unwrap().frequencies[..6],
                [1000; 6]
            );
            assert_eq!(
                store.read(queue[index].end).unwrap().frequencies[..6],
                [1000; 6]
            );
            if index < 3 {
                assert_eq!(queue[index].end, queue[index + 1].start);
            }
        }
        let consumed = [queue[0].start, queue[1].start, queue[2].end, queue[3].end];
        // A saturated pool still accepts changes requiring no new copies.
        let mut frame = store.read(queue[0].start).unwrap();
        frame.frequencies = [5000; 7];
        store.write(queue[0].start, frame).unwrap();
        syllable.start = 0;
        smooth(&mut store, &mut queue, &mut syllable, &[0; 6], &mut work).unwrap();
        assert_eq!(store.pool.available(), 0);
        assert_eq!(
            store.read(queue[0].start).unwrap().frequencies[..6],
            [1000; 6]
        );
        for handle in consumed {
            store.pool.release(handle).unwrap();
        }
        assert_eq!(store.pool.available(), 4);
        assert_eq!(Frame::decode(&bytes[8..52]).unwrap().frequencies[0], 5000);
    }
    #[test]
    fn failed_admission_and_arithmetic_leave_existing_frames_and_ring_unchanged() {
        let (bytes, mut queue) = fixture();
        let mut pool = Pool::new(3).unwrap();
        let mut store = ResidentPool {
            bytes: &bytes,
            pool: &mut pool,
            settings: Settings::default(),
            effects: Default::default(),
        };
        // A queued writable frame would change in the backward pass, before
        // discovering the shortage for later copies in an unplanned algorithm.
        queue[0].start = copy_frame(&mut store, queue[0].start, false).unwrap();
        let original = queue;
        let retained = store.read(queue[0].start).unwrap();
        let mut work = Workspace::new(Handle::default());
        let mut syllable = Syllable {
            start: 0,
            end: 4,
            centre: Some(2),
        };
        assert!(smooth(&mut store, &mut queue, &mut syllable, &[0; 6], &mut work).is_err());
        assert_eq!(queue, original);
        assert_eq!(syllable.start, 0);
        assert_eq!(store.read(queue[0].start).unwrap(), retained);
        assert_eq!(store.pool.available(), 2);
        assert!(smooth(
            &mut store,
            &mut queue,
            &mut syllable,
            &[i32::MAX; 6],
            &mut work
        )
        .is_err());
        assert_eq!(queue, original);
        assert_eq!(store.read(queue[0].start).unwrap(), retained);
        syllable.centre = Some(4);
        assert!(smooth(&mut store, &mut queue, &mut syllable, &[0; 6], &mut work).is_err());
        assert_eq!(queue, original);
        syllable.centre = None;
        smooth(&mut store, &mut queue, &mut syllable, &[0; 6], &mut work).unwrap();
        assert_eq!(queue, original);
        assert_eq!(syllable.start, 4);
    }
}
