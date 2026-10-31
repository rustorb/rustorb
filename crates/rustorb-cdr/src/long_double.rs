//! `long double`: the IEEE 754 binary128 (quadruple precision) format that
//! CDR sends in 16 octets (CORBA 3.4 part 2, 9.3.1.2).

use std::fmt;

/// A `long double` value, kept as its IEEE 754 binary128 bits: the sign
/// in the top bit, a 15-bit exponent biased by 16383 and a 112-bit
/// fraction.
///
/// Rust has no 128-bit float, so arithmetic is left to the caller: the
/// value converts exactly from `f64` ([`LongDouble::from_f64`]) and
/// rounds to the nearest `f64` ([`LongDouble::to_f64`]). The bits travel
/// as they are, so a value received and sent again keeps its precision.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct LongDouble(u128);

const FRAC_BITS: u32 = 112;
const EXP_MAX: u128 = 0x7fff;
const BIAS: i32 = 16383;
const FRAC_MASK: u128 = (1 << FRAC_BITS) - 1;
const SIGN: u128 = 1 << 127;

impl LongDouble {
    /// Zero.
    pub const ZERO: LongDouble = LongDouble(0);

    /// The value with these binary128 bits.
    pub const fn from_bits(bits: u128) -> LongDouble {
        LongDouble(bits)
    }

    /// The binary128 bits.
    pub const fn to_bits(self) -> u128 {
        self.0
    }

    /// The value of `v`, exactly (binary128 holds every `f64`).
    pub const fn from_f64(v: f64) -> LongDouble {
        let bits = v.to_bits();
        let sign = ((bits >> 63) as u128) << 127;
        let exp = ((bits >> 52) & 0x7ff) as i32;
        let frac = (bits & ((1 << 52) - 1)) as u128;
        LongDouble(
            sign | match exp {
                0x7ff => (EXP_MAX << FRAC_BITS) | (frac << (FRAC_BITS - 52)),
                0 if frac == 0 => 0,
                // A subnormal double is a normal binary128: move the
                // leading one into the implicit bit.
                0 => {
                    let top = 127 - frac.leading_zeros(); // frac * 2^-1074
                    let e = top as i32 - 1074 + BIAS;
                    ((e as u128) << FRAC_BITS) | ((frac << (FRAC_BITS - top)) & FRAC_MASK)
                }
                _ => (((exp - 1023 + BIAS) as u128) << FRAC_BITS) | (frac << (FRAC_BITS - 52)),
            },
        )
    }

    /// The nearest `f64` (ties to even); out of range values become
    /// infinities or zeros, a NaN stays a NaN.
    pub fn to_f64(self) -> f64 {
        let sign = ((self.0 >> 127) as u64) << 63;
        let exp = ((self.0 >> FRAC_BITS) & EXP_MAX) as i32;
        let frac = self.0 & FRAC_MASK;
        if exp == EXP_MAX as i32 {
            return f64::from_bits(if frac == 0 {
                sign | 0x7ff0_0000_0000_0000
            } else {
                sign | 0x7ff8_0000_0000_0000 | (frac >> (FRAC_BITS - 52)) as u64
            });
        }
        if exp == 0 && frac == 0 {
            return f64::from_bits(sign);
        }
        // The value is m * 2^(e - 112), m with its implicit bit.
        let (m, e) = if exp == 0 {
            (frac, 1 - BIAS)
        } else {
            (frac | (1 << FRAC_BITS), exp - BIAS)
        };
        let bits = if e >= -1022 {
            // Normal: keep 53 bits; rounding may carry into the exponent,
            // which the addition below takes care of.
            let r = round_shift(m, FRAC_BITS - 52);
            let biased = (e + 1023) as u64;
            if r >> 53 != 0 {
                if biased + 1 >= 0x7ff {
                    0x7ff0_0000_0000_0000
                } else {
                    ((biased + 1) << 52) | ((r >> 1) as u64 & ((1 << 52) - 1))
                }
            } else if biased >= 0x7ff {
                0x7ff0_0000_0000_0000
            } else {
                (biased << 52) | (r as u64 & ((1 << 52) - 1))
            }
        } else {
            // Subnormal (or zero): a mantissa of 2^52 is the smallest
            // normal, which the bits express as they are.
            let shift = (FRAC_BITS - 52) as i32 + (-1022 - e);
            round_shift(m, shift.min(127) as u32) as u64
        };
        f64::from_bits(sign | bits)
    }

    /// Whether the value is a NaN.
    pub fn is_nan(self) -> bool {
        (self.0 >> FRAC_BITS) & EXP_MAX == EXP_MAX && self.0 & FRAC_MASK != 0
    }

    /// Whether the sign bit is set (also for -0 and NaNs).
    pub fn is_sign_negative(self) -> bool {
        self.0 & SIGN != 0
    }

    /// The bytes in big endian order, as CDR sends them in a big endian
    /// stream.
    pub fn to_be_bytes(self) -> [u8; 16] {
        self.0.to_be_bytes()
    }

    /// The value of these big endian bytes.
    pub fn from_be_bytes(bytes: [u8; 16]) -> LongDouble {
        LongDouble(u128::from_be_bytes(bytes))
    }
}

/// `m >> shift`, rounded to nearest, ties to even.
fn round_shift(m: u128, shift: u32) -> u128 {
    if shift == 0 {
        return m;
    }
    let q = m >> shift;
    let rest = m & ((1 << shift) - 1);
    let half = 1 << (shift - 1);
    if rest > half || (rest == half && q & 1 == 1) {
        q + 1
    } else {
        q
    }
}

impl From<f64> for LongDouble {
    fn from(v: f64) -> LongDouble {
        LongDouble::from_f64(v)
    }
}

impl From<f32> for LongDouble {
    fn from(v: f32) -> LongDouble {
        LongDouble::from_f64(f64::from(v))
    }
}

impl From<LongDouble> for f64 {
    fn from(v: LongDouble) -> f64 {
        v.to_f64()
    }
}

impl fmt::Debug for LongDouble {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LongDouble({:?} / {:#034x})", self.to_f64(), self.0)
    }
}

/// Shows the nearest `f64`.
impl fmt::Display for LongDouble {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.to_f64(), f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(v: f64) {
        let ld = LongDouble::from_f64(v);
        assert_eq!(ld.to_f64().to_bits(), v.to_bits(), "{v:e}");
    }

    #[test]
    fn known_bits() {
        assert_eq!(LongDouble::from_f64(1.0).to_bits(), 0x3fff << 112);
        assert_eq!(LongDouble::from_f64(-2.0).to_bits(), SIGN | (0x4000 << 112));
        assert_eq!(
            LongDouble::from_f64(1.5).to_bits(),
            (0x3fff << 112) | (1 << 111)
        );
        assert_eq!(LongDouble::from_f64(f64::INFINITY).to_bits(), 0x7fff << 112);
        assert_eq!(LongDouble::from_f64(-0.0).to_bits(), SIGN);
        // The smallest subnormal double, 2^-1074.
        assert_eq!(
            LongDouble::from_f64(f64::from_bits(1)).to_bits(),
            ((16383 - 1074) as u128) << 112
        );
    }

    #[test]
    fn doubles_round_trip() {
        for v in [
            0.0,
            -0.0,
            1.0,
            -1.0,
            0.1,
            std::f64::consts::PI,
            f64::MAX,
            f64::MIN_POSITIVE,
            f64::from_bits(1),
            f64::from_bits(0x000f_ffff_ffff_ffff),
            f64::from_bits(0x0008_0000_0000_0001),
            f64::INFINITY,
            f64::NEG_INFINITY,
            1e300,
            -1e-310,
        ] {
            round_trip(v);
        }
        assert!(LongDouble::from_f64(f64::NAN).is_nan());
        assert!(LongDouble::from_f64(f64::NAN).to_f64().is_nan());
    }

    #[test]
    fn rounding() {
        let one = 0x3fffu128 << 112;
        // 1 + 2^-53 is half way between 1 and the next double: ties to even.
        assert_eq!(LongDouble::from_bits(one | (1 << 59)).to_f64(), 1.0);
        // Just above half way rounds up.
        assert_eq!(
            LongDouble::from_bits(one | (1 << 59) | 1).to_f64(),
            1.0 + f64::EPSILON
        );
        // 1 + 3 * 2^-53 is half way to an even mantissa above.
        assert_eq!(
            LongDouble::from_bits(one | (3 << 59)).to_f64(),
            1.0 + 2.0 * f64::EPSILON
        );
        // Every fraction bit set rounds up to 2.
        assert_eq!(LongDouble::from_bits(one | FRAC_MASK).to_f64(), 2.0);
        // Past the largest double: infinity.
        assert_eq!(LongDouble::from_bits(0x43ff << 112).to_f64(), f64::INFINITY);
        assert_eq!(
            LongDouble::from_bits((0x43fe << 112) | FRAC_MASK).to_f64(),
            f64::INFINITY
        );
        // Below the smallest subnormal double: zero, keeping the sign.
        let tiny = LongDouble::from_bits(SIGN | (1 << 112));
        assert_eq!(tiny.to_f64().to_bits(), (-0.0f64).to_bits());
        // 2^-1075 is half of the smallest subnormal: ties to even, 0.
        let half = LongDouble::from_bits(((16383 - 1075) as u128) << 112);
        assert_eq!(half.to_f64(), 0.0);
        // A little more rounds up to it.
        let more = LongDouble::from_bits((((16383 - 1075) as u128) << 112) | 1);
        assert_eq!(more.to_f64(), f64::from_bits(1));
        // The largest subnormal plus a bit rounds to the smallest normal.
        let edge = LongDouble::from_bits((((16383 - 1023) as u128) << 112) | FRAC_MASK);
        assert_eq!(edge.to_f64(), f64::MIN_POSITIVE);
        // A binary128 subnormal is far below every double.
        assert_eq!(LongDouble::from_bits(1).to_f64(), 0.0);
    }
}
