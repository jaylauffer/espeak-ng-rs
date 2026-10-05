//! SSML parameter-stack planning with bounded embedded command storage.
// Copyright (C) 2005-2015 Jonathan Duddington, 2015-2017 Reece H. Dunn,
// 2018 Juho Hiltunen. Rust migration (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fmt::{self, Write};
pub const PARAMETERS: usize = 15;
pub const STACK: usize = 20;
pub const COMMANDS: usize = 80;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Frame {
    pub kind: i32,
    pub values: [i32; PARAMETERS],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Bounds,
    Capacity,
}

/// Commit these effects only after admitting the complete command prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Effects {
    pub values: [i32; PARAMETERS],
    pub punctuation: i32,
    pub capitals: i32,
    pub length: u32,
    pub changed: u32,
    pub count: u32,
    pub commands: [u8; COMMANDS],
}
impl Write for Effects {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let start = self.length as usize;
        let end = start
            .checked_add(text.len())
            .filter(|n| *n < COMMANDS)
            .ok_or(fmt::Error)?;
        self.commands[start..end].copy_from_slice(text.as_bytes());
        self.length = end as u32;
        Ok(())
    }
}
impl Effects {
    /// Unchanged stacks do not touch output, including its current terminator.
    pub fn publish_commands(&self, output: &mut [u8]) -> Result<usize, Error> {
        if self.changed != 0 {
            let length = self.length as usize;
            let source = self.commands.get(..=length).ok_or(Error::Bounds)?;
            let destination = output.get_mut(..=length).ok_or(Error::Capacity)?;
            destination.copy_from_slice(source);
        }
        Ok(self.length as usize)
    }
}

pub fn parameters(
    frames: &[Frame],
    current: &[i32; PARAMETERS],
    punctuation: i32,
    capitals: i32,
) -> Result<Effects, Error> {
    if frames.len() > STACK {
        return Err(Error::Bounds);
    }
    let mut effects = Effects {
        values: [-1; PARAMETERS],
        punctuation,
        capitals,
        length: 0,
        changed: 0,
        count: frames.len() as u32,
        commands: [0; COMMANDS],
    };
    for frame in frames {
        for (value, proposed) in effects.values.iter_mut().zip(frame.values) {
            if proposed >= 0 {
                *value = proposed;
            }
        }
    }
    for (parameter, old) in current.iter().enumerate() {
        let value = effects.values[parameter];
        if value == *old {
            continue;
        }
        effects.changed = 1;
        let command = match parameter {
            1 => Some('S'),
            2 => Some('A'),
            3 => Some('P'),
            4 => Some('R'),
            12 => Some('F'),
            5 => {
                effects.punctuation = value - 1;
                None
            }
            6 => {
                effects.capitals = value;
                None
            }
            _ => None,
        };
        if let Some(command) = command {
            write!(&mut effects, "\u{1}{value}{command}").map_err(|_| Error::Capacity)?;
        }
    }
    Ok(effects)
}

/// Saturation retains the compatibility spare frame: the final slot is reset
/// and returned without increasing the number of active frames.
pub fn push(frames: &mut [Frame], count: &mut usize, kind: i32) -> Result<usize, Error> {
    if frames.is_empty() || frames.len() > STACK || *count >= frames.len() {
        return Err(Error::Bounds);
    }
    let index = *count;
    frames[index] = Frame {
        kind,
        values: [-1; PARAMETERS],
    };
    if index < frames.len() - 1 {
        *count += 1;
    }
    Ok(index)
}

pub fn pop(
    frames: &[Frame],
    kind: i32,
    current: &[i32; PARAMETERS],
    punctuation: i32,
    capitals: i32,
) -> Result<Effects, Error> {
    if frames.len() > STACK {
        return Err(Error::Bounds);
    }
    let kind = if kind >= 32 { kind - 32 } else { kind };
    let top = frames
        .iter()
        .rposition(|frame| frame.kind == kind)
        .unwrap_or(0);
    let count = if top > 0 { top } else { frames.len() };
    parameters(&frames[..count], current, punctuation, capitals)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordered_values_commands_and_saturation_match_compatibility() {
        let mut frames = [Frame {
            kind: 3,
            values: [-1; PARAMETERS],
        }; STACK];
        frames[0].values = [100; PARAMETERS];
        frames[1].values[1] = 200;
        frames[1].values[5] = 2;
        let effect = parameters(&frames[..2], &[100; PARAMETERS], 9, 8).unwrap();
        assert_eq!(effect.values[1], 200);
        assert_eq!(effect.punctuation, 1);
        assert_eq!(effect.capitals, 8);
        let mut out = [0xa5; COMMANDS];
        assert_eq!(effect.publish_commands(&mut out), Ok(5));
        assert_eq!(&out[..7], b"\x01200S\0\xa5");
        let mut count = STACK - 1;
        assert_eq!(push(&mut frames, &mut count, 12), Ok(STACK - 1));
        assert_eq!(count, STACK - 1);
        assert_eq!(
            frames[STACK - 1],
            Frame {
                kind: 12,
                values: [-1; PARAMETERS]
            }
        );
        let before = frames;
        count = STACK;
        assert_eq!(push(&mut frames, &mut count, 12), Err(Error::Bounds));
        assert_eq!(frames, before);
    }
    #[test]
    fn pop_selects_last_matching_frame_and_preserves_failed_outputs() {
        let frames = [
            Frame {
                kind: 3,
                values: [100; PARAMETERS],
            },
            Frame {
                kind: 12,
                values: [200; PARAMETERS],
            },
            Frame {
                kind: 3,
                values: [300; PARAMETERS],
            },
            Frame {
                kind: 3,
                values: [400; PARAMETERS],
            },
        ];
        let result = pop(&frames, 35, &[100; PARAMETERS], 7, 8).unwrap();
        assert_eq!(result.count, 3);
        assert_eq!(result.values, [300; PARAMETERS]);
        let mut out = [0xa5; 2];
        assert_eq!(result.publish_commands(&mut out), Err(Error::Capacity));
        assert_eq!(out, [0xa5; 2]);
        let unchanged = parameters(&frames[..1], &[100; PARAMETERS], 7, 8).unwrap();
        assert_eq!(unchanged.publish_commands(&mut []), Ok(0));
        assert_eq!(unchanged.punctuation, 7);
        let only_options = parameters(&[], &[-1; PARAMETERS], 7, 8).unwrap();
        assert_eq!(only_options.changed, 0);
        assert_eq!(pop(&frames, 99, &[100; PARAMETERS], 7, 8).unwrap().count, 4);
        let wide = parameters(
            &[Frame {
                kind: 3,
                values: [i32::MAX; PARAMETERS],
            }],
            &[0; PARAMETERS],
            0,
            0,
        )
        .unwrap();
        assert_eq!(wide.length, 60);
    }
}
