//! Loadngo's compute boundary for future measured speech partitions.
//!
//! Device enumeration is capability discovery, not proof of NPU execution.
//! The currently ported table lookups/decoders run on CPU. Formant synthesis
//! uses small sequential filters, so no neural model or artificial tensor
//! workload is added just to exercise the hardware.
// SPDX-License-Identifier: GPL-3.0-or-later

pub use loadngo_inference::compute::{ComputePolicy, DeviceKind, PreparedCompute};

/// Query supported accelerator devices once at initialization, never per sample.
pub fn available_devices() -> Vec<DeviceKind> {
    #[cfg(target_os = "macos")]
    {
        loadngo_coreml::available_devices()
    }
    #[cfg(not(target_os = "macos"))]
    {
        vec![DeviceKind::Cpu]
    }
}
