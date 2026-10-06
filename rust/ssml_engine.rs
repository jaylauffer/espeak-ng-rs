//! Owned SSML controller with explicit host effects and reusable output.
// Copyright (C) 2005-2015 Jonathan Duddington, 2015-2017 Reece H. Dunn,
// 2018 Juho Hiltunen. Rust migration (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    ssml::Wide, ssml_clause as clause, ssml_control as control, ssml_parameters as parameters,
    ssml_resource as resource, ssml_text as text, ssml_voice as voice,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Bounds,
    Capacity,
    Tag,
    Parameter,
    Voice,
    Resource,
    Arithmetic,
}
/// Owned initialized controller data; no process-global Rust state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct State {
    pub parameters: [parameters::Frame; 20],
    pub parameter_count: usize,
    pub current: [i32; 15],
    pub punctuation: i32,
    pub capitals: i32,
    pub voices: [voice::Frame; 20],
    pub voice_count: usize,
    pub current_voice: [u8; 40],
    pub previous_identifier: [u8; 40],
    pub skip: [u8; 50],
    pub audio: bool,
    pub ignore: bool,
    pub clear_skipping: bool,
    pub sayas_mode: i32,
    pub sayas_start: i32,
}
impl State {
    pub fn new(base_parameters: [i32; 15], base_voice: voice::Frame) -> Self {
        let mut parameters = [parameters::Frame {
            kind: 0,
            values: [-1; 15],
        }; 20];
        parameters[0].values = base_parameters;
        let mut voices = [voice::Frame {
            kind: 0,
            variant: 0,
            gender: 0,
            age: 0,
            name: [0; 40],
            language: [0; 20],
        }; 20];
        voices[0] = base_voice;
        Self {
            parameters,
            parameter_count: 1,
            current: base_parameters,
            punctuation: base_parameters[5],
            capitals: base_parameters[6],
            voices,
            voice_count: 1,
            current_voice: [0; 40],
            previous_identifier: [0; 40],
            skip: [0; 50],
            audio: false,
            ignore: false,
            clear_skipping: false,
            sayas_mode: 0,
            sayas_start: 0,
        }
    }
}
pub struct Base {
    pub languages: [u8; 300],
    pub gender: u8,
    pub variant: [u8; 40],
}
pub struct Settings {
    pub signed_bytes: bool,
    pub decimal: u32,
    pub tone_language: i32,
    pub sonic: bool,
}
pub struct Rate {
    pub clause_pause: i32,
    pub pause: i32,
}
/// Host and controller must have disjoint ownership. Methods run on the caller
/// owner/worker; substantial resource work must already be offloaded there.
/// Classifiers are synchronous/pure. Name/voice results are copied snapshots.
/// No method may reenter, invalidate input/output, or mutate controller storage.
pub trait Host {
    fn wide_space(&self, c: u32) -> bool;
    fn byte_space(&self, c: u32) -> bool;
    fn byte_lower(&self, c: u32) -> i32;
    fn append_name(&mut self, name: &[u8]) -> i32;
    fn load_sound(&mut self, path: &[u8]) -> i32;
    fn has_uri_callback(&self) -> bool;
    fn uri(&mut self, name: &[u8], base: Option<&[u8]>) -> i32;
    fn rate(&mut self, rate: i32) -> Rate;
    fn resolve_name(&mut self, name: &[u8; 40]) -> Result<Option<[u8; 40]>, voice::Error>;
    fn select_voice(&mut self, choice: &voice::Choice) -> Result<Option<[u8; 40]>, voice::Error>;
    /// Compatibility hosts can publish a copied snapshot before a backend call
    /// and refresh changed scalar/frame effects afterwards. Default native
    /// hosts own their separate resources and need neither operation. Refreshed
    /// frames/counts must remain initialized and within the admitted bounds.
    fn publish(&mut self, _state: &State) {}
    fn refresh(&mut self, _state: &mut State) {}
}
/// Borrow only initialized prefix storage. A C bridge may implement sparse raw
/// writes to admitted writable capacity without borrowing undefined tail bytes.
pub trait Output {
    fn capacity(&self) -> usize;
    fn prefix(&self) -> &[u8];
    fn write(&mut self, index: usize, bytes: &[u8]) -> Result<(), Error>;
    fn set_length(&mut self, length: usize) -> Result<(), Error>;
}
pub struct Buffer<'a> {
    bytes: &'a mut [u8],
    length: usize,
}
impl<'a> Buffer<'a> {
    pub fn new(bytes: &'a mut [u8], length: usize) -> Result<Self, Error> {
        if length > bytes.len() {
            return Err(Error::Bounds);
        }
        Ok(Self { bytes, length })
    }
    pub fn length(&self) -> usize {
        self.length
    }
}
impl Output for Buffer<'_> {
    fn capacity(&self) -> usize {
        self.bytes.len()
    }
    fn prefix(&self) -> &[u8] {
        &self.bytes[..self.length]
    }
    fn write(&mut self, index: usize, bytes: &[u8]) -> Result<(), Error> {
        let end = index.checked_add(bytes.len()).ok_or(Error::Capacity)?;
        self.bytes
            .get_mut(index..end)
            .ok_or(Error::Capacity)?
            .copy_from_slice(bytes);
        Ok(())
    }
    fn set_length(&mut self, length: usize) -> Result<(), Error> {
        if length > self.bytes.len() {
            return Err(Error::Capacity);
        }
        self.length = length;
        Ok(())
    }
}
pub enum Tag<'a> {
    U16(&'a mut [u16]),
    U32(&'a mut [u32]),
}
impl Tag<'_> {
    fn view(&self) -> Wide<'_> {
        match self {
            Self::U16(v) => Wide::U16(v),
            Self::U32(v) => Wide::U32(v),
        }
    }
    fn slash(&mut self, index: usize) -> Result<(), Error> {
        match self {
            Self::U16(v) => *v.get_mut(index).ok_or(Error::Bounds)? = 32,
            Self::U32(v) => *v.get_mut(index).ok_or(Error::Bounds)? = 32,
        }
        Ok(())
    }
}
pub struct Controller {
    pub state: State,
    pub base: Base,
}
fn terminated(bytes: &[u8]) -> Result<&[u8], Error> {
    let end = bytes.iter().position(|b| *b == 0).ok_or(Error::Bounds)?;
    Ok(&bytes[..end])
}
fn append(output: &mut impl Output, bytes: &[u8], logical: usize) -> Result<(), Error> {
    let offset = output.prefix().len();
    output.write(offset, bytes)?;
    output.set_length(offset.checked_add(logical).ok_or(Error::Capacity)?)
}
impl Controller {
    fn parameters(&mut self, kind: Option<i32>, output: &mut impl Output) -> Result<(), Error> {
        let state = &mut self.state;
        let frames = &state.parameters[..state.parameter_count];
        let effects = match kind {
            Some(kind) => parameters::pop(
                frames,
                kind,
                &state.current,
                state.punctuation,
                state.capitals,
            ),
            None => {
                parameters::parameters(frames, &state.current, state.punctuation, state.capitals)
            }
        }
        .map_err(|_| Error::Parameter)?;
        if effects.changed != 0 {
            append(
                output,
                &effects.commands[..=effects.length as usize],
                effects.length as usize,
            )?;
        }
        state.current = effects.values;
        state.punctuation = effects.punctuation;
        state.capitals = effects.capitals;
        if kind.is_some() {
            state.parameter_count = effects.count as usize;
        }
        Ok(())
    }
    fn signal(
        &mut self,
        kind: u32,
        index: i32,
        frame: Option<usize>,
        output: &mut impl Output,
    ) -> Result<(), Error> {
        let effect = resource::signal(kind, index).map_err(|_| Error::Resource)?;
        if effect.length != 0 {
            append(
                output,
                &effect.bytes[..=effect.length as usize],
                effect.length as usize,
            )?;
        }
        if effect.silence != 0 {
            if let Some(frame) = frame {
                self.state.parameters[frame].values[0] = 1;
            }
        }
        Ok(())
    }
    fn voice(
        &mut self,
        kind: i32,
        input: Wide<'_>,
        start: usize,
        host: &mut impl Host,
    ) -> Result<i32, Error> {
        let change = voice::frame_change(
            input,
            start,
            kind,
            self.state.voice_count,
            |c| host.wide_space(c),
            |c| host.byte_space(c),
        )
        .map_err(|_| Error::Voice)?;
        if change.action == 0 {
            return Ok(0);
        }
        if change.action == 2 {
            self.state.voices[change.index as usize] = change.frame;
        }
        host.publish(&self.state);
        let choice = voice::choice(
            &self.state.voices[..change.count as usize],
            &self.base.languages,
            &self.state.previous_identifier,
            |name| host.resolve_name(name),
        )
        .map_err(|_| Error::Voice)?;
        self.state.previous_identifier = choice.identifier;
        host.publish(&self.state);
        let selected = host.select_voice(&choice).map_err(|_| Error::Voice)?;
        host.refresh(&mut self.state);
        let selected = selected.unwrap_or_else(|| {
            let mut id = [0; 40];
            id[..7].copy_from_slice(b"default");
            id
        });
        let selected_text = terminated(&selected)?;
        let variant = voice::base_variant(
            selected_text,
            choice.gender as u8,
            self.base.gender,
            terminated(&self.base.variant)?,
        );
        let selected_text = if let Some(ref variant) = variant {
            terminated(variant)?
        } else {
            selected_text
        };
        if let Some(change) = voice::voice_changed(&self.state.current_voice, selected_text)
            .map_err(|_| Error::Voice)?
        {
            self.state.current_voice[..selected_text.len() + 1]
                .copy_from_slice(&change[..selected_text.len() + 1]);
            Ok(clause::VOICE_CHANGE)
        } else {
            Ok(0)
        }
    }
    /// Process a single initialized tag on its caller owner/worker. Capacity
    /// failures preserve each rejected effect but can follow earlier separator,
    /// push or host effects; whole-controller rollback is not implied.
    pub fn process(
        &mut self,
        tag: &mut Tag<'_>,
        output: &mut impl Output,
        settings: &Settings,
        base: Option<&[u8]>,
        host: &mut impl Host,
    ) -> Result<i32, Error> {
        if !(1..20).contains(&self.state.parameter_count)
            || !(1..=20).contains(&self.state.voice_count)
            || output.prefix().len() > output.capacity()
            || output.capacity() > i32::MAX as usize
        {
            return Err(Error::Bounds);
        }
        let parsed = control::tag(
            tag.view(),
            settings.signed_bytes,
            |c| host.wide_space(c),
            |c| host.byte_lower(c),
        )
        .map_err(|_| Error::Tag)?;
        if parsed.separator != 0 && output.prefix().len() == output.capacity() {
            return Err(Error::Capacity);
        }
        if parsed.slash_index != u32::MAX {
            tag.slash(parsed.slash_index as usize)?;
        }
        if parsed.separator != 0 {
            append(output, b" ", 1)?;
        }
        if parsed.ignore != 0 {
            return Ok(0);
        }
        let input = tag.view();
        let start = parsed.attributes as usize;
        let kind = parsed.kind;
        match kind {
            3 | 10 | 12 => {
                let frame = control::directive(
                    kind,
                    input,
                    start,
                    control::Context {
                        base: &self.state.parameters[0].values,
                        current: &self.state.current,
                        tone_language: settings.tone_language,
                        decimal: settings.decimal,
                    },
                    |c| host.wide_space(c),
                )
                .map_err(|_| Error::Parameter)?;
                let index = parameters::push(
                    &mut self.state.parameters,
                    &mut self.state.parameter_count,
                    kind,
                )
                .map_err(|_| Error::Parameter)?;
                self.state.parameters[index] = frame;
                self.parameters(None, output)?;
            }
            35 | 42 | 44 => self.parameters(Some(kind), output)?,
            8 | 4 | 36 | 9 | 14 | 41 | 46 => {
                let plan = text::plan(
                    text::Request {
                        kind,
                        input,
                        start,
                        prefix: output.prefix(),
                        capacity: output.capacity(),
                        state: text::State {
                            offset: output.prefix().len() as i32,
                            mode: self.state.sayas_mode,
                            start: self.state.sayas_start,
                            ignore: u32::from(self.state.ignore),
                        },
                    },
                    |c| host.wide_space(c),
                    |c| host.byte_space(c),
                )
                .map_err(|_| Error::Capacity)?;
                let mut error = None;
                plan.emit(|index, bytes| {
                    if error.is_none() {
                        error = output.write(index, bytes).err();
                    }
                });
                if let Some(error) = error {
                    return Err(error);
                }
                output.set_length(plan.state.offset as usize)?;
                self.state.sayas_mode = plan.state.mode;
                self.state.sayas_start = plan.state.start;
                self.state.ignore = plan.state.ignore != 0;
            }
            5 => {
                let request = resource::request(
                    kind,
                    input,
                    start,
                    |c| host.wide_space(c),
                    |c| host.byte_space(c),
                )
                .map_err(|_| Error::Resource)?;
                match resource::marker(&request, terminated(&self.state.skip)?)
                    .map_err(|_| Error::Resource)?
                {
                    1 => {
                        self.state.clear_skipping = true;
                        self.state.skip[0] = 0;
                        return Ok(clause::NONE);
                    }
                    2 => {
                        host.publish(&self.state);
                        let index = host.append_name(request.name().map_err(|_| Error::Resource)?);
                        host.refresh(&mut self.state);
                        self.signal(1, index, None, output)?;
                    }
                    _ => {}
                }
            }
            11 | 43 => {
                let effect =
                    resource::audio(kind, parsed.self_closing != 0).map_err(|_| Error::Resource)?;
                if effect.push != 0 {
                    let frame = parameters::push(
                        &mut self.state.parameters,
                        &mut self.state.parameter_count,
                        kind,
                    )
                    .map_err(|_| Error::Parameter)?;
                    let request = resource::request(
                        kind,
                        input,
                        start,
                        |c| host.wide_space(c),
                        |c| host.byte_space(c),
                    )
                    .map_err(|_| Error::Resource)?;
                    if request.present != 0 {
                        host.publish(&self.state);
                        if host.has_uri_callback() {
                            let name = request.name().map_err(|_| Error::Resource)?;
                            let index = host.append_name(name);
                            host.refresh(&mut self.state);
                            if index >= 0 {
                                host.publish(&self.state);
                                let result = host.uri(name, base);
                                host.refresh(&mut self.state);
                                if result == 0 {
                                    self.signal(3, index, Some(frame), output)?;
                                }
                            }
                        } else if let Ok(path) = resource::file(&request, base) {
                            let index = host.load_sound(terminated(&path.bytes)?);
                            host.refresh(&mut self.state);
                            self.signal(2, index, Some(frame), output)?;
                        }
                    }
                    self.parameters(None, output)?;
                }
                if effect.pop != 0 {
                    self.parameters(Some(kind), output)?;
                }
                if effect.text != 2 {
                    self.state.audio = effect.text != 0;
                }
                return Ok(effect.terminator);
            }
            13 => {
                let request = clause::pause(
                    input,
                    start,
                    self.state.current[1],
                    self.state.current[10],
                    |c| host.wide_space(c),
                )
                .map_err(|_| Error::Arithmetic)?;
                if request.length != 0 {
                    append(output, &request.command, request.length as usize)?;
                }
                let factors = if request.timed != 0 {
                    host.publish(&self.state);
                    let factors = host.rate(request.rate);
                    host.refresh(&mut self.state);
                    factors
                } else {
                    Rate {
                        clause_pause: 0,
                        pause: 0,
                    }
                };
                return request
                    .finish(factors.clause_pause, factors.pause, settings.sonic)
                    .map_err(|_| Error::Arithmetic);
            }
            1 | 2 | 33 | 34 | 15 | 47 | 6 | 7 | 38 | 39 => {
                let mut kinds = [0; 20];
                for (target, frame) in kinds
                    .iter_mut()
                    .zip(&self.state.voices[..self.state.voice_count])
                {
                    *target = frame.kind;
                }
                let plan = clause::voice(kind, &kinds[..self.state.voice_count])
                    .map_err(|_| Error::Voice)?;
                if kind == 1 {
                    let request = resource::request(
                        kind,
                        input,
                        start,
                        |c| host.wide_space(c),
                        |c| host.byte_space(c),
                    )
                    .map_err(|_| Error::Resource)?;
                    if request.present != 0 {
                        host.publish(&self.state);
                        host.append_name(request.name().map_err(|_| Error::Resource)?);
                        host.refresh(&mut self.state);
                    }
                }
                self.state.voice_count = plan.count as usize;
                let mut flags = 0;
                for action in &plan.tags[..plan.length as usize] {
                    flags |= self.voice(*action, input, start, host)?;
                }
                return plan.finish(flags).map_err(|_| Error::Voice);
            }
            _ => {}
        }
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Mock {
        names: crate::name_storage::Names,
        uri: bool,
        trace: Vec<String>,
        pending_rate: Option<i32>,
        published: Option<State>,
    }
    fn identifier(value: &[u8]) -> [u8; 40] {
        let mut result = [0; 40];
        result[..value.len()].copy_from_slice(value);
        result
    }
    impl Host for Mock {
        fn wide_space(&self, c: u32) -> bool {
            matches!(c, 9..=13 | 32)
        }
        fn byte_space(&self, c: u32) -> bool {
            matches!(c, 9..=13 | 32)
        }
        fn byte_lower(&self, c: u32) -> i32 {
            (c as u8).to_ascii_lowercase() as i32
        }
        fn append_name(&mut self, name: &[u8]) -> i32 {
            self.trace
                .push(format!("name:{}", String::from_utf8_lossy(name)));
            let mut bytes = [0; 160];
            bytes[..name.len()].copy_from_slice(name);
            self.names
                .append(&bytes[..name.len() + 1], 1)
                .unwrap()
                .offset as i32
        }
        fn load_sound(&mut self, path: &[u8]) -> i32 {
            self.trace
                .push(format!("file:{}", String::from_utf8_lossy(path)));
            7
        }
        fn has_uri_callback(&self) -> bool {
            self.uri
        }
        fn uri(&mut self, name: &[u8], base: Option<&[u8]>) -> i32 {
            assert_eq!(self.published.as_ref().unwrap().parameter_count, 2);
            assert_eq!(name, b"stable-uri");
            assert_eq!(base, None);
            let mut extra = [b'x'; 4097];
            extra[4096] = 0;
            self.names.append(&extra, 1).unwrap();
            assert_eq!(name, b"stable-uri");
            self.trace.push("uri:stable-uri".into());
            0
        }
        fn rate(&mut self, rate: i32) -> Rate {
            self.trace.push(format!("rate:{rate}"));
            self.pending_rate = Some(rate);
            Rate {
                clause_pause: 256,
                pause: 128,
            }
        }
        fn resolve_name(&mut self, name: &[u8; 40]) -> Result<Option<[u8; 40]>, voice::Error> {
            self.trace.push(format!(
                "resolve:{}",
                String::from_utf8_lossy(terminated(name).unwrap())
            ));
            Ok((terminated(name).unwrap() == b"known").then(|| identifier(b"gmw/en")))
        }
        fn select_voice(
            &mut self,
            choice: &voice::Choice,
        ) -> Result<Option<[u8; 40]>, voice::Error> {
            self.trace.push("select".into());
            Ok(Some(identifier(
                if terminated(&choice.language).unwrap() == b"fr" {
                    b"roa/fr"
                } else {
                    b"gmw/en"
                },
            )))
        }
        fn publish(&mut self, state: &State) {
            self.published = Some(*state);
        }
        fn refresh(&mut self, state: &mut State) {
            if let Some(rate) = self.pending_rate.take() {
                state.parameters[0].values[1] = rate;
            }
        }
    }
    fn engine() -> (Controller, Mock, Settings) {
        let mut base_voice = voice::Frame {
            kind: 0,
            variant: 0,
            gender: 0,
            age: 0,
            name: [0; 40],
            language: [0; 20],
        };
        base_voice.language[..2].copy_from_slice(b"en");
        let parameters = [0, 175, 100, 50, 50, 0, 0, 0, 0, 0, 100, 0, 0, 0, 0];
        let mut base = Base {
            languages: [0; 300],
            gender: 1,
            variant: [0; 40],
        };
        base.languages[..15].copy_from_slice(b"\x05en-gb\0\x08en\0\0\0\0\0");
        base.variant[..2].copy_from_slice(b"m2");
        (
            Controller {
                state: State::new(parameters, base_voice),
                base,
            },
            Mock {
                names: crate::name_storage::Names::new(65536).unwrap(),
                uri: false,
                trace: Vec::new(),
                pending_rate: None,
                published: None,
            },
            Settings {
                signed_bytes: true,
                decimal: 46,
                tone_language: 0,
                sonic: false,
            },
        )
    }
    fn process(
        engine: &mut Controller,
        host: &mut Mock,
        settings: &Settings,
        name: &str,
        bytes: &mut [u8],
        length: usize,
        base: Option<&[u8]>,
    ) -> Result<(i32, usize), Error> {
        let mut units: Vec<_> = name.chars().map(u32::from).chain([0]).collect();
        let mut output = Buffer::new(bytes, length).unwrap();
        let result =
            engine.process(&mut Tag::U32(&mut units), &mut output, settings, base, host)?;
        Ok((result, output.length()))
    }
    #[test]
    fn independent_controllers_preserve_nested_parameter_effects_and_text_tails() {
        let (mut first, mut host, settings) = engine();
        let (second, _, _) = engine();
        let mut bytes = [0xa5; 80];
        assert_eq!(
            process(
                &mut first,
                &mut host,
                &settings,
                "prosody rate='200%'",
                &mut bytes,
                0,
                None
            ),
            Ok((0, 6))
        );
        assert_eq!(&bytes[..7], b" \x01350S\0");
        assert_eq!(first.state.parameter_count, 2);
        assert_eq!(second.state.parameter_count, 1);
        assert_eq!(second.state.current[1], 175);
        assert_eq!(
            process(&mut first, &mut host, &settings, "/prosody", &mut bytes, 0, None),
            Ok((0, 6))
        );
        assert_eq!(&bytes[..7], b" \x01175S\0");
        assert_eq!(first.state.parameter_count, 1);
        assert_eq!(
            process(
                &mut first,
                &mut host,
                &settings,
                "say-as interpret-as='tts:key'",
                &mut bytes,
                0,
                None
            ),
            Ok((0, 5))
        );
        // Tag dispatch contributes the key's trailing separator itself.
        bytes[5..10].copy_from_slice(b"space");
        assert_eq!(
            process(&mut first, &mut host, &settings, "/say-as", &mut bytes, 10, None),
            Ok((0, 10))
        );
        assert_eq!(&bytes[5..10], &[0xee, 0x80, 0xa0, 1, b'Y']);
        assert_eq!(bytes[11], 0);
    }
    #[test]
    fn resource_callbacks_observe_push_and_copy_lifetime_then_pop_and_text() {
        let (mut engine, mut host, settings) = engine();
        host.uri = true;
        engine.state.audio = true;
        let mut bytes = [0xa5; 80];
        assert_eq!(
            process(
                &mut engine,
                &mut host,
                &settings,
                "audio src='stable-uri'/",
                &mut bytes,
                0,
                None
            ),
            Ok((clause::NONE, 4))
        );
        assert_eq!(&bytes[..5], b" \x010U\0");
        assert_eq!(host.trace, ["name:stable-uri", "uri:stable-uri"]);
        assert_eq!(engine.state.parameter_count, 1);
        assert!(engine.state.audio);
        assert_eq!(engine.state.current[0], 0);
        host.uri = false;
        host.trace.clear();
        assert_eq!(
            process(
                &mut engine,
                &mut host,
                &settings,
                "audio src='tone.wav'",
                &mut bytes,
                0,
                Some(b"root")
            ),
            Ok((clause::NONE, 4))
        );
        assert_eq!(host.trace, ["file:root/tone.wav"]);
        assert_eq!(engine.state.parameter_count, 2);
        assert_eq!(engine.state.current[0], 1);
        assert_eq!(
            process(
                &mut engine,
                &mut host,
                &settings,
                "/audio",
                &mut bytes,
                0,
                None
            ),
            Ok((clause::NONE, 1))
        );
        assert!(!engine.state.audio);
        assert_eq!(engine.state.parameter_count, 1);
    }
    #[test]
    fn rate_and_voice_effects_retain_order_local_counts_and_checked_admission() {
        let (mut engine, mut host, settings) = engine();
        let mut bytes = [0xa5; 80];
        assert_eq!(
            process(
                &mut engine,
                &mut host,
                &settings,
                "break strength='weak' time='2s'",
                &mut bytes,
                0,
                None
            ),
            Ok((clause::NONE + 200, 4))
        );
        assert_eq!(&bytes[..5], b" \x012B\0");
        assert_eq!(host.trace, ["rate:175"]);
        host.trace.clear();
        assert_eq!(
            process(
                &mut engine,
                &mut host,
                &settings,
                "voice name='known'",
                &mut bytes,
                0,
                None
            ),
            Ok((0x24000, 1))
        );
        assert_eq!(host.trace, ["resolve:known", "select"]);
        assert_eq!(engine.state.voice_count, 1);
        assert_eq!(
            terminated(&engine.state.current_voice).unwrap(),
            b"gmw/en+m2"
        );
        let before = engine.state;
        assert_eq!(
            process(
                &mut engine,
                &mut host,
                &settings,
                "voice/",
                &mut [],
                0,
                None
            ),
            Err(Error::Capacity)
        );
        assert_eq!(engine.state, before);
        assert_eq!(
            process(
                &mut engine,
                &mut host,
                &settings,
                "emphasis level='invalid'",
                &mut bytes,
                0,
                None
            ),
            Err(Error::Parameter)
        );
        assert_eq!(engine.state.parameter_count, before.parameter_count);
    }
    #[test]
    fn windows_tags_preserve_legacy_code_unit_encoding_without_backend_calls() {
        let (mut engine, mut host, settings) = engine();
        let mut input: Vec<_> = "phoneme alphabet='espeak' ph='😀'"
            .encode_utf16()
            .chain([0])
            .collect();
        let mut bytes = [0xa5; 40];
        let mut output = Buffer::new(&mut bytes, 0).unwrap();
        assert_eq!(
            engine.process(
                &mut Tag::U16(&mut input),
                &mut output,
                &settings,
                None,
                &mut host
            ),
            Ok(0)
        );
        assert_eq!(output.length(), 11);
        assert!(host.trace.is_empty());
        assert_eq!(&bytes[..12], b" [[\xed\xa0\xbd\xed\xb8\x80]]\xa5");
    }
}
