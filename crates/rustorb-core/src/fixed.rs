//! `fixed`: decimal fixed-point values.

use std::fmt;
use std::str::FromStr;

/// The value of an IDL `fixed<digits, scale>`: a decimal number with a
/// sign, its digits and the number of them after the decimal point.
///
/// The value is kept as it was made, and fitted to the digits and scale of
/// a `fixed` type when it is sent: digits after the scale are cut off,
/// and an integer part that does not fit is `DATA_CONVERSION`. Values
/// compare by their number, so `1.50` equals `1.5`. Arithmetic is not
/// provided.
#[derive(Debug, Clone, Default)]
pub struct Fixed {
    /// The digits, each 0 to 9, most significant first, without leading
    /// zeros in the integer part.
    digits: Vec<u8>,
    /// How many of the digits are after the decimal point.
    scale: u16,
    negative: bool,
}

/// The text given to [`Fixed::from_str`] is not a decimal number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseFixedError;

impl fmt::Display for ParseFixedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid fixed-point literal")
    }
}

impl std::error::Error for ParseFixedError {}

impl Fixed {
    /// The value with the digits `digits` (each 0 to 9, most significant
    /// first), `scale` of them after the decimal point. `None` if a digit
    /// is over 9 or there are fewer digits than `scale`.
    pub fn from_digits(digits: &[u8], scale: u16, negative: bool) -> Option<Fixed> {
        if digits.iter().any(|&d| d > 9) || digits.len() < scale as usize {
            return None;
        }
        let int_len = digits.len() - scale as usize;
        let skip = digits[..int_len].iter().take_while(|&&d| d == 0).count();
        let digits = digits[skip..].to_vec();
        let negative = negative && digits.iter().any(|&d| d != 0);
        Some(Fixed {
            digits,
            scale,
            negative,
        })
    }

    /// The digits, most significant first, without leading zeros in the
    /// integer part.
    pub fn digits(&self) -> &[u8] {
        &self.digits
    }

    /// How many of the digits are after the decimal point.
    pub fn scale(&self) -> u16 {
        self.scale
    }

    /// Whether the value is below zero.
    pub fn is_negative(&self) -> bool {
        self.negative
    }

    fn int_digits(&self) -> &[u8] {
        &self.digits[..self.digits.len() - self.scale as usize]
    }

    fn frac_digits(&self) -> &[u8] {
        &self.digits[self.digits.len() - self.scale as usize..]
    }

    /// The value with at most `scale` digits after the decimal point, the
    /// others cut off.
    pub fn truncate(&self, scale: u16) -> Fixed {
        if scale >= self.scale {
            return self.clone();
        }
        let cut = (self.scale - scale) as usize;
        let digits = &self.digits[..self.digits.len() - cut];
        Fixed::from_digits(digits, scale, self.negative).expect("valid digits")
    }

    /// The value with at most `scale` digits after the decimal point,
    /// rounded half away from zero.
    pub fn round(&self, scale: u16) -> Fixed {
        if scale >= self.scale {
            return self.clone();
        }
        let cut = (self.scale - scale) as usize;
        let keep = self.digits.len() - cut;
        let mut digits = self.digits[..keep].to_vec();
        if self.digits[keep] >= 5 {
            let mut i = digits.len();
            loop {
                if i == 0 {
                    digits.insert(0, 1);
                    break;
                }
                i -= 1;
                if digits[i] == 9 {
                    digits[i] = 0;
                } else {
                    digits[i] += 1;
                    break;
                }
            }
        }
        Fixed::from_digits(&digits, scale, self.negative).expect("valid digits")
    }

    /// The exactly `digits` digits of the value as a `fixed<digits,
    /// scale>`, digits after `scale` cut off; `None` if the integer part
    /// does not fit.
    pub fn to_digits(&self, digits: u16, scale: u16) -> Option<Vec<u8>> {
        let int = self.int_digits();
        let int_room = digits.checked_sub(scale)? as usize;
        if int.len() > int_room {
            return None;
        }
        let mut out = vec![0; int_room - int.len()];
        out.extend_from_slice(int);
        let frac = self.frac_digits();
        for i in 0..scale as usize {
            out.push(frac.get(i).copied().unwrap_or(0));
        }
        Some(out)
    }

    /// The integer part without leading zeros and the fraction without
    /// trailing zeros, for comparisons.
    fn normalized(&self) -> (&[u8], &[u8]) {
        let frac = self.frac_digits();
        let end = frac.len() - frac.iter().rev().take_while(|&&d| d == 0).count();
        (self.int_digits(), &frac[..end])
    }

    fn is_zero(&self) -> bool {
        self.digits.iter().all(|&d| d == 0)
    }
}

impl PartialEq for Fixed {
    fn eq(&self, other: &Fixed) -> bool {
        if self.is_zero() && other.is_zero() {
            return true;
        }
        self.negative == other.negative && self.normalized() == other.normalized()
    }
}

impl Eq for Fixed {}

impl FromStr for Fixed {
    type Err = ParseFixedError;

    /// Parses `[-+]digits[.digits]`, with an optional `d` or `D` suffix as
    /// in IDL literals.
    fn from_str(s: &str) -> Result<Fixed, ParseFixedError> {
        let s = s.strip_suffix(['d', 'D']).unwrap_or(s);
        let (negative, s) = match s.as_bytes().first() {
            Some(b'-') => (true, &s[1..]),
            Some(b'+') => (false, &s[1..]),
            _ => (false, s),
        };
        let (int, frac) = s.split_once('.').unwrap_or((s, ""));
        if int.is_empty() && frac.is_empty() {
            return Err(ParseFixedError);
        }
        let mut digits = Vec::with_capacity(int.len() + frac.len());
        for c in int.bytes().chain(frac.bytes()) {
            if !c.is_ascii_digit() {
                return Err(ParseFixedError);
            }
            digits.push(c - b'0');
        }
        let scale = u16::try_from(frac.len()).map_err(|_| ParseFixedError)?;
        Fixed::from_digits(&digits, scale, negative).ok_or(ParseFixedError)
    }
}

impl fmt::Display for Fixed {
    /// Writes the value with all its digits: `-12.50`, `0.5`, `7`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.negative {
            f.write_str("-")?;
        }
        let int = self.int_digits();
        if int.is_empty() {
            f.write_str("0")?;
        }
        for d in int {
            write!(f, "{d}")?;
        }
        let frac = self.frac_digits();
        if !frac.is_empty() {
            f.write_str(".")?;
            for d in frac {
                write!(f, "{d}")?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(s: &str) -> Fixed {
        s.parse().unwrap()
    }

    #[test]
    fn parses_and_prints() {
        assert_eq!(f("123.45").to_string(), "123.45");
        assert_eq!(f("-0.50").to_string(), "-0.50");
        assert_eq!(f("-1.50").to_string(), "-1.50");
        assert_eq!(f("007").to_string(), "7");
        assert_eq!(f(".5").to_string(), "0.5");
        assert_eq!(f("1.5d").to_string(), "1.5");
        assert_eq!(f("-0").to_string(), "0");
        assert!("".parse::<Fixed>().is_err());
        assert!("1.2.3".parse::<Fixed>().is_err());
        assert!("-".parse::<Fixed>().is_err());
    }

    #[test]
    fn compares_by_number() {
        assert_eq!(f("1.50"), f("1.5"));
        assert_eq!(f("-0.0"), f("0"));
        assert_ne!(f("1.5"), f("-1.5"));
        assert_ne!(f("1.5"), f("15"));
    }

    #[test]
    fn fits_to_a_type() {
        assert_eq!(f("12.345").to_digits(5, 2), Some(vec![0, 1, 2, 3, 4]));
        assert_eq!(f("-1").to_digits(3, 1), Some(vec![0, 1, 0]));
        assert_eq!(f("1234").to_digits(5, 2), None);
        assert_eq!(f("0.5").to_digits(1, 1), Some(vec![5]));
    }

    #[test]
    fn rounds_and_truncates() {
        assert_eq!(f("1.25").round(1).to_string(), "1.3");
        assert_eq!(f("-1.25").round(1).to_string(), "-1.3");
        assert_eq!(f("9.96").round(1).to_string(), "10.0");
        assert_eq!(f("1.29").truncate(1).to_string(), "1.2");
        assert_eq!(f("1.2").round(3).to_string(), "1.2");
    }
}
