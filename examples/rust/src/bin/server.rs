//! Serves `Example::Echo` and binds it in the naming service as `Echo`.
//!
//!     cargo run --bin server -- -ORBInitRef NameService=corbaloc::localhost:2809/NameService

use std::error::Error;

use rustorb_core::{Orb, SystemException};
use rustorb_example::example::{EchoPoa, EchoServant};
use rustorb_naming::{NamingContext, to_name};

struct EchoImpl {
    orb: Orb,
}

impl EchoServant for EchoImpl {
    fn echo(&self, message: &str) -> Result<String, SystemException> {
        println!("server: {message}");
        Ok(message.to_owned())
    }

    fn shutdown(&self) -> Result<(), SystemException> {
        self.orb.shutdown(false)
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let orb = Orb::init(&mut args)?;
    let poa = orb.root_poa()?;
    poa.the_poa_manager().activate()?;
    let (_, echo) = EchoPoa(EchoImpl { orb: orb.clone() }).activate(&poa)?;

    let naming = orb.resolve_initial_references("NameService")?;
    let root = NamingContext::narrow(naming)?.ok_or("NameService is not a NamingContext")?;
    root.rebind(&to_name("Echo")?, echo.object())?;
    println!("server: ready");

    orb.run();
    orb.destroy()?;
    Ok(())
}
