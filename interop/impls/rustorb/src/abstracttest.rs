//! The abstract interface tests (`idl/AbstractTest.idl`): the Person
//! servant, the operations of Card, the AbstractEcho servant and the client
//! cases, the same as those of `impls/rustorbpy/abstracttest.py`. Shared by
//! the client and the server.

#![allow(dead_code)]

use rustorb_core::idl::{IdlType as _, Sys};
use rustorb_core::{Any, Orb, Poa};

use crate::idl::abstracts::abstract_test::{
    AbstractEcho, AbstractEchoServant, Card, CardState, Named, NamedPair, NamedSeq, NamedServant,
    Person, PersonPoa, PersonServant,
};

/// `AbstractTest::Person`.
pub struct PersonImpl {
    pub name: String,
    pub age: i32,
}

impl NamedServant for PersonImpl {
    fn name(&self) -> Sys<String> {
        Ok(self.name.clone())
    }
}

impl PersonServant for PersonImpl {
    fn age(&self) -> Sys<i32> {
        Ok(self.age)
    }
}

/// `AbstractTest::AbstractEcho`.
pub struct AbstractEchoImpl {
    pub orb: Orb,
    pub poa: Poa,
}

impl AbstractEchoServant for AbstractEchoImpl {
    fn echo_named(&self, n: Option<&Named>) -> Sys<Option<Named>> {
        Ok(n.cloned())
    }

    fn name_of(&self, n: Option<&Named>) -> Sys<String> {
        match n {
            Some(n) => n.name(),
            None => Err(rustorb_core::idl::bad_param()),
        }
    }

    fn is_object(&self, n: Option<&Named>) -> Sys<bool> {
        Ok(matches!(n, Some(Named::Object(_))))
    }

    fn echo_pair(&self, p: &NamedPair) -> Sys<NamedPair> {
        Ok(p.clone())
    }

    fn echo_seq(&self, s: &[Option<Named>]) -> Sys<NamedSeq> {
        Ok(s.to_vec())
    }

    fn make_person(&self, name: &str) -> Sys<Option<Person>> {
        let (_, p) = PersonPoa(PersonImpl {
            name: name.to_owned(),
            age: 42,
        })
        .activate(&self.poa)
        .map_err(|e| poa_error(&e))?;
        Ok(Some(p))
    }

    fn echo_any(&self, a: &Any) -> Sys<Any> {
        Ok(a.clone())
    }

    fn shutdown(&self) -> Sys<()> {
        self.orb.shutdown(false)
    }
}

fn poa_error(e: &rustorb_core::PoaError) -> rustorb_core::SystemException {
    eprintln!("activate: {e}");
    rustorb_core::SystemException::new(
        rustorb_core::SystemExceptionKind::INTERNAL,
        0,
        rustorb_core::CompletionStatus::No,
    )
}

// --- client ----------------------------------------------------------------------

type Case<'a> = (String, Box<dyn Fn() -> Result<(), String> + 'a>);

fn check(cond: bool, what: impl FnOnce() -> String) -> Result<(), String> {
    if cond { Ok(()) } else { Err(what()) }
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn card(title: &str) -> Named {
    Card::new(CardState {
        title: title.into(),
    })
    .into()
}

fn is_card(v: &Option<Named>, title: &str) -> Result<(), String> {
    let v = v.as_ref().ok_or("got null")?;
    let h = v.value().ok_or_else(|| format!("got {v:?}"))?;
    let c = Card::from_handle(h.clone()).ok_or_else(|| format!("got {h:?}"))?;
    let got = c.get().title;
    check(got == title, || format!("got {got:?}"))?;
    let name = v.name().map_err(err)?;
    check(name == title, || format!("name() is {name:?}"))
}

fn is_alice(v: &Option<Named>) -> Result<(), String> {
    let v = v.as_ref().ok_or("got null")?;
    check(v.object().is_some(), || format!("got {v:?}"))?;
    let name = v.name().map_err(err)?;
    check(name == "alice", || format!("wrong name {name:?}"))
}

fn same(a: &Option<Named>, b: &Option<Named>) -> bool {
    match (a, b) {
        (Some(Named::Value(a)), Some(Named::Value(b))) => a.ptr_eq(b),
        _ => false,
    }
}

fn add<'a>(cases: &mut Vec<Case<'a>>, name: &str, f: impl Fn() -> Result<(), String> + 'a) {
    cases.push((name.to_owned(), Box::new(f)));
}

/// The abstract interface client cases against the AbstractEcho object
/// `echo`; the Person the client sends is activated in `poa`, which must
/// be active.
pub fn cases<'a>(echo: &'a AbstractEcho, poa: &Poa) -> Result<Vec<Case<'a>>, String> {
    let (_, alice) = PersonPoa(PersonImpl {
        name: "alice".into(),
        age: 30,
    })
    .activate(poa)
    .map_err(err)?;
    let alice: Named = alice.into();
    let mut cases = Vec::new();
    {
        let alice = alice.clone();
        add(&mut cases, "echo_named[object]", move || {
            is_alice(&echo.echo_named(Some(&alice)).map_err(err)?)
        });
    }
    add(&mut cases, "echo_named[value]", move || {
        is_card(&echo.echo_named(Some(&card("ace"))).map_err(err)?, "ace")
    });
    add(&mut cases, "echo_named[null]", move || {
        let got = echo.echo_named(None).map_err(err)?;
        check(got.is_none(), || format!("got {got:?}"))
    });
    {
        let alice = alice.clone();
        add(&mut cases, "name_of[object]", move || {
            let got = echo.name_of(Some(&alice)).map_err(err)?;
            check(got == "alice", || format!("got {got:?}"))
        });
    }
    add(&mut cases, "name_of[value]", move || {
        let got = echo.name_of(Some(&card("ace"))).map_err(err)?;
        check(got == "ace", || format!("got {got:?}"))
    });
    {
        let alice = alice.clone();
        add(&mut cases, "is_object[object]", move || {
            check(echo.is_object(Some(&alice)).map_err(err)?, || {
                "not an object".into()
            })
        });
    }
    add(&mut cases, "is_object[value]", move || {
        check(!echo.is_object(Some(&card("ace"))).map_err(err)?, || {
            "an object".into()
        })
    });
    add(&mut cases, "echo_pair[shared]", move || {
        let c = card("king");
        let got = echo
            .echo_pair(&NamedPair {
                first: Some(c.clone()),
                second: Some(c),
            })
            .map_err(err)?;
        is_card(&got.first, "king")?;
        check(same(&got.first, &got.second), || "not one instance".into())
    });
    {
        let alice = alice.clone();
        add(&mut cases, "echo_pair[mixed]", move || {
            let got = echo
                .echo_pair(&NamedPair {
                    first: Some(alice.clone()),
                    second: Some(card("queen")),
                })
                .map_err(err)?;
            is_alice(&got.first)?;
            is_card(&got.second, "queen")
        });
    }
    {
        let alice = alice.clone();
        add(&mut cases, "echo_seq", move || {
            let c = card("jack");
            let got = echo
                .echo_seq(&[Some(c.clone()), Some(alice.clone()), None, Some(c)])
                .map_err(err)?;
            check(got.len() == 4, || format!("got {} items", got.len()))?;
            is_card(&got[0], "jack")?;
            is_alice(&got[1])?;
            check(got[2].is_none(), || "not null".into())?;
            check(same(&got[0], &got[3]), || "not one instance".into())
        });
    }
    add(&mut cases, "make_person", move || {
        let p = echo.make_person("bob").map_err(err)?.ok_or("got nil")?;
        let (name, age) = (p.name().map_err(err)?, p.age().map_err(err)?);
        check((name.as_str(), age) == ("bob", 42), || {
            format!("got {name:?} {age}")
        })
    });
    add(&mut cases, "echo_any[value]", move || {
        let a = Any::insert::<Named>(&Some(card("joker"))).map_err(err)?;
        let got = echo.echo_any(&a).map_err(err)?;
        check(got.typecode().equal(&Named::type_code()), || {
            format!("got {:?}", got.typecode())
        })?;
        is_card(&got.extract::<Named>().ok_or("not a Named")?, "joker")
    });
    add(&mut cases, "echo_any[object]", move || {
        let a = Any::insert::<Named>(&Some(alice.clone())).map_err(err)?;
        let got = echo.echo_any(&a).map_err(err)?;
        check(got.typecode().equal(&Named::type_code()), || {
            format!("got {:?}", got.typecode())
        })?;
        is_alice(&got.extract::<Named>().ok_or("not a Named")?)
    });
    Ok(cases)
}
