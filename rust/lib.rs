//! Native parts of the eSpeak NG Rust port.
//!
//! The safe API has no C dependencies. The `c-abi` feature lets the existing
//! engine use these implementations while the remaining pipeline is migrated.
// SPDX-License-Identifier: GPL-3.0-or-later

pub mod core_storage;
pub mod dictionary;
pub mod dictionary_storage;
pub mod encoding;
pub mod formant;
pub mod ieee80;
pub mod language;
pub mod language_options;
pub mod letters;
pub mod lookup;
pub mod mnemonics;
pub mod phoneme;
pub mod phoneme_context;
pub mod phoneme_data;
pub mod phoneme_program;
pub mod rule_match;
pub mod rules;
pub mod smoothing;
pub mod sound_icons;
pub mod spectrum;
pub mod speed;
pub mod suffix;
pub mod unicode;
pub mod voice;
pub mod voice_backend;
pub mod voice_catalog;
pub mod voice_current;
pub mod voice_directive;
pub mod voice_load;
pub mod voice_reader;
pub mod voice_request;
pub mod voice_selection;
pub mod voice_setup;
pub mod voice_storage;
pub mod word_key;
pub mod word_stress;

#[cfg(feature = "npu")]
pub mod acceleration;
#[cfg(feature = "proactor")]
pub mod data_io;
#[cfg(feature = "proactor")]
pub mod resident;

#[cfg(feature = "c-abi")]
mod ffi;
