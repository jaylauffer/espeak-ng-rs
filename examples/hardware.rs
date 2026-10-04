// SPDX-License-Identifier: GPL-3.0-or-later
fn main() {
    println!(
        "Available loadngo compute devices: {:?}",
        espeak_ng_rs::acceleration::available_devices()
    );
    println!("Current native Rust modules execute on CPU; no NPU speech partition is enabled.");
}
