//! `rorb_object_sendc`: the AMI send of the C ABI (`docs/ami.md`).

use std::ffi::{c_char, c_void};
use std::sync::Mutex;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use rustorb_core::{
    CompletionStatus, EndpointAddr, InStream, ObjectRef, Orb, OrbOptions, Servant, ServerReply,
    SystemException, SystemExceptionKind, minor,
};
use rustorb_ffi::object::{Invocation, rorb_object_sendc};
use rustorb_ffi::stream::{RorbIn, RorbOut, rorb_in_read_long, rorb_out_write_long};
use rustorb_ffi::{
    CALLBACK_FAILED, Exception, OK, SYSTEM_EXCEPTION, Status, USER_EXCEPTION, rorb_exception_clear,
};

const OOPS_ID: &str = "IDL:test/Oops:1.0";

struct TestServant;

impl Servant for TestServant {
    fn repository_id(&self) -> &str {
        "IDL:test/Sendc:1.0"
    }

    fn dispatch(
        &self,
        operation: &str,
        input: &mut InStream<'_>,
        reply: &mut ServerReply,
    ) -> Result<(), SystemException> {
        match operation {
            "double" => {
                let v = input.read::<i32>()?;
                reply.output().write_i32(2 * v);
                Ok(())
            }
            "fail" => {
                let v = input.read::<i32>()?;
                reply.user_exception(OOPS_ID)?.write_i32(v);
                Ok(())
            }
            "sleep" => {
                thread::sleep(Duration::from_millis(500));
                Ok(())
            }
            _ => Err(SystemException::new(
                SystemExceptionKind::BAD_OPERATION,
                minor::UNKNOWN_OPERATION,
                CompletionStatus::No,
            )),
        }
    }
}

/// What the callbacks saw, sent once `done` is called.
#[derive(Debug, Default)]
struct Seen {
    writes: usize,
    read: Option<i32>,
    user: Option<(String, i32)>,
    status: Status,
    system: Option<(String, u32)>,
    thread: Option<String>,
}

struct Context {
    arg: i32,
    fail_write: bool,
    seen: Mutex<Seen>,
    tx: Mutex<mpsc::Sender<Seen>>,
}

extern "C" fn write_args(ctx: *mut c_void, out: *mut RorbOut) -> Status {
    let ctx = unsafe { &*(ctx as *const Context) };
    ctx.seen.lock().unwrap().writes += 1;
    if ctx.fail_write {
        return CALLBACK_FAILED;
    }
    unsafe { rorb_out_write_long(out, ctx.arg) };
    OK
}

extern "C" fn read_reply(ctx: *mut c_void, input: *mut RorbIn) -> Status {
    let ctx = unsafe { &*(ctx as *const Context) };
    ctx.seen.lock().unwrap().read = Some(unsafe { rorb_in_read_long(input) });
    OK
}

extern "C" fn read_user_exception(
    ctx: *mut c_void,
    id: *const c_char,
    len: usize,
    input: *mut RorbIn,
) -> Status {
    let ctx = unsafe { &*(ctx as *const Context) };
    let id = unsafe { std::slice::from_raw_parts(id as *const u8, len) };
    let id = String::from_utf8(id.to_vec()).unwrap();
    let v = unsafe { rorb_in_read_long(input) };
    ctx.seen.lock().unwrap().user = Some((id, v));
    USER_EXCEPTION
}

/// Takes back the context, as a binding releases it in `done`.
extern "C" fn done(ctx: *mut c_void, status: Status, exc: *const Exception) {
    let ctx = unsafe { Box::from_raw(ctx as *mut Context) };
    let mut seen = std::mem::take(&mut *ctx.seen.lock().unwrap());
    seen.status = status;
    seen.thread = thread::current().name().map(str::to_owned);
    if status == SYSTEM_EXCEPTION {
        let exc = unsafe { &*exc };
        let id = unsafe {
            std::slice::from_raw_parts(exc.repository_id as *const u8, exc.repository_id_len)
        };
        seen.system = Some((String::from_utf8(id.to_vec()).unwrap(), exc.minor));
    }
    let _ = ctx.tx.lock().unwrap().send(seen);
}

fn empty() -> Exception {
    Exception {
        repository_id: std::ptr::null_mut(),
        repository_id_len: 0,
        minor: 0,
        completed: 0,
        local: 0,
        index: 0,
    }
}

/// Sends `op` with `arg`; the status of rorb_object_sendc and what `done`
/// will see.
fn sendc(
    obj: &ObjectRef,
    op: &str,
    arg: i32,
    fail_write: bool,
) -> (Status, mpsc::Receiver<Seen>, Exception) {
    let (tx, rx) = mpsc::channel();
    let ctx = Box::into_raw(Box::new(Context {
        arg,
        fail_write,
        seen: Mutex::default(),
        tx: Mutex::new(tx),
    }));
    // The operation name only lives for the call.
    let name = op.to_owned();
    let inv = Invocation {
        operation: name.as_ptr() as *const c_char,
        operation_len: name.len(),
        oneway: 0,
        timeout_us: -1,
        context: ctx.cast(),
        write_args: Some(write_args),
        read_reply: Some(read_reply),
        read_user_exception: Some(read_user_exception),
    };
    let mut exc = empty();
    let handle = Box::into_raw(Box::new(obj.clone()));
    let status = unsafe { rorb_object_sendc(handle, &inv, Some(done), &mut exc) };
    drop(unsafe { Box::from_raw(handle) });
    drop(name);
    if status != OK {
        drop(unsafe { Box::from_raw(ctx) });
    }
    (status, rx, exc)
}

fn wait(rx: &mpsc::Receiver<Seen>) -> Seen {
    let seen = rx.recv_timeout(Duration::from_secs(10)).expect("done");
    // Exactly once.
    assert!(rx.recv_timeout(Duration::from_millis(50)).is_err());
    seen
}

fn setup() -> (Orb, Orb, ObjectRef) {
    let server = Orb::with_options(OrbOptions {
        endpoints: vec![EndpointAddr {
            host: "127.0.0.1".into(),
            port: 0,
        }],
        ..OrbOptions::default()
    })
    .unwrap();
    let root = server.root_poa().unwrap();
    let id = root
        .activate_object(std::sync::Arc::new(TestServant))
        .unwrap();
    let obj = root.id_to_reference(&id).unwrap();
    root.the_poa_manager().activate().unwrap();
    let client = Orb::with_options(OrbOptions::default()).unwrap();
    let target = client
        .string_to_object(&server.object_to_string(Some(&obj)).unwrap())
        .unwrap()
        .unwrap();
    (server, client, target)
}

#[test]
fn replies_and_exceptions_reach_done() {
    let (server, client, target) = setup();

    let (st, rx, _) = sendc(&target, "double", 21, false);
    assert_eq!(st, OK);
    let seen = wait(&rx);
    assert_eq!(seen.status, OK);
    assert_eq!(seen.read, Some(42));
    assert_eq!(seen.writes, 1);
    assert_eq!(seen.thread.as_deref(), Some("rustorb-upcall"));

    let seen = wait(&sendc(&target, "fail", 3, false).1);
    assert_eq!(seen.status, USER_EXCEPTION);
    assert_eq!(seen.user, Some((OOPS_ID.to_owned(), 3)));

    let seen = wait(&sendc(&target, "nope", 0, false).1);
    assert_eq!(seen.status, SYSTEM_EXCEPTION);
    assert_eq!(
        seen.system,
        Some((
            "IDL:omg.org/CORBA/BAD_OPERATION:1.0".to_owned(),
            minor::UNKNOWN_OPERATION
        ))
    );

    let seen = wait(&sendc(&target, "double", 1, true).1);
    assert_eq!(seen.status, CALLBACK_FAILED);
    assert_eq!(seen.read, None);

    client.destroy().unwrap();
    server.destroy().unwrap();
}

#[test]
fn refused_sends_do_not_call_done() {
    let (server, client, target) = setup();
    let handle = Box::into_raw(Box::new(target.clone()));

    // A oneway operation, or no done.
    let mut inv = Invocation {
        operation: "double".as_ptr() as *const c_char,
        operation_len: 6,
        oneway: 1,
        timeout_us: -1,
        context: std::ptr::null_mut(),
        write_args: None,
        read_reply: None,
        read_user_exception: None,
    };
    let mut exc = empty();
    assert_eq!(
        unsafe { rorb_object_sendc(handle, &inv, Some(done), &mut exc) },
        SYSTEM_EXCEPTION
    );
    unsafe { rorb_exception_clear(&mut exc) };
    inv.oneway = 0;
    assert_eq!(
        unsafe { rorb_object_sendc(handle, &inv, None, &mut exc) },
        SYSTEM_EXCEPTION
    );
    unsafe { rorb_exception_clear(&mut exc) };
    drop(unsafe { Box::from_raw(handle) });

    // Waiting when the client ORB is destroyed: done still runs once.
    let (st, rx, _) = sendc(&target, "sleep", 0, false);
    assert_eq!(st, OK);
    thread::sleep(Duration::from_millis(100));
    client.destroy().unwrap();
    let seen = wait(&rx);
    assert_eq!(seen.status, SYSTEM_EXCEPTION);

    // Once shut down, sending fails.
    let (st, _rx, mut exc) = sendc(&target, "double", 1, false);
    assert_eq!(st, SYSTEM_EXCEPTION);
    assert_eq!(exc.minor, minor::ORB_SHUT_DOWN);
    unsafe { rorb_exception_clear(&mut exc) };

    server.destroy().unwrap();
}
