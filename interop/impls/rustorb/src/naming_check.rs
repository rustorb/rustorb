//! Runs the CosNaming operation sequence of `impls/omniorbpy/naming_check.py`
//! with the rustorb naming client, printing the same lines, so that the
//! output can be compared with `expected/naming-check.txt`.
//!
//!     rustorb-naming-check --naming corbaloc::naming:2809/NameService

use std::process::ExitCode;
use std::time::{Duration, Instant};

use rustorb_core::{ObjectRef, Orb, SystemException, SystemExceptionKind};
use rustorb_naming::{Binding, NameComponent, NamingContext, NamingContextExt, NamingError};

fn fmt_name(name: &[NameComponent]) -> String {
    if name.is_empty() {
        return "<empty>".to_owned();
    }
    name.iter()
        .map(|c| {
            if c.kind.is_empty() {
                c.id.clone()
            } else {
                format!("{}.{}", c.id, c.kind)
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn fmt_system(e: &SystemException) -> String {
    let name = e.kind().map_or(e.repository_id.as_str(), |k| k.name());
    let completed = match e.completed {
        rustorb_core::CompletionStatus::Yes => "COMPLETED_YES",
        rustorb_core::CompletionStatus::No => "COMPLETED_NO",
        rustorb_core::CompletionStatus::Maybe => "COMPLETED_MAYBE",
    };
    format!("{name}(completed={completed})")
}

fn fmt_exc(e: &NamingError) -> String {
    match e {
        NamingError::NotFound { why, rest_of_name } => {
            format!("NotFound({why}, rest={})", fmt_name(rest_of_name))
        }
        NamingError::CannotProceed { rest_of_name, .. } => {
            format!("CannotProceed(rest={})", fmt_name(rest_of_name))
        }
        NamingError::InvalidName => "InvalidName".to_owned(),
        NamingError::AlreadyBound => "AlreadyBound".to_owned(),
        NamingError::NotEmpty => "NotEmpty".to_owned(),
        NamingError::InvalidAddress => "InvalidAddress".to_owned(),
        NamingError::System(e) => fmt_system(e),
    }
}

/// Python's `repr` of a string, for the plain strings used here.
fn py_repr(s: &str) -> String {
    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
}

fn fmt_components(name: &[NameComponent]) -> String {
    let parts: Vec<_> = name
        .iter()
        .map(|c| format!("({}, {})", py_repr(&c.id), py_repr(&c.kind)))
        .collect();
    format!("[{}]", parts.join(", "))
}

fn fmt_bindings(bindings: &[Binding]) -> String {
    let mut items: Vec<_> = bindings
        .iter()
        .map(|b| (fmt_name(&b.binding_name), b.binding_type.name()))
        .collect();
    items.sort();
    let items: Vec<_> = items.iter().map(|(n, t)| format!("{n}:{t}")).collect();
    format!("[{}]", items.join(", "))
}

fn py_bool(b: bool) -> &'static str {
    if b { "True" } else { "False" }
}

/// `path` as a name: components split at `/`, id and kind at the first `.`.
fn n(path: &str) -> Vec<NameComponent> {
    path.split('/')
        .filter(|c| !c.is_empty())
        .map(|c| match c.split_once('.') {
            Some((id, kind)) => NameComponent::new(id, kind),
            None => NameComponent::new(c, ""),
        })
        .collect()
}

fn step<T>(
    label: &str,
    f: impl FnOnce() -> Result<T, NamingError>,
    show: impl FnOnce(&T) -> String,
) -> Option<T> {
    match f() {
        Ok(v) => {
            println!("{label:<40} -> {}", show(&v));
            Some(v)
        }
        Err(e) => {
            println!("{label:<40} -> raises {}", fmt_exc(&e));
            None
        }
    }
}

fn ok<T>(_: &T) -> String {
    "ok".to_owned()
}

fn root_context(orb: &Orb, naming: &str, timeout: Duration) -> Result<NamingContext, String> {
    let deadline = Instant::now() + timeout;
    loop {
        let attempt = orb.string_to_object(naming).and_then(|obj| {
            let obj = obj.ok_or_else(|| {
                SystemException::new(
                    SystemExceptionKind::INV_OBJREF,
                    0,
                    rustorb_core::CompletionStatus::No,
                )
            })?;
            NamingContext::narrow(obj)
        });
        match attempt {
            Ok(Some(root)) => return Ok(root),
            Ok(None) => return Err(format!("{naming} is not a naming context")),
            Err(e)
                if (e.is(SystemExceptionKind::TRANSIENT)
                    || e.is(SystemExceptionKind::COMM_FAILURE))
                    && Instant::now() < deadline =>
            {
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(e) => return Err(format!("the naming service: {e}")),
        }
    }
}

fn run() -> Result<(), String> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut naming = None;
    let mut timeout = Duration::from_secs(30);
    let mut orb_args = Vec::new();
    let mut iter = args.drain(..);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--naming" => naming = iter.next(),
            "--timeout" => {
                let v = iter.next().ok_or("--timeout needs a value")?;
                timeout = Duration::from_secs_f64(v.parse().map_err(|_| "bad --timeout")?);
            }
            _ => orb_args.push(arg),
        }
    }
    drop(iter);
    let naming = naming.ok_or("--naming is required")?;
    orb_args.extend(["-ORBInitRef".to_owned(), format!("NameService={naming}")]);
    let orb = Orb::init(&mut orb_args).map_err(|e| format!("ORB_init: {e}"))?;
    if !orb_args.is_empty() {
        return Err(format!("unknown arguments: {orb_args:?}"));
    }
    let root = root_context(&orb, &naming, timeout)?;
    let ext =
        NamingContextExt::narrow(root.object().clone()).map_err(|e| format!("narrow: {e}"))?;

    let Some(t) = step(
        "bind_new_context test",
        || root.bind_new_context(&n("test")),
        ok,
    ) else {
        return Err("cannot create the test context".into());
    };
    // Any object reference will do. The test context is used rather than
    // the root (as the omniORBpy check does) because its IOR comes from the
    // naming service, while the root's was made from the corbaloc URL, and
    // the comparisons below are by IOR.
    let obj: &ObjectRef = t.object();
    let equivalent = |o: &Option<ObjectRef>| match o {
        Some(o) if o.is_equivalent(obj) => "equivalent".to_owned(),
        _ => "different object".to_owned(),
    };
    let is_context = |o: &Option<ObjectRef>| {
        let context = o
            .clone()
            .and_then(|o| NamingContext::narrow(o).ok().flatten())
            .is_some();
        if context {
            "NamingContext"
        } else {
            "not a context"
        }
        .to_owned()
    };

    step("bind test/a", || root.bind(&n("test/a"), obj), ok);
    step("bind test/a (again)", || root.bind(&n("test/a"), obj), ok);
    step("rebind test/a", || root.rebind(&n("test/a"), obj), ok);
    step("resolve test/a", || root.resolve(&n("test/a")), equivalent);
    step("bind test/b.kind", || t.bind(&n("b.kind"), obj), ok);
    step(
        "resolve test/b (no kind)",
        || root.resolve(&n("test/b")),
        ok,
    );
    step(
        "resolve test/b.kind",
        || root.resolve(&n("test/b.kind")),
        equivalent,
    );
    step(
        "resolve test/missing",
        || root.resolve(&n("test/missing")),
        ok,
    );
    step(
        "resolve test/missing/deeper",
        || root.resolve(&n("test/missing/deeper")),
        ok,
    );
    step(
        "resolve test/a/x (not a context)",
        || root.resolve(&n("test/a/x")),
        ok,
    );
    step("resolve <empty>", || root.resolve(&[]), ok);
    step("bind <empty>", || root.bind(&[], obj), ok);

    let sub = step(
        "bind_new_context test/sub",
        || root.bind_new_context(&n("test/sub")),
        ok,
    );
    step(
        "bind_new_context test/sub (again)",
        || root.bind_new_context(&n("test/sub")),
        ok,
    );
    step("bind test/sub/c", || root.bind(&n("test/sub/c"), obj), ok);
    step(
        "resolve test/sub (is a context)",
        || root.resolve(&n("test/sub")),
        is_context,
    );
    let loose = step("new_context", || root.new_context(), ok);
    if let Some(loose) = &loose {
        step(
            "bind_context test/loose",
            || root.bind_context(&n("test/loose"), loose),
            ok,
        );
        step(
            "rebind_context test/loose",
            || root.rebind_context(&n("test/loose"), loose),
            ok,
        );
    }

    step(
        "list test (how_many=10)",
        || t.list(10),
        |(bl, it)| {
            let more = if it.is_some() { " + iterator" } else { "" };
            format!("{}{more}", fmt_bindings(bl))
        },
    );
    step(
        "list test (how_many=1) + iterator",
        || {
            let (bl, mut it) = t.list(1)?;
            let mut names = bl.clone();
            let mut steps = vec![format!("first={}", bl.len())];
            while let Some(iter) = &it {
                let (more, chunk) = iter.next_n(2)?;
                steps.push(format!("next_n(2)={}/{}", py_bool(more), chunk.len()));
                names.extend(chunk);
                if !more {
                    iter.destroy()?;
                    it = None;
                }
            }
            Ok(format!("{} {}", fmt_bindings(&names), steps.join(" ")))
        },
        String::clone,
    );

    if let Some(sub) = &sub {
        step("destroy test/sub (not empty)", || sub.destroy(), ok);
        step("unbind test/sub/c", || root.unbind(&n("test/sub/c")), ok);
        step("destroy test/sub", || sub.destroy(), ok);
    }
    step(
        "resolve test/sub (dangling)",
        || root.resolve(&n("test/sub")),
        |_| "still bound".to_owned(),
    );
    step("unbind test/sub", || root.unbind(&n("test/sub")), ok);
    step("unbind test/a", || root.unbind(&n("test/a")), ok);
    step("unbind test/a (again)", || root.unbind(&n("test/a")), ok);
    step("resolve test/a", || root.resolve(&n("test/a")), ok);

    match &ext {
        Some(ext) => {
            let name = [
                NameComponent::new("a", "b"),
                NameComponent::new("c/d", ""),
                NameComponent::new("", "e"),
            ];
            step(
                "to_string [a.b, c/d, .e]",
                || ext.to_string(&name),
                String::clone,
            );
            step(
                "to_name a.b/c\\/d/.e",
                || ext.to_name("a.b/c\\/d/.e"),
                |n| fmt_components(n),
            );
            step(
                "to_url host:2809 a/b",
                || ext.to_url("host:2809", "a/b"),
                String::clone,
            );
            step(
                "to_url :host:2809 a/b c",
                || ext.to_url(":host:2809", "a/b c"),
                String::clone,
            );
            step(
                "resolve_str test/b.kind",
                || ext.resolve_str("test/b.kind"),
                equivalent,
            );
            step(
                "to_name <invalid>",
                || ext.to_name("a//b"),
                |n| fmt_components(n),
            );
        }
        None => println!("root context is not a NamingContextExt"),
    }

    // corbaname: URLs, resolved by the ORB through the naming service.
    let base = naming.replacen("corbaloc:", "corbaname:", 1);
    let orb_ref = &orb;
    let url = |s: String| move || orb_ref.string_to_object(&s).map_err(NamingError::System);
    step(
        "corbaname #test/b.kind",
        url(format!("{base}#test/b.kind")),
        equivalent,
    );
    step(
        "corbaname #test%2Fb%2ekind",
        url(format!("{base}#test%2Fb%2ekind")),
        equivalent,
    );
    step(
        "corbaname rir: #test/b.kind",
        url("corbaname:rir:#test/b.kind".to_owned()),
        equivalent,
    );
    step("corbaname (no name)", url(base.clone()), is_context);
    step(
        "corbaname #test/loose",
        url(format!("{base}#test/loose")),
        is_context,
    );
    step(
        "corbaname #test/missing",
        url(format!("{base}#test/missing")),
        ok,
    );
    step(
        "corbaname #test//b (invalid)",
        url(format!("{base}#test//b")),
        ok,
    );

    step(
        "list test (final)",
        || t.list(100),
        |(bl, _)| fmt_bindings(bl),
    );
    let _ = orb.shutdown(true);
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
