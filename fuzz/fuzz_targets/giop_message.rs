//! Decodes the input as one GIOP message and reads well-known bodies.
#![no_main]

use libfuzzer_sys::fuzz_target;
use rustorb_cdr::CdrDecode;
use rustorb_giop::{
    CodeSetContext, Ior, Message, MessageEncoder, ReplyStatus, SystemExceptionBody, decode_message,
};

fuzz_target!(|data: &[u8]| {
    let Ok((header, message)) = decode_message(data) else {
        return;
    };
    match &message {
        Message::Request { header: req, body } => {
            let _ = CodeSetContext::find(&req.service_context);
            let _ = body.decoder().read_string();
            // Anything that decodes must encode again in the same version.
            if let Ok(mut m) = MessageEncoder::request(header.version, header.byte_order, req) {
                m.body().write_octets(body.as_bytes());
                let _ = m.finish();
            }
        }
        Message::Reply { header: rep, body } => {
            let mut dec = body.decoder();
            match rep.reply_status {
                ReplyStatus::SystemException => drop(SystemExceptionBody::decode(&mut dec)),
                ReplyStatus::LocationForward | ReplyStatus::LocationForwardPerm => {
                    drop(Ior::decode(&mut dec))
                }
                _ => {}
            }
            let _ = MessageEncoder::reply(header.version, header.byte_order, rep);
        }
        Message::LocateRequest(req) => {
            let _ = MessageEncoder::locate_request(header.version, header.byte_order, req);
        }
        _ => {}
    }
});
