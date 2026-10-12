//! Complete active-voice admission, resource lifetime and final publication.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    voice::Voice, voice_directive::Features, voice_load, voice_reader::Reader,
    voice_request::Request, voice_setup::Setup,
};
use std::io::Read;

/// Native operations are serialized initialization/worker effects. Each input
/// loan ends at its operation; current metadata is prepared from fresh source
/// fields after path probes/open/fallback, rather than the initial path snapshot.
pub trait Host: voice_load::Host {
    type Source: Read;
    type Loaded: Copy;
    fn length(&mut self, path: &[u8]) -> i64;
    fn open(&mut self, path: &[u8], width: usize) -> Option<Reader<Self::Source>>;
    fn prepare(&mut self, fallback: &[u8; 40], tone_only: bool) -> Option<Setup>;
    fn clear_translator(&mut self);
    fn bind_current(&mut self);
    fn reset(&mut self, tone_only: bool);
    fn snapshot(&mut self) -> Option<(Voice, i32)>;
    fn close(&mut self, reader: Option<Reader<Self::Source>>);
    fn publish_languages(&mut self, setup: &Setup);
    fn result(&mut self) -> Option<Self::Loaded>;
}

pub struct Config<'a> {
    pub root: &'a [u8],
    pub default: &'a [u8],
    pub separator: u8,
    pub width: usize,
    pub features: Features,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rejected;

/// Preserve admission-before-reset and close-before-status ordering. A refused
/// open/fallback does not prepare current metadata or disturb the translator.
/// Accepted preparation/reset/directive effects remain visible on later failure.
pub fn load<H: Host>(
    host: &mut H,
    config: &Config<'_>,
    input: Option<&[u8]>,
    control: u32,
) -> Result<Option<H::Loaded>, Rejected> {
    let Some(request) = Request::prepare(
        config.root,
        input,
        control,
        config.separator,
        config.width,
        |path| host.length(path),
    )
    .map_err(|_| Rejected)?
    else {
        return Ok(None);
    };
    let reader = host.open(request.path(), config.width);
    let fallback = request
        .fallback(reader.is_some(), false, config.default)
        .map_err(|_| Rejected);
    let fallback = match fallback {
        Ok(Some(mut fallback)) => {
            if reader.is_none() && host.select_table(request.name()) >= 0 {
                fallback = request
                    .fallback(false, true, config.default)
                    .map_err(|_| Rejected)?
                    .ok_or(Rejected)?;
            }
            fallback
        }
        Ok(None) => {
            host.close(reader);
            return Ok(None);
        }
        Err(error) => {
            host.close(reader);
            return Err(error);
        }
    };
    let Some(mut setup) = host.prepare(&fallback, request.tone_only()) else {
        host.close(reader);
        return Err(Rejected);
    };
    if !request.tone_only() {
        host.clear_translator();
        host.bind_current();
    }
    host.reset(request.tone_only());
    let Some((mut voice, mut fast)) = host.snapshot() else {
        host.close(reader);
        return Err(Rejected);
    };
    let mut reader = reader;
    let configured = voice_load::configure(
        reader.as_mut(),
        &mut setup,
        &mut voice,
        &mut fast,
        config.features,
        control,
        host,
    );
    host.close(reader);
    if configured.is_err() {
        return Ok(None);
    }
    // The historical final publication updates only languages. A late native
    // effect may have changed the visible name, so do not recommit both fields.
    host.publish_languages(&setup);
    Ok(host.result())
}
