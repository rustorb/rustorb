//! Code set negotiation (CORBA 3.0, 13.10.2) and conversion of `char` data.
//!
//! rustorb's native code set for `char` and `string` is UTF-8, which it
//! can convert to ISO 8859-1. For `wchar` and `wstring` it is UTF-16, with
//! no conversions.

use rustorb_giop::{CodeSetComponent, CodeSetComponentInfo, CodeSetContext, code_set};

use crate::{CompletionStatus, SystemException, SystemExceptionKind};

/// Native `char` code set of rustorb.
pub const NATIVE_CHAR: u32 = code_set::UTF_8;
/// `char` code sets rustorb converts to and from.
pub const CONVERSION_CHAR: &[u32] = &[code_set::ISO_8859_1];
/// Native `wchar` code set of rustorb.
pub const NATIVE_WCHAR: u32 = code_set::UTF_16;

/// The transmission code sets used on a connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodeSets {
    /// Transmission code set for `char` and `string`.
    pub char_data: u32,
    /// Transmission code set for `wchar` and `wstring`, or `None` when the
    /// server did not publish one.
    pub wchar_data: Option<u32>,
}

impl CodeSets {
    /// The code sets that apply without negotiation: ISO 8859-1 for
    /// `char` and no `wchar` code set. GIOP 1.0, and objects whose IOR has
    /// no code set component, use these.
    pub const DEFAULT: CodeSets = CodeSets {
        char_data: code_set::ISO_8859_1,
        wchar_data: None,
    };

    /// The native code sets of rustorb, UTF-8 and UTF-16: what values
    /// marshalled outside a connection use.
    pub const NATIVE: CodeSets = CodeSets {
        char_data: NATIVE_CHAR,
        wchar_data: Some(NATIVE_WCHAR),
    };

    /// Chooses the transmission code sets for a server that published
    /// `info`, as the client side of the negotiation.
    pub fn negotiate(info: &CodeSetComponentInfo) -> Result<CodeSets, SystemException> {
        let char_data =
            choose(&info.for_char_data, NATIVE_CHAR, CONVERSION_CHAR).ok_or_else(incompatible)?;
        let wchar = &info.for_wchar_data;
        let wchar_data = if wchar.native_code_set == 0 && wchar.conversion_code_sets.is_empty() {
            None
        } else {
            Some(choose(wchar, NATIVE_WCHAR, &[]).ok_or_else(incompatible)?)
        };
        Ok(CodeSets {
            char_data,
            wchar_data,
        })
    }

    /// The `CodeSets` service context announcing these code sets. A
    /// missing `wchar` code set is sent as `0`.
    pub fn context(&self) -> CodeSetContext {
        CodeSetContext {
            char_data: self.char_data,
            wchar_data: self.wchar_data.unwrap_or(0),
        }
    }

    /// Encodes `s` in the `char` transmission code set.
    pub fn encode_string(&self, s: &str) -> Result<Vec<u8>, SystemException> {
        match self.char_data {
            code_set::UTF_8 => Ok(s.as_bytes().to_vec()),
            code_set::ISO_8859_1 => s
                .chars()
                .map(|c| {
                    u8::try_from(u32::from(c)).map_err(|_| data_conversion(CompletionStatus::No))
                })
                .collect(),
            _ => Err(incompatible()),
        }
    }

    /// Decodes `bytes` from the `char` transmission code set.
    pub fn decode_string(&self, bytes: &[u8]) -> Result<String, SystemException> {
        match self.char_data {
            code_set::UTF_8 => String::from_utf8(bytes.to_vec())
                .map_err(|_| data_conversion(CompletionStatus::Yes)),
            code_set::ISO_8859_1 => Ok(bytes.iter().map(|&b| char::from(b)).collect()),
            _ => Err(incompatible()),
        }
    }
}

/// The negotiation rules: the client's native code set if the server
/// uses or converts to it, else the server's native code set if the
/// client converts to it, else the first code set both convert to.
fn choose(server: &CodeSetComponent, native: u32, conversion: &[u32]) -> Option<u32> {
    if server.native_code_set == native || server.conversion_code_sets.contains(&native) {
        Some(native)
    } else if conversion.contains(&server.native_code_set) {
        Some(server.native_code_set)
    } else {
        conversion
            .iter()
            .copied()
            .find(|c| server.conversion_code_sets.contains(c))
    }
}

fn incompatible() -> SystemException {
    SystemException::new(
        SystemExceptionKind::CODESET_INCOMPATIBLE,
        0,
        CompletionStatus::No,
    )
}

fn data_conversion(completed: CompletionStatus) -> SystemException {
    SystemException::new(SystemExceptionKind::DATA_CONVERSION, 0, completed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(char_native: u32, char_conv: &[u32], wchar_native: u32) -> CodeSetComponentInfo {
        CodeSetComponentInfo {
            for_char_data: CodeSetComponent {
                native_code_set: char_native,
                conversion_code_sets: char_conv.to_vec(),
            },
            for_wchar_data: CodeSetComponent {
                native_code_set: wchar_native,
                conversion_code_sets: Vec::new(),
            },
        }
    }

    #[test]
    fn prefers_utf8_when_the_server_converts_to_it() {
        let cs = CodeSets::negotiate(&info(
            code_set::ISO_8859_1,
            &[code_set::UTF_8],
            code_set::UTF_16,
        ))
        .unwrap();
        assert_eq!(cs.char_data, code_set::UTF_8);
        assert_eq!(cs.wchar_data, Some(code_set::UTF_16));
    }

    #[test]
    fn falls_back_to_latin1() {
        let cs = CodeSets::negotiate(&info(code_set::ISO_8859_1, &[], 0)).unwrap();
        assert_eq!(cs.char_data, code_set::ISO_8859_1);
        assert_eq!(cs.wchar_data, None);
        assert_eq!(cs.encode_string("caf\u{e9}").unwrap(), b"caf\xe9");
        assert_eq!(cs.decode_string(b"caf\xe9").unwrap(), "caf\u{e9}");
        assert!(cs.encode_string("\u{3042}").is_err());
    }

    #[test]
    fn rejects_unknown_code_sets() {
        let e = CodeSets::negotiate(&info(0x0001_0020, &[], 0)).unwrap_err();
        assert!(e.is(SystemExceptionKind::CODESET_INCOMPATIBLE));
    }
}
