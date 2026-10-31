//! Looks `Echo` up in the naming service and calls it.
//!
//!     cargo run --bin client -- -ORBInitRef NameService=corbaloc::localhost:2809/NameService [--shutdown]

use std::error::Error;

use rustorb_core::Orb;
use rustorb_example::example::Echo;
use rustorb_naming::NamingContextExt;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let orb = Orb::init(&mut args)?;
    let shutdown = args.iter().any(|a| a == "--shutdown");

    let naming = orb.resolve_initial_references("NameService")?;
    let root = NamingContextExt::narrow(naming)?.ok_or("NameService is not a NamingContextExt")?;
    let obj = root.resolve_str("Echo")?.ok_or("Echo is nil")?;
    let echo = Echo::narrow(&obj)?.ok_or("Echo is not an Example::Echo")?;

    let reply = echo.echo("hello from Rust")?;
    println!("client: {reply}");
    if shutdown {
        echo.shutdown()?;
    }
    orb.destroy()?;
    Ok(())
}
