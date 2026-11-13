//! `PortableServer::POAManagerFactory` (CORBA 3): identities of POA
//! managers, `create_POAManager`, `list`, `find`, sharing a manager among
//! POAs, and the end of a manager with the last of its POAs.

use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use rustorb_core::{
    EndpointAddr, Exception, IdAssignment, InStream, ObjectRef, Orb, OrbOptions, PoaError,
    PoaManagerState, Policy, PolicyError, PolicyValue, Servant, ServerReply, SystemExceptionKind,
};

struct Echo;

impl Servant for Echo {
    fn repository_id(&self) -> &str {
        "IDL:test/Echo:1.0"
    }

    fn dispatch(
        &self,
        _operation: &str,
        input: &mut InStream<'_>,
        reply: &mut ServerReply,
    ) -> Result<(), rustorb_core::SystemException> {
        let v = input.read::<i32>()?;
        reply.output().write_i32(v);
        Ok(())
    }
}

fn server_orb() -> Orb {
    Orb::with_options(OrbOptions {
        endpoints: vec![EndpointAddr {
            host: "127.0.0.1".into(),
            port: 0,
        }],
        ..OrbOptions::default()
    })
    .unwrap()
}

fn echo(obj: &ObjectRef, v: i32) -> Result<i32, Exception> {
    let reply = obj
        .request("echo")
        .args(move |out| {
            out.write_i32(v);
            Ok(())
        })
        .invoke()?;
    Ok(reply.input().read::<i32>()?)
}

fn ids(orb: &Orb) -> Vec<String> {
    let root = orb.root_poa().unwrap();
    root.the_poa_manager_factory()
        .list()
        .iter()
        .map(|m| m.get_id().to_owned())
        .collect()
}

#[test]
fn identities_and_list() {
    let orb = server_orb();
    let root = orb.root_poa().unwrap();
    let factory = root.the_poa_manager_factory();
    assert_eq!(root.the_poa_manager().get_id(), "RootPOAManager");
    assert_eq!(ids(&orb), ["RootPOAManager"]);

    // create_POA without a manager makes one with a fresh identity.
    let a = root.create_poa("A", None, &[]).unwrap();
    let b = a.create_poa("B", None, &[]).unwrap();
    let ida = a.the_poa_manager().get_id().to_owned();
    let idb = b.the_poa_manager().get_id().to_owned();
    assert_ne!(ida, idb);
    assert_ne!(ida, "RootPOAManager");

    let x = factory.create_poa_manager("x", &[]).unwrap();
    assert_eq!(x.get_id(), "x");
    assert_eq!(x.state(), PoaManagerState::Holding);
    let empty = factory.create_poa_manager("", &[]).unwrap();
    assert_eq!(empty.get_id(), "");
    assert_eq!(ids(&orb), ["RootPOAManager", &ida, &idb, "x", ""]);

    // Identities are unique in the ORB, the implicit ones included.
    for id in ["x", "", "RootPOAManager", ida.as_str()] {
        assert_eq!(
            factory.create_poa_manager(id, &[]),
            Err(PoaError::ManagerAlreadyExists)
        );
    }

    assert_eq!(factory.find("x"), Some(x.clone()));
    assert_eq!(factory.find(&ida), Some(a.the_poa_manager()));
    assert_eq!(factory.find("RootPOAManager"), Some(root.the_poa_manager()));
    assert_eq!(factory.find("nope"), None);

    // The factory of any POA is the ORB's.
    assert_eq!(b.the_poa_manager_factory().find("x"), Some(x));
    orb.destroy().unwrap();
}

#[test]
fn implicit_identities_skip_those_in_use() {
    let orb = server_orb();
    let root = orb.root_poa().unwrap();
    let factory = root.the_poa_manager_factory();
    let first = root.create_poa("first", None, &[]).unwrap();
    let taken = first.the_poa_manager().get_id().to_owned();
    // Make the identities the next implicit managers would take.
    let prefix = taken.trim_end_matches(|c: char| c.is_ascii_digit());
    let n: u64 = taken[prefix.len()..].parse().unwrap();
    for i in 1..=3 {
        factory
            .create_poa_manager(&format!("{prefix}{}", n + i), &[])
            .unwrap();
    }
    let next = root.create_poa("next", None, &[]).unwrap();
    let id = next.the_poa_manager().get_id().to_owned();
    assert_eq!(ids(&orb).iter().filter(|i| **i == id).count(), 1);
    orb.destroy().unwrap();
}

#[test]
fn policies_are_refused() {
    let orb = server_orb();
    let factory = orb.root_poa().unwrap().the_poa_manager_factory();
    for p in [
        PolicyValue::Poa(Policy::IdAssignment(IdAssignment::User)),
        PolicyValue::SyncScope(1),
    ] {
        assert_eq!(
            factory.create_poa_manager("p", &[p]),
            Err(PoaError::Policy(PolicyError {
                reason: PolicyError::UNSUPPORTED_POLICY
            }))
        );
    }
    assert_eq!(factory.find("p"), None);
    orb.destroy().unwrap();
}

#[test]
fn a_shared_manager_switches_all_its_poas() {
    let orb = server_orb();
    let root = orb.root_poa().unwrap();
    root.the_poa_manager().activate().unwrap();
    let manager = root
        .the_poa_manager_factory()
        .create_poa_manager("shared", &[])
        .unwrap();
    let user = [Policy::IdAssignment(IdAssignment::User)];
    let p1 = root.create_poa("p1", Some(&manager), &user).unwrap();
    let p2 = root.create_poa("p2", Some(&manager), &user).unwrap();
    let p3 = p2.create_poa("p3", Some(&manager), &user).unwrap();
    let other = root.create_poa("other", None, &user).unwrap();
    other.the_poa_manager().activate().unwrap();
    let objs: Vec<_> = [&p1, &p2, &p3, &other]
        .iter()
        .map(|poa| {
            poa.activate_object_with_id(b"o", Arc::new(Echo)).unwrap();
            poa.id_to_reference(b"o").unwrap()
        })
        .collect();
    for poa in [&p1, &p2, &p3] {
        assert_eq!(poa.the_poa_manager(), manager);
    }

    // Holding: requests to every POA of the manager wait.
    let (tx, rx) = mpsc::channel();
    for (i, obj) in objs[..3].iter().enumerate() {
        let (tx, obj) = (tx.clone(), obj.clone());
        thread::spawn(move || tx.send(echo(&obj, i as i32).unwrap()).unwrap());
    }
    assert_eq!(echo(&objs[3], 7).unwrap(), 7);
    assert!(rx.recv_timeout(Duration::from_millis(300)).is_err());
    manager.activate().unwrap();
    let mut got: Vec<i32> = (0..3)
        .map(|_| rx.recv_timeout(Duration::from_secs(5)).unwrap())
        .collect();
    got.sort();
    assert_eq!(got, [0, 1, 2]);

    // Discarding reaches all of them, and only them.
    manager.discard_requests(true).unwrap();
    for obj in &objs[..3] {
        match echo(obj, 1) {
            Err(Exception::System(e)) => assert!(e.is(SystemExceptionKind::TRANSIENT)),
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(echo(&objs[3], 8).unwrap(), 8);
    manager.activate().unwrap();
    assert_eq!(echo(&objs[2], 9).unwrap(), 9);
    orb.destroy().unwrap();
}

#[test]
fn a_manager_ends_with_its_last_poa() {
    let orb = server_orb();
    let root = orb.root_poa().unwrap();
    let factory = root.the_poa_manager_factory();
    let x = factory.create_poa_manager("x", &[]).unwrap();
    let c = root.create_poa("C", Some(&x), &[]).unwrap();
    let d = c.create_poa("D", Some(&x), &[]).unwrap();
    let e = root.create_poa("E", Some(&x), &[]).unwrap();

    // Destroying C destroys D too; E still uses x.
    c.destroy(false, true).unwrap();
    assert_eq!(factory.find("x"), Some(x.clone()));
    drop(d);
    e.destroy(false, true).unwrap();
    assert_eq!(factory.find("x"), None);
    assert!(!ids(&orb).contains(&"x".to_owned()));

    // The destroyed manager takes no new POA; its identity is free again.
    match root.create_poa("F", Some(&x), &[]) {
        Err(PoaError::System(e)) => assert!(e.is(SystemExceptionKind::OBJECT_NOT_EXIST)),
        other => panic!("{other:?}"),
    }
    assert!(root.find_poa("F", false).is_err());
    let x2 = factory.create_poa_manager("x", &[]).unwrap();
    assert_ne!(x2, x);
    assert_eq!(x.get_id(), "x");

    // An implicit manager ends with its POA.
    let g = root.create_poa("G", None, &[]).unwrap();
    let gid = g.the_poa_manager().get_id().to_owned();
    assert!(factory.find(&gid).is_some());
    g.destroy(false, true).unwrap();
    assert_eq!(factory.find(&gid), None);

    // Deactivating does not end a manager, nor does having no POA yet.
    let w = factory.create_poa_manager("w", &[]).unwrap();
    w.deactivate(false, true).unwrap();
    assert_eq!(factory.find("w"), Some(w.clone()));
    let h = root.create_poa("H", Some(&w), &[]).unwrap();
    assert_eq!(h.the_poa_manager().state(), PoaManagerState::Inactive);
    orb.destroy().unwrap();
}
