use rustorb_cdr::{ByteOrder, CdrDecode, CdrEncode, CdrVersion, Decoder, Encoder};

/// `IOP::ServiceContext`, with the context data left as an encapsulation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceContext {
    /// Context id such as [`service_id::CODE_SETS`].
    pub context_id: u32,
    /// Encapsulated context data.
    pub context_data: Vec<u8>,
}

impl CdrEncode for ServiceContext {
    fn encode(&self, enc: &mut Encoder) -> rustorb_cdr::Result<()> {
        enc.write_u32(self.context_id);
        enc.write_octet_seq(&self.context_data)
    }
}

impl CdrDecode for ServiceContext {
    fn decode(dec: &mut Decoder<'_>) -> rustorb_cdr::Result<Self> {
        Ok(ServiceContext {
            context_id: dec.read_u32()?,
            context_data: dec.read_octet_seq()?.to_vec(),
        })
    }
}

/// Standard `IOP::ServiceId` values.
pub mod service_id {
    /// `TransactionService`.
    pub const TRANSACTION_SERVICE: u32 = 0;
    /// `CodeSets`, carrying a [`CodeSetContext`](crate::CodeSetContext).
    pub const CODE_SETS: u32 = 1;
    /// `ChainBypassCheck`.
    pub const CHAIN_BYPASS_CHECK: u32 = 2;
    /// `ChainBypassInfo`.
    pub const CHAIN_BYPASS_INFO: u32 = 3;
    /// `LogicalThreadId`.
    pub const LOGICAL_THREAD_ID: u32 = 4;
    /// `BI_DIR_IIOP`.
    pub const BI_DIR_IIOP: u32 = 5;
    /// `SendingContextRunTime`.
    pub const SENDING_CONTEXT_RUN_TIME: u32 = 6;
    /// `INVOCATION_POLICIES`.
    pub const INVOCATION_POLICIES: u32 = 7;
    /// `FORWARDED_IDENTITY`.
    pub const FORWARDED_IDENTITY: u32 = 8;
    /// `UnknownExceptionInfo`.
    pub const UNKNOWN_EXCEPTION_INFO: u32 = 9;
    /// `RTCorbaPriority`.
    pub const RT_CORBA_PRIORITY: u32 = 10;
    /// `RTCorbaPriorityRange`.
    pub const RT_CORBA_PRIORITY_RANGE: u32 = 11;
    /// `FT_GROUP_VERSION`.
    pub const FT_GROUP_VERSION: u32 = 12;
    /// `FT_REQUEST`.
    pub const FT_REQUEST: u32 = 13;
    /// `ExceptionDetailMessage`.
    pub const EXCEPTION_DETAIL_MESSAGE: u32 = 14;
    /// `SecurityAttributeService`.
    pub const SECURITY_ATTRIBUTE_SERVICE: u32 = 15;
    /// `ActivityService`.
    pub const ACTIVITY_SERVICE: u32 = 16;
}

/// OSF registry ids of common code sets (`CONV_FRAME::CodeSetId`).
pub mod code_set {
    /// ISO 8859-1 (Latin-1), the default `char` code set.
    pub const ISO_8859_1: u32 = 0x0001_0001;
    /// UCS-2 level 1.
    pub const UCS_2_LEVEL_1: u32 = 0x0001_0100;
    /// UTF-16.
    pub const UTF_16: u32 = 0x0001_0109;
    /// UTF-8.
    pub const UTF_8: u32 = 0x0501_0001;
}

/// `CONV_FRAME::CodeSetContext`, the transmission code sets chosen by a
/// client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodeSetContext {
    /// Transmission code set for `char` and `string`.
    pub char_data: u32,
    /// Transmission code set for `wchar` and `wstring`.
    pub wchar_data: u32,
}

impl CodeSetContext {
    /// Wraps the context in a [`service_id::CODE_SETS`] service context.
    pub fn to_service_context(&self, order: ByteOrder) -> ServiceContext {
        let mut enc = Encoder::encapsulation(order, CdrVersion::Giop1_2);
        enc.write_u32(self.char_data);
        enc.write_u32(self.wchar_data);
        ServiceContext {
            context_id: service_id::CODE_SETS,
            context_data: enc.into_bytes(),
        }
    }

    /// Reads the context from a service context's data.
    pub fn from_service_context(context: &ServiceContext) -> rustorb_cdr::Result<CodeSetContext> {
        let mut dec = Decoder::encapsulation(&context.context_data, CdrVersion::Giop1_2)?;
        Ok(CodeSetContext {
            char_data: dec.read_u32()?,
            wchar_data: dec.read_u32()?,
        })
    }

    /// Finds and reads the code set context in a service context list.
    pub fn find(contexts: &[ServiceContext]) -> Option<rustorb_cdr::Result<CodeSetContext>> {
        contexts
            .iter()
            .find(|c| c.context_id == service_id::CODE_SETS)
            .map(CodeSetContext::from_service_context)
    }
}
