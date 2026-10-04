//! Native parts of the eSpeak NG Rust port.
//!
//! The safe API has no C dependencies. The `c-abi` feature lets the existing
//! engine use these implementations while the remaining pipeline is migrated.
// SPDX-License-Identifier: GPL-3.0-or-later

pub mod dictionary;
pub mod encoding;
pub mod ieee80;
pub mod mnemonics;
pub mod phoneme;
pub mod unicode;

#[cfg(feature = "npu")]
pub mod acceleration;
#[cfg(feature = "proactor")]
pub mod data_io;

#[cfg(feature = "c-abi")]
mod ffi;
