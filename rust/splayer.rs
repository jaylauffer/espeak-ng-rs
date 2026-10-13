//! Complete speechPlayer engine/frame/pitch/mix policy over admitted views.
// Derived from eSpeak NG and NV Speech Player, GPL-3.0-or-later.
use crate::{
    speech_player::{Error, Frame, Player},
    wavegen::WgenData,
};
const FADE: i32 = 110;
const CHUNK: usize = 256;
#[derive(Clone, Copy, Default)]
pub struct Spectrum {
    pub frequencies: [i16; 6],
    pub bandwidths: [u8; 4],
    pub nasal: u8,
}
#[derive(Clone, Copy, Default)]
pub struct Voice {
    pub voicing: i32,
    pub breath: i32,
    pub frequency: [i16; 6],
    pub frequency_add: [i16; 6],
    pub width: [i16; 4],
}
pub struct Engine {
    player: Player,
    active: bool,
    pcm: [i16; CHUNK],
    written: usize,
}
impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}
pub struct Mix<'a> {
    pub base: usize,
    pub bytes: &'a [u8],
}
impl Mix<'_> {
    fn byte(&self, address: usize, index: i32) -> Result<u8, Error> {
        let address = address
            .checked_add_signed(index as isize)
            .ok_or(Error::Invalid)?;
        let index = address.checked_sub(self.base).ok_or(Error::Invalid)?;
        self.bytes.get(index).copied().ok_or(Error::Invalid)
    }
}
pub fn following(commands: &[isize]) -> bool {
    for &command in commands {
        if command == 5 || command == 6 {
            break;
        }
        if command == 1 {
            return true;
        }
    }
    false
}
pub fn frame(data: &WgenData, voice: &Voice, spectrum: &Spectrum) -> Frame {
    let mut frame = Frame::default();
    let f = &mut frame.values;
    f[0] = f64::from(data.pitch) / 4096.0;
    f[5] = f64::from(voice.voicing) / 64.0;
    f[6] = f64::from(voice.breath) / 64.0;
    for i in 0..6 {
        f[7 + i] = f64::from(i32::from(spectrum.frequencies[i]) * i32::from(voice.frequency[i]))
            / 256.0
            + f64::from(voice.frequency_add[i]);
    }
    f[14] = 200.0;
    f[13] = 250.0;
    if spectrum.nasal > 0 {
        f[23] = 1.0;
        f[13] = f64::from(spectrum.nasal) * 2.0;
    }
    for i in 0..4 {
        f[15 + i] = f64::from(spectrum.bandwidths[i]) * 2.0 * (f64::from(voice.width[i]) / 256.0);
    }
    f[19] = 1000.0;
    f[20] = 1000.0;
    f[21] = 100.0;
    f[22] = 100.0;
    f[44] = 1.0;
    f[45] = 3.0 * (f64::from(data.amplitude) / 100.0);
    f[46] = f[0];
    frame
}
impl Engine {
    pub fn new() -> Self {
        Self {
            player: Player::new(22050).expect("constant positive rate"),
            active: false,
            pcm: [0; CHUNK],
            written: 0,
        }
    }
    pub fn initialize(&mut self) {
        self.player.reset(22050).expect("constant positive rate");
        self.active = true;
    }
    pub fn finish(&mut self) {
        self.active = false;
        self.player.frames.clear();
    }
    pub fn pending(&self) -> usize {
        self.player.frames.pending()
    }
    pub fn written(&self) -> usize {
        self.written
    }
    /// Admitted output/mix views are disjoint. On a late bad mix read, completed
    /// pitch/queue/DSP/mix effects stay visible; no C replay or rollback occurs.
    #[allow(clippy::too_many_arguments)]
    pub fn fill(
        &mut self,
        data: &mut WgenData,
        voice: &Voice,
        length: i32,
        frames: Option<(&Spectrum, &Spectrum)>,
        envelope: &[u8; 128],
        follows: bool,
        output: &mut [u8],
        mix: Mix<'_>,
        random: &mut impl FnMut() -> f64,
    ) -> Result<usize, Error> {
        self.written = 0;
        if !self.active {
            return Err(Error::Invalid);
        }
        if let Some((first, last)) = frames {
            let mut first = frame(data, voice, first);
            let mut last = frame(data, voice, last);
            let next_index = data
                .pitch_ix
                .checked_add(
                    data.pitch_inc
                        .checked_mul(length / 64)
                        .ok_or(Error::Invalid)?,
                )
                .ok_or(Error::Invalid)?;
            let env_index = (next_index >> 8).min(127);
            let env_index = usize::try_from(env_index).map_err(|_| Error::Invalid)?;
            let pitch = ((i32::from(envelope[env_index])
                .checked_mul(data.pitch_range)
                .ok_or(Error::Invalid)?
                >> 8)
                .checked_add(data.pitch_base))
            .ok_or(Error::Invalid)?;
            last.values[46] = f64::from(pitch / 4096); // original integer division
            if data.n_mix_wavefile != 0 {
                first.values[45] /= 5.0;
                last.values[45] /= 5.0;
            }
            let main = length
                .checked_sub(FADE)
                .and_then(|n| n.checked_sub(if follows { 0 } else { FADE }))
                .ok_or(Error::Invalid)?;
            let required = 1 + usize::from(main >= 1) + if follows { 0 } else { 2 };
            if self.pending() + required > crate::speech_player::QUEUE_CAPACITY {
                return Err(Error::Full);
            }
            data.pitch_ix = next_index;
            data.pitch = pitch;
            self.player
                .frames
                .queue(Some(first), FADE as u32, FADE as u32, -1, false)?;
            if main >= 1 {
                self.player
                    .frames
                    .queue(Some(last), main as u32, main as u32, -1, false)?;
            }
            if !follows {
                last.values[0] = last.values[46];
                last.values[44] = 0.0;
                self.player.frames.queue(
                    Some(last),
                    (FADE / 2) as u32,
                    (FADE / 2) as u32,
                    -1,
                    false,
                )?;
                last.values[45] = 0.0;
                self.player.frames.queue(
                    Some(last),
                    (FADE / 2) as u32,
                    (FADE / 2) as u32,
                    -1,
                    false,
                )?;
            }
        }
        let capacity = output.len() / 2;
        let mut written = 0;
        while written < capacity {
            let count = (capacity - written).min(CHUNK);
            let produced = self.player.synthesize(&mut self.pcm[..count], random);
            for i in 0..produced {
                let mut sample = self.pcm[i];
                if data.mix_wavefile_ix < data.n_mix_wavefile {
                    let at = data
                        .mix_wavefile_ix
                        .checked_add(data.mix_wavefile_offset)
                        .ok_or(Error::Invalid)?;
                    let mut value = if data.mix_wave_scale == 0 {
                        let low = i32::from(mix.byte(data.mix_wavefile, at)?);
                        data.mix_wavefile_ix =
                            data.mix_wavefile_ix.checked_add(1).ok_or(Error::Invalid)?;
                        low + i32::from(
                            mix.byte(data.mix_wavefile, at.checked_add(1).ok_or(Error::Invalid)?)?
                                as i8,
                        ) * 256
                    } else {
                        i32::from(mix.byte(data.mix_wavefile, at)? as i8)
                            .checked_mul(data.mix_wave_scale)
                            .ok_or(Error::Invalid)?
                    };
                    value = (f64::from(value) * (f64::from(data.amplitude_v) / 1024.0)) as i32;
                    value = value.checked_mul(data.mix_wave_amp).ok_or(Error::Invalid)? / 40;
                    sample = sample.wrapping_add(value as i16);
                    if data
                        .mix_wavefile_ix
                        .checked_add(data.mix_wavefile_offset)
                        .ok_or(Error::Invalid)?
                        >= data.mix_wavefile_max
                    {
                        data.mix_wavefile_offset = data
                            .mix_wavefile_offset
                            .checked_sub(
                                data.mix_wavefile_max.checked_mul(3).ok_or(Error::Invalid)? / 4,
                            )
                            .ok_or(Error::Invalid)?;
                    }
                    data.mix_wavefile_ix =
                        data.mix_wavefile_ix.checked_add(1).ok_or(Error::Invalid)?;
                }
                let at = (written + i) * 2;
                output[at..at + 2].copy_from_slice(&sample.to_le_bytes());
                self.written += 2;
            }
            written += produced;
            if produced < count {
                break;
            }
        }
        Ok(written * 2)
    }
}
