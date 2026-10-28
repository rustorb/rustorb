//! The naming service, called by the rustorb stubs from another ORB.

use std::net::TcpListener;
use std::path::PathBuf;

use rustorb_core::{ObjectRef, Orb, SystemExceptionKind, minor};
use rustorb_naming::{
    BindingType, LogStore, MemoryStore, NameComponent, NamingContext, NamingContextExt,
    NamingError, NamingService, NotFoundReason, to_name,
};

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn server_orb(port: u16) -> Orb {
    let mut args = vec![
        "-ORBendPoint".to_owned(),
        format!("giop:tcp:127.0.0.1:{port}"),
    ];
    Orb::init(&mut args).unwrap()
}

fn client_root(port: u16) -> (Orb, NamingContextExt) {
    let orb = Orb::init(&mut Vec::new()).unwrap();
    let obj = orb
        .string_to_object(&format!("corbaloc::127.0.0.1:{port}/NameService"))
        .unwrap()
        .unwrap();
    let root = NamingContextExt::narrow(obj)
        .unwrap()
        .expect("a NamingContextExt");
    (orb, root)
}

fn n(s: &str) -> Vec<NameComponent> {
    to_name(s).unwrap()
}

fn not_found(e: NamingError) -> (NotFoundReason, String) {
    match e {
        NamingError::NotFound { why, rest_of_name } => {
            (why, rustorb_naming::to_string(&rest_of_name).unwrap())
        }
        e => panic!("expected NotFound, got {e}"),
    }
}

fn names(bindings: &[rustorb_naming::Binding]) -> Vec<String> {
    bindings
        .iter()
        .map(|b| {
            let t = match b.binding_type {
                BindingType::Object => "o",
                BindingType::Context => "c",
            };
            format!(
                "{}:{t}",
                rustorb_naming::to_string(&b.binding_name).unwrap()
            )
        })
        .collect()
}

#[test]
fn operations_behave_like_omninames() {
    let port = free_port();
    let orb = server_orb(port);
    let _service = NamingService::start(&orb, MemoryStore).unwrap();
    let (_client, root) = client_root(port);
    let obj: ObjectRef = root.object().clone();

    let t = root.bind_new_context(&n("t")).unwrap();
    root.bind(&n("t/a"), &obj).unwrap();
    assert!(matches!(
        root.bind(&n("t/a"), &obj),
        Err(NamingError::AlreadyBound)
    ));
    root.rebind(&n("t/a"), &obj).unwrap();
    assert!(
        root.resolve(&n("t/a"))
            .unwrap()
            .unwrap()
            .is_equivalent(&obj)
    );
    t.bind(&n("b.kind"), &obj).unwrap();
    assert_eq!(
        not_found(root.resolve(&n("t/b")).unwrap_err()),
        (NotFoundReason::MissingNode, "b".into())
    );
    assert_eq!(
        not_found(root.resolve(&n("t/missing/deeper")).unwrap_err()),
        (NotFoundReason::MissingNode, "missing/deeper".into())
    );
    assert_eq!(
        not_found(root.resolve(&n("t/a/x")).unwrap_err()),
        (NotFoundReason::NotContext, "a/x".into())
    );
    assert!(matches!(root.resolve(&[]), Err(NamingError::InvalidName)));
    assert!(matches!(
        root.bind(&[], &obj),
        Err(NamingError::InvalidName)
    ));
    assert!(matches!(root.unbind(&[]), Err(NamingError::InvalidName)));

    // rebind and rebind_context replace a binding of either type.
    let sub = root.bind_new_context(&n("t/sub")).unwrap();
    root.bind(&n("t/sub/c"), &obj).unwrap();
    assert!(matches!(
        root.bind_new_context(&n("t/sub")),
        Err(NamingError::AlreadyBound)
    ));
    assert!(matches!(sub.destroy(), Err(NamingError::NotEmpty)));
    let loose = root.new_context().unwrap();
    root.bind_context(&n("t/loose"), &loose).unwrap();
    root.rebind(&n("t/loose"), &obj).unwrap();
    root.rebind_context(&n("t/loose"), &loose).unwrap();
    assert!(
        NamingContext::narrow(root.resolve_str("t/loose").unwrap().unwrap())
            .unwrap()
            .is_some()
    );

    let (all, it) = t.list(10).unwrap();
    assert_eq!(names(&all), ["a:o", "b.kind:o", "loose:c", "sub:c"]);
    assert!(it.is_none());
    let (first, it) = t.list(1).unwrap();
    assert_eq!(names(&first), ["a:o"]);
    let it = it.expect("an iterator for the rest");
    assert_eq!(it.next_n(0).unwrap(), (false, Vec::new()));
    assert_eq!(names(&[it.next_one().unwrap().unwrap()]), ["b.kind:o"]);
    let (more, chunk) = it.next_n(5).unwrap();
    assert!(more);
    assert_eq!(names(&chunk), ["loose:c", "sub:c"]);
    assert_eq!(it.next_one().unwrap(), None);
    assert_eq!(it.next_n(5).unwrap(), (false, Vec::new()));
    it.destroy().unwrap();
    let e = it.next_one().unwrap_err();
    let e = e.as_system().expect("a system exception");
    assert!(e.is(SystemExceptionKind::OBJECT_NOT_EXIST));
    assert_eq!(e.minor, minor::NO_SUCH_OBJECT);

    // A destroyed context is gone, but names bound to it stay.
    root.unbind(&n("t/sub/c")).unwrap();
    sub.destroy().unwrap();
    assert!(sub.object().non_existent().unwrap());
    root.resolve(&n("t/sub")).unwrap().unwrap();
    let e = root.resolve(&n("t/sub/c")).unwrap_err();
    assert!(
        e.as_system()
            .unwrap()
            .is(SystemExceptionKind::OBJECT_NOT_EXIST)
    );

    root.unbind(&n("t/a")).unwrap();
    assert_eq!(
        not_found(root.unbind(&n("t/a")).unwrap_err()),
        (NotFoundReason::MissingNode, "a".into())
    );

    assert_eq!(
        root.to_string(&[NameComponent::new("a", "b"), NameComponent::new("c/d", "")])
            .unwrap(),
        "a.b/c\\/d"
    );
    assert_eq!(root.to_name("a.b/c").unwrap(), n("a.b/c"));
    assert!(matches!(
        root.to_name("a//b"),
        Err(NamingError::InvalidName)
    ));
    assert!(matches!(
        root.to_url("host:2809", "a"),
        Err(NamingError::InvalidAddress)
    ));
    assert_eq!(
        root.to_url(":host:2809", "a/b c").unwrap(),
        "corbaname::host:2809#a/b%20c"
    );
    assert_eq!(root.to_url(":h", "").unwrap(), "corbaname::h");

    // corbaname: through the service.
    let resolved = root
        .object()
        .orb()
        .string_to_object(&format!("corbaname::127.0.0.1:{port}#t/b.kind"))
        .unwrap()
        .unwrap();
    assert!(resolved.is_equivalent(&obj));
    orb.destroy().unwrap();
}

#[test]
fn contexts_of_another_service_are_followed() {
    let (p1, p2) = (free_port(), free_port());
    let (orb1, orb2) = (server_orb(p1), server_orb(p2));
    let _s1 = NamingService::start(&orb1, MemoryStore).unwrap();
    let _s2 = NamingService::start(&orb2, MemoryStore).unwrap();
    let (_c1, root1) = client_root(p1);
    let (_c2, root2) = client_root(p2);
    let obj = root2.object().clone();
    let remote = root2.bind_new_context(&n("far")).unwrap();
    root1.bind_context(&n("link"), &remote).unwrap();
    root1.bind(&n("link/x.k"), &obj).unwrap();
    assert!(root2.resolve(&n("far/x.k")).unwrap().is_some());
    assert!(root1.resolve(&n("link/x.k")).unwrap().is_some());
    assert_eq!(
        not_found(root1.resolve(&n("link/y")).unwrap_err()),
        (NotFoundReason::MissingNode, "y".into())
    );
    root1.bind_new_context(&n("link/deeper")).unwrap();
    root1.unbind(&n("link/x.k")).unwrap();
    let (bl, _) = remote.list(10).unwrap();
    assert_eq!(names(&bl), ["deeper:c"]);
    orb1.destroy().unwrap();
    orb2.destroy().unwrap();
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rustorb-naming-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn bindings_and_references_survive_a_restart() {
    let dir = temp_dir("restart");
    let port = free_port();
    let (sub_ior, obj_ior) = {
        let orb = server_orb(port);
        let _service = NamingService::start(&orb, LogStore::new(&dir)).unwrap();
        let (client, root) = client_root(port);
        let sub = root.bind_new_context(&n("app")).unwrap();
        sub.bind(&n("Echo.obj"), root.object()).unwrap();
        root.bind(&n("gone"), root.object()).unwrap();
        root.unbind(&n("gone")).unwrap();
        let iors = (
            client.object_to_string(Some(sub.object())).unwrap(),
            client.object_to_string(Some(root.object())).unwrap(),
        );
        client.destroy().unwrap();
        orb.destroy().unwrap();
        iors
    };
    let orb = server_orb(port);
    let service = NamingService::start(&orb, LogStore::new(&dir)).unwrap();
    let (client, root) = client_root(port);
    let echo = root.resolve_str("app/Echo.obj").unwrap().unwrap();
    assert!(echo.is_equivalent(&client.string_to_object(&obj_ior).unwrap().unwrap()));
    assert!(matches!(
        root.resolve(&n("gone")),
        Err(NamingError::NotFound { .. })
    ));
    // The reference to the context from before the restart still works.
    let sub = NamingContext::unchecked_narrow(client.string_to_object(&sub_ior).unwrap().unwrap());
    let (bl, _) = sub.list(10).unwrap();
    assert_eq!(names(&bl), ["Echo.obj:o"]);
    assert_eq!(service.snapshot().contexts.len(), 2);
    client.destroy().unwrap();
    orb.destroy().unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_root_context_can_be_called_in_the_same_orb() {
    let port = free_port();
    let orb = server_orb(port);
    let service = NamingService::start(&orb, MemoryStore).unwrap();
    let root = service.root_context();
    root.bind_new_context(&n("local")).unwrap();
    root.bind(&n("local/self"), root.object()).unwrap();
    assert!(root.resolve_str("local/self").unwrap().is_some());
    // The root itself bound in a context is recognised as local.
    root.bind_context(&n("local/root"), &root).unwrap();
    assert!(root.resolve_str("local/root/local/self").unwrap().is_some());
    // Without the simple key, the root context is not found.
    orb.unregister_object_key(rustorb_naming::ROOT_KEY);
    let e = root.list(1).unwrap_err();
    assert!(
        e.as_system()
            .unwrap()
            .is(SystemExceptionKind::OBJECT_NOT_EXIST)
    );
    orb.destroy().unwrap();
}
