//! Decode AIFF's big-endian 80-bit sample rate without native extended floats.
// Copyright (C) 2022 Ulrich Müller; Rust adaptation (C) 2026.
// SPDX-License-Identifier: GPL-3.0-or-later OR BSD-2-Clause

pub fn extended_to_double(bytes: &[u8; 10]) -> f64 {
    let exponent = i32::from(u16::from_be_bytes([bytes[0] & 0x7f, bytes[1]]));
    let mantissa = u64::from_be_bytes(bytes[2..].try_into().expect("eight mantissa bytes"));
    let value = match exponent {
        0 => 0.0, // all extended denormals underflow in binary64
        0x7fff => {
            if mantissa & 0x7fff_ffff_ffff_ffff == 0 {
                f64::INFINITY
            } else {
                f64::NAN
            }
        }
        _ => scale(mantissa as f64, exponent - 16446),
    };
    if bytes[0] & 0x80 != 0 {
        -value
    } else {
        value
    }
}

// Split scaling to keep the power itself from overflowing/underflowing before
// multiplication. In particular, a 64-bit mantissa rescues powers below -1074.
fn scale(mut value: f64, mut power: i32) -> f64 {
    while power < -1022 {
        value *= f64::from_bits(1_u64 << 52);
        power += 1022;
    }
    while power > 1023 {
        value *= f64::from_bits(2046_u64 << 52);
        power -= 1023;
    }
    value * f64::from_bits(((power + 1023) as u64) << 52)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aiff_sample_rates_signed_zero_and_extremes() {
        assert_eq!(
            extended_to_double(&[0x40, 0x0e, 0xac, 0x44, 0, 0, 0, 0, 0, 0]),
            44100.0
        );
        assert_eq!(
            extended_to_double(&[0xbf, 0xff, 0x80, 0, 0, 0, 0, 0, 0, 0]),
            -1.0
        );
        assert_eq!(
            extended_to_double(&[0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0]).to_bits(),
            (-0.0_f64).to_bits()
        );
        assert!(extended_to_double(&[0x7f, 0xff, 0xc0, 0, 0, 0, 0, 0, 0, 0]).is_nan());
        assert_eq!(
            extended_to_double(&[0x3b, 0xcd, 0x80, 0, 0, 0, 0, 0, 0, 0]).to_bits(),
            1
        );
    }
}
