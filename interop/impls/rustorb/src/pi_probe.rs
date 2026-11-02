//! The rustorb side of `run-pi-probe.sh`: Portable Interceptors between
//! rustorb and another ORB (docs/portable-interceptors.md).
//!
//!   rustorb-pi-probe server [ORB options]   prints the IOR of an Echo
//!                                            object, then serves
//!   rustorb-pi-probe client IOR [ORB options]
//!
//! Both sides carry a request service context and a reply service context
//! with the id [`CONTEXT`], and the server's IOR interceptor adds a tagged
//! component with the tag [`TAG`]; each holds a string written with
//! `IOP::Codec` (CDR encapsulation 1.2, `encode_value`). The server reads
//! the request context in `receive_request_service_contexts`, keeps it in
//! a PICurrent slot until `send_reply`, and answers with
//! "<name> got: <request context>". The client prints the reply context,
//! the component and the result; the peer's probe (interop.PiProbe on
//! JacORB) prints the same lines, so the two can be compared.

use std::sync::{Arc, Mutex};

use rustorb_core::iop::{Codec, Encoding, ServiceContext, TaggedComponent};
use rustorb_core::pi::{
    ClientRequestInfo, ClientRequestInterceptor, InterceptError, Interceptor, IorInfo,
    IorInterceptor, OrbInitInfo, OrbInitializer, ServerRequestInfo, ServerRequestInterceptor,
    SlotId,
};
use rustorb_core::{
    Any, InStream, Orb, OrbOptions, Servant, ServerReply, SystemException, TypeCode, Value,
};

/// The service context id of the probe ("PIPR").
const CONTEXT: u32 = 0x5049_5052;
/// The tagged component tag of the probe ("PIPR").
const TAG: u32 = 0x5049_5052;
const NAME: &str = "rustorb";
const ECHO_ID: &str = "IDL:PiProbe/Echo:1.0";

fn encode(codec: &Codec, s: &str) -> Vec<u8> {
    codec
        .encode_value(&Any::new(TypeCode::string(0), Value::String(s.into())))
        .unwrap()
}

fn decode(codec: &Codec, data: &[u8]) -> String {
    match codec.decode_value(data, &TypeCode::string(0)) {
        Ok(any) => match any.value() {
            Value::String(s) => s.clone(),
            v => format!("<{v:?}>"),
        },
        Err(e) => format!("<{e}>"),
    }
}

fn string_of(any: &Any) -> String {
    match any.value() {
        Value::String(s) => s.clone(),
        _ => "<none>".into(),
    }
}

struct Probe {
    codec: Codec,
    slot: SlotId,
    /// What the client saw in `receive_reply`.
    seen: Mutex<Vec<String>>,
}

impl Interceptor for Probe {
    fn name(&self) -> &str {
        "pi-probe"
    }
}

impl ClientRequestInterceptor for Probe {
    fn send_request(&self, ri: &ClientRequestInfo<'_>) -> Result<(), InterceptError> {
        ri.add_request_service_context(
            ServiceContext {
                context_id: CONTEXT,
                context_data: encode(&self.codec, &format!("from {NAME}")),
            },
            false,
        )?;
        Ok(())
    }

    fn receive_reply(&self, ri: &ClientRequestInfo<'_>) -> Result<(), SystemException> {
        let mut seen = self.seen.lock().unwrap();
        seen.push(match ri.get_reply_service_context(CONTEXT) {
            Ok(sc) => format!("reply context: {}", decode(&self.codec, &sc.context_data)),
            Err(e) => format!("reply context: {}", e.repository_id),
        });
        seen.push(match ri.get_effective_component(TAG) {
            Ok(c) => format!("component: {}", decode(&self.codec, &c.component_data)),
            Err(e) => format!("component: {}", e.repository_id),
        });
        Ok(())
    }
}

impl ServerRequestInterceptor for Probe {
    fn receive_request_service_contexts(
        &self,
        ri: &ServerRequestInfo<'_>,
    ) -> Result<(), InterceptError> {
        let got = match ri.get_request_service_context(CONTEXT) {
            Ok(sc) => decode(&self.codec, &sc.context_data),
            Err(e) => e.repository_id,
        };
        ri.set_slot(self.slot, Any::new(TypeCode::string(0), Value::String(got)))
            .unwrap();
        Ok(())
    }

    fn send_reply(&self, ri: &ServerRequestInfo<'_>) -> Result<(), SystemException> {
        let got = string_of(&ri.get_slot(self.slot).unwrap());
        ri.add_reply_service_context(
            ServiceContext {
                context_id: CONTEXT,
                context_data: encode(&self.codec, &format!("{NAME} got: {got}")),
            },
            false,
        )
    }
}

impl IorInterceptor for Probe {
    fn establish_components(&self, info: &IorInfo<'_>) -> Result<(), SystemException> {
        info.add_ior_component(TaggedComponent {
            tag: TAG,
            component_data: encode(&self.codec, &format!("{NAME} component")),
        });
        Ok(())
    }
}

struct Init(Mutex<Option<Arc<Probe>>>);

impl OrbInitializer for Init {
    fn pre_init(&self, info: &mut OrbInitInfo<'_>) -> Result<(), SystemException> {
        let probe = Arc::new(Probe {
            codec: info
                .codec_factory()
                .create_codec(Encoding::cdr(1, 2))
                .unwrap(),
            slot: info.allocate_slot_id(),
            seen: Mutex::default(),
        });
        info.add_client_request_interceptor(probe.clone()).unwrap();
        info.add_server_request_interceptor(probe.clone()).unwrap();
        info.add_ior_interceptor(probe.clone()).unwrap();
        *self.0.lock().unwrap() = Some(probe);
        Ok(())
    }
}

struct Echo;

impl Servant for Echo {
    fn repository_id(&self) -> &str {
        ECHO_ID
    }

    fn dispatch(
        &self,
        operation: &str,
        input: &mut InStream<'_>,
        reply: &mut ServerReply,
    ) -> Result<(), SystemException> {
        match operation {
            "echo" => {
                let s = input.read_string()?;
                reply.output().write_string(&s)?;
                Ok(())
            }
            _ => Err(SystemException::new(
                rustorb_core::SystemExceptionKind::BAD_OPERATION,
                0,
                rustorb_core::CompletionStatus::No,
            )),
        }
    }
}

fn main() {
    let mut args: Vec<String> = std::env::args().collect();
    let init = Arc::new(Init(Mutex::default()));
    let mut options = OrbOptions {
        arguments: args.clone(),
        initializers: vec![init.clone()],
        ..OrbOptions::default()
    };
    options.parse_args(&mut args).expect("ORB options");
    let orb = Orb::with_options(options).expect("ORB_init");
    let probe = init.0.lock().unwrap().clone().unwrap();
    match args.get(1).map(String::as_str) {
        Some("server") => {
            let root = orb.root_poa().unwrap();
            let id = root.activate_object(Arc::new(Echo)).unwrap();
            root.the_poa_manager().activate().unwrap();
            let obj = root.id_to_reference(&id).unwrap();
            println!("{}", orb.object_to_string(Some(&obj)).unwrap());
            orb.run();
        }
        Some("client") => {
            let ior = args.get(2).expect("client IOR");
            let obj = orb.string_to_object(ior).unwrap().expect("a reference");
            let reply = obj
                .request("echo")
                .args(|out| out.write_string("hello"))
                .invoke();
            for line in probe.seen.lock().unwrap().iter() {
                println!("{line}");
            }
            match reply {
                Ok(reply) => println!("result: {}", reply.input().read_string().unwrap()),
                Err(e) => println!("result: {e:?}"),
            }
            let _ = orb.destroy();
        }
        _ => {
            eprintln!("usage: rustorb-pi-probe server|client IOR [ORB options]");
            std::process::exit(2);
        }
    }
}
