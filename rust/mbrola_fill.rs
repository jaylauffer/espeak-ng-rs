//! Owned MBROLA sample accounting over bounded caller-provided PCM storage.
//!
//! A pending read never ends an entry. Hosts resume on their I/O completion;
//! this state performs one read per call and creates no scheduler or buffer.
//! Ordinary command flushes do not acknowledge the end of the audio stream.
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::mbrola_output::scale_pcm;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Read {
    /// Initialized little-endian sample pairs written into the requested slice.
    Samples(usize),
    /// No samples consumed; resume only on a fresh host completion.
    Pending,
    End,
    Failed,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Status {
    Complete,
    More,
    Pending,
    End,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Outcome {
    pub bytes: usize,
    pub status: Status,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Bounds,
    Arithmetic,
    Resume,
    Read,
    Scaling,
}
#[derive(Clone, Copy, Default, Eq, PartialEq)]
enum Phase {
    #[default]
    Fresh,
    Active,
    Complete,
    End,
    Failed,
}
#[derive(Default)]
pub struct Fill {
    remaining: usize,
    rate: i32,
    milliseconds: i32,
    phase: Phase,
}

impl Fill {
    /// Start an entry, or resume its unconsumed sample target. Length/rate
    /// must remain the same on resume; amplitude may change with the host.
    /// Invalid parameters preserve state and issue no read. A host/scaling
    /// failure makes this run terminal; already consumed audio is not rolled
    /// back. Restart clears the cursor without resetting the external backend.
    pub fn fill(
        &mut self,
        output: &mut [u8],
        rate: i32,
        milliseconds: i32,
        resume: bool,
        amplitude: i32,
        mut read: impl FnMut(&mut [u8]) -> Read,
    ) -> Result<Outcome, Error> {
        if output.len() % 2 != 0 || rate <= 0 || milliseconds < 0 {
            return Err(Error::Bounds);
        }
        if resume {
            if self.rate != rate
                || self.milliseconds != milliseconds
                || matches!(self.phase, Phase::Fresh | Phase::Failed)
            {
                return Err(Error::Resume);
            }
        } else {
            let samples = i64::from(rate) * i64::from(milliseconds) / 1000;
            let samples = usize::try_from(samples).map_err(|_| Error::Arithmetic)?;
            self.remaining = samples;
            self.rate = rate;
            self.milliseconds = milliseconds;
            self.phase = Phase::Active;
        }
        let result = self.run(output, amplitude, &mut read);
        if result.is_err() {
            self.phase = Phase::Failed;
        }
        result
    }

    fn run(
        &mut self,
        output: &mut [u8],
        amplitude: i32,
        read: &mut impl FnMut(&mut [u8]) -> Read,
    ) -> Result<Outcome, Error> {
        let empty = |status| Outcome { bytes: 0, status };
        if self.phase == Phase::End {
            return Ok(empty(Status::End));
        }
        if self.remaining == 0 {
            self.phase = Phase::Complete;
            return Ok(empty(Status::Complete));
        }
        let requested = self.remaining.min(output.len() / 2);
        if requested == 0 {
            return Ok(empty(Status::More));
        }
        let target = &mut output[..requested * 2];
        match read(target) {
            Read::Pending => Ok(empty(Status::Pending)),
            Read::End => {
                self.phase = Phase::End;
                Ok(empty(Status::End))
            }
            Read::Failed => Err(Error::Read),
            Read::Samples(samples) => {
                if samples == 0 || samples > requested {
                    return Err(Error::Read);
                }
                let bytes = samples * 2;
                scale_pcm(&mut target[..bytes], amplitude).map_err(|_| Error::Scaling)?;
                self.remaining -= samples;
                let status = if self.remaining == 0 {
                    self.phase = Phase::Complete;
                    Status::Complete
                } else {
                    Status::More
                };
                Ok(Outcome { bytes, status })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_reads_preserve_remaining_samples_and_only_scale_written_pairs() {
        let mut state = Fill::default();
        let mut output = [0x5a; 10];
        let mut requests = Vec::new();
        let mut read = |target: &mut [u8]| {
            requests.push(target.len());
            let samples = target.len().min(4) / 2;
            for pair in target[..samples * 2].chunks_exact_mut(2) {
                pair.copy_from_slice(&20000_i16.to_le_bytes());
            }
            Read::Samples(samples)
        };
        for (resume, bytes, status) in [
            (false, 4, Status::More),
            (true, 4, Status::More),
            (true, 2, Status::Complete),
        ] {
            output.fill(0x5a);
            assert_eq!(
                state.fill(&mut output, 1000, 5, resume, 80, &mut read),
                Ok(Outcome { bytes, status })
            );
            assert!(output[..bytes]
                .chunks_exact(2)
                .all(|s| s == i16::MAX.to_le_bytes()));
            assert!(output[bytes..].iter().all(|&b| b == 0x5a));
        }
        assert_eq!(requests, [10, 6, 2]);
        assert_eq!(
            state.fill(&mut output, 1000, 5, true, 40, |_| panic!(
                "completed entry"
            )),
            Ok(Outcome {
                bytes: 0,
                status: Status::Complete
            })
        );
    }

    #[test]
    fn full_output_and_pending_reads_do_not_end_or_consume_the_entry() {
        let mut state = Fill::default();
        assert_eq!(
            state.fill(&mut [], 22050, 2, false, 40, |_| panic!("no output room")),
            Ok(Outcome {
                bytes: 0,
                status: Status::More
            })
        );
        let mut output = [0x5a; 100];
        for _ in 0..3 {
            assert_eq!(
                state.fill(&mut output, 22050, 2, true, 40, |_| Read::Pending),
                Ok(Outcome {
                    bytes: 0,
                    status: Status::Pending
                })
            );
            assert_eq!(output, [0x5a; 100]);
        }
        assert_eq!(
            state.fill(&mut output, 22050, 2, true, 40, |target| {
                assert_eq!(target.len(), 88);
                target.fill(0);
                Read::Samples(44)
            }),
            Ok(Outcome {
                bytes: 88,
                status: Status::Complete
            })
        );
        assert_eq!(
            state.fill(&mut output, 22050, 10, false, 40, |_| Read::End),
            Ok(Outcome {
                bytes: 0,
                status: Status::End
            })
        );
        assert_eq!(
            state.fill(&mut output, 22050, 10, true, 40, |_| panic!("ended entry")),
            Ok(Outcome {
                bytes: 0,
                status: Status::End
            })
        );
        assert_eq!(
            state.fill(&mut output, 22050, 0, false, 40, |_| panic!(
                "zero sample target"
            )),
            Ok(Outcome {
                bytes: 0,
                status: Status::Complete
            })
        );
    }

    #[test]
    fn invalid_parameters_preserve_state_and_read_failures_are_terminal() {
        let mut state = Fill::default();
        let mut output = [0; 8];
        assert_eq!(
            state
                .fill(&mut output, 1000, 10, false, 40, |_| Read::Pending)
                .unwrap()
                .status,
            Status::Pending
        );
        for (rate, length, resume, size, error) in [
            (0, 10, true, 8, Error::Bounds),
            (1000, -1, false, 8, Error::Bounds),
            (1000, 10, true, 3, Error::Bounds),
            (1001, 10, true, 8, Error::Resume),
            (1000, 11, true, 8, Error::Resume),
        ] {
            assert_eq!(
                state.fill(&mut output[..size], rate, length, resume, 40, |_| panic!(
                    "rejected"
                )),
                Err(error)
            );
        }
        assert_eq!(
            state.fill(&mut output, 1000, 10, true, 40, |_| Read::Samples(5)),
            Err(Error::Read)
        );
        assert_eq!(
            state.fill(&mut output, 1000, 10, true, 40, |_| panic!("terminal")),
            Err(Error::Resume)
        );
        for read in [Read::Samples(0), Read::Samples(usize::MAX), Read::Failed] {
            assert_eq!(
                state.fill(&mut output, 1000, 10, false, 40, |_| read),
                Err(Error::Read)
            );
        }
        assert_eq!(
            state.fill(&mut output, 1000, 1, false, i32::MAX, |s| {
                s.copy_from_slice(&i16::MAX.to_le_bytes());
                Read::Samples(1)
            }),
            Err(Error::Scaling)
        );
        let mut other = Fill::default();
        assert_eq!(
            other
                .fill(&mut output, 1000, 2, false, 40, |s| {
                    s.fill(0);
                    Read::Samples(2)
                })
                .unwrap()
                .status,
            Status::Complete
        );
        assert_eq!(
            state.fill(&mut output, 1000, 1, true, 40, |_| panic!("still terminal")),
            Err(Error::Resume)
        );
    }
}
