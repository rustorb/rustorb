//! Hand-written mapping of `InteropTest.idl`, shared by the client and
//! the server until the code generator exists: `Put` / `Get` for every IDL
//! type used, and the InteropTest types.

#![allow(dead_code)]

use rustorb_core::cdr::CdrEncode;
use rustorb_core::{
    CompletionStatus, InStream, ObjectRef, OutStream, SystemException, SystemExceptionKind,
};

pub type Sys<T> = Result<T, SystemException>;

/// Writes an IDL value to a request.
pub trait Put {
    fn put(&self, out: &mut OutStream<'_>) -> Sys<()>;
}

/// Reads an IDL value from a reply.
pub trait Get: Sized {
    fn get(input: &mut InStream<'_>) -> Sys<Self>;
}

macro_rules! primitive {
    ($($ty:ty),*) => {
        $(
            impl Put for $ty {
                fn put(&self, out: &mut OutStream<'_>) -> Sys<()> {
                    Ok(self.encode(out)?)
                }
            }
            impl Get for $ty {
                fn get(input: &mut InStream<'_>) -> Sys<Self> {
                    input.read::<$ty>()
                }
            }
        )*
    };
}

primitive!(bool, u8, i16, u16, i32, u32, i64, u64, f32, f64);

impl Put for () {
    fn put(&self, _: &mut OutStream<'_>) -> Sys<()> {
        Ok(())
    }
}

impl Get for () {
    fn get(_: &mut InStream<'_>) -> Sys<Self> {
        Ok(())
    }
}

impl Put for String {
    fn put(&self, out: &mut OutStream<'_>) -> Sys<()> {
        out.write_string(self)
    }
}

impl Put for &str {
    fn put(&self, out: &mut OutStream<'_>) -> Sys<()> {
        out.write_string(self)
    }
}

impl Get for String {
    fn get(input: &mut InStream<'_>) -> Sys<Self> {
        input.read_string()
    }
}

/// IDL `char`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Char(pub u8);

impl Put for Char {
    fn put(&self, out: &mut OutStream<'_>) -> Sys<()> {
        out.write_char(self.0);
        Ok(())
    }
}

impl Get for Char {
    fn get(input: &mut InStream<'_>) -> Sys<Self> {
        Ok(Char(input.read::<u8>()?))
    }
}

/// IDL `wchar`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WChar(pub u16);

impl Put for WChar {
    fn put(&self, out: &mut OutStream<'_>) -> Sys<()> {
        out.write_wchar(self.0)
    }
}

impl Get for WChar {
    fn get(input: &mut InStream<'_>) -> Sys<Self> {
        Ok(WChar(input.read_wchar()?))
    }
}

/// IDL `wstring`.
#[derive(Debug, Clone, PartialEq)]
pub struct WString(pub String);

impl Put for WString {
    fn put(&self, out: &mut OutStream<'_>) -> Sys<()> {
        out.write_wstring(&self.0)
    }
}

impl Get for WString {
    fn get(input: &mut InStream<'_>) -> Sys<Self> {
        Ok(WString(input.read_wstring()?))
    }
}

impl<T: Put> Put for Vec<T> {
    fn put(&self, out: &mut OutStream<'_>) -> Sys<()> {
        out.write_len(self.len())?;
        self.iter().try_for_each(|v| v.put(out))
    }
}

impl<T: Get> Get for Vec<T> {
    fn get(input: &mut InStream<'_>) -> Sys<Self> {
        let len = input.read::<u32>()? as usize;
        if len > input.remaining() {
            return Err(marshal());
        }
        (0..len).map(|_| T::get(input)).collect()
    }
}

impl<T: Put, const N: usize> Put for [T; N] {
    fn put(&self, out: &mut OutStream<'_>) -> Sys<()> {
        self.iter().try_for_each(|v| v.put(out))
    }
}

impl<T: Get + Default + Copy, const N: usize> Get for [T; N] {
    fn get(input: &mut InStream<'_>) -> Sys<Self> {
        let mut a = [T::default(); N];
        for v in &mut a {
            *v = T::get(input)?;
        }
        Ok(a)
    }
}

impl Put for Option<ObjectRef> {
    fn put(&self, out: &mut OutStream<'_>) -> Sys<()> {
        out.write_object(self.as_ref())
    }
}

impl Get for Option<ObjectRef> {
    fn get(input: &mut InStream<'_>) -> Sys<Self> {
        input.read_object()
    }
}

macro_rules! tuple {
    ($($name:ident),*) => {
        impl<$($name: Put),*> Put for ($($name,)*) {
            #[allow(non_snake_case)]
            fn put(&self, out: &mut OutStream<'_>) -> Sys<()> {
                let ($($name,)*) = self;
                $($name.put(out)?;)*
                Ok(())
            }
        }
        impl<$($name: Get),*> Get for ($($name,)*) {
            fn get(input: &mut InStream<'_>) -> Sys<Self> {
                Ok(($($name::get(input)?,)*))
            }
        }
    };
}

tuple!(A);
tuple!(A, B);
tuple!(A, B, C);

pub fn marshal() -> SystemException {
    SystemException::new(SystemExceptionKind::MARSHAL, 0, CompletionStatus::Yes)
}

// --- InteropTest types -----------------------------------------------------

pub const MAGIC: i32 = 0x5255_5354;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Color {
    Red,
    Green,
    Blue,
}

impl Put for Color {
    fn put(&self, out: &mut OutStream<'_>) -> Sys<()> {
        (*self as u32).put(out)
    }
}

impl Get for Color {
    fn get(input: &mut InStream<'_>) -> Sys<Self> {
        match u32::get(input)? {
            0 => Ok(Color::Red),
            1 => Ok(Color::Green),
            2 => Ok(Color::Blue),
            _ => Err(marshal()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Put for Point {
    fn put(&self, out: &mut OutStream<'_>) -> Sys<()> {
        (self.x, self.y).put(out)
    }
}

impl Get for Point {
    fn get(input: &mut InStream<'_>) -> Sys<Self> {
        let (x, y) = Get::get(input)?;
        Ok(Point { x, y })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub id: i32,
    pub name: String,
    pub hue: Color,
    pub pos: Point,
    pub values: Vec<i32>,
    pub active: bool,
}

impl Put for Record {
    fn put(&self, out: &mut OutStream<'_>) -> Sys<()> {
        self.id.put(out)?;
        self.name.put(out)?;
        self.hue.put(out)?;
        self.pos.put(out)?;
        self.values.put(out)?;
        self.active.put(out)
    }
}

impl Get for Record {
    fn get(input: &mut InStream<'_>) -> Sys<Self> {
        Ok(Record {
            id: Get::get(input)?,
            name: Get::get(input)?,
            hue: Get::get(input)?,
            pos: Get::get(input)?,
            values: Get::get(input)?,
            active: Get::get(input)?,
        })
    }
}

/// `union Value switch (long)`.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    L(i32),
    S(String),
    P(Point),
    /// Discriminator 4 or 5.
    Seq(i32, Vec<i32>),
    /// The default member, with its discriminator.
    Flag(i32, bool),
}

impl Put for Value {
    fn put(&self, out: &mut OutStream<'_>) -> Sys<()> {
        match self {
            Value::L(v) => (1i32, *v).put(out),
            Value::S(v) => (2i32, v.clone()).put(out),
            Value::P(v) => (3i32, *v).put(out),
            Value::Seq(d, v) => (*d, v.clone()).put(out),
            Value::Flag(d, v) => (*d, *v).put(out),
        }
    }
}

impl Get for Value {
    fn get(input: &mut InStream<'_>) -> Sys<Self> {
        Ok(match i32::get(input)? {
            1 => Value::L(Get::get(input)?),
            2 => Value::S(Get::get(input)?),
            3 => Value::P(Get::get(input)?),
            d @ (4 | 5) => Value::Seq(d, Get::get(input)?),
            d => Value::Flag(d, Get::get(input)?),
        })
    }
}

/// `union Shade switch (Color)`, with an implicit default for BLUE.
#[derive(Debug, Clone, PartialEq)]
pub enum Shade {
    Level(u8),
    Label(String),
    Blue,
}

impl Put for Shade {
    fn put(&self, out: &mut OutStream<'_>) -> Sys<()> {
        match self {
            Shade::Level(v) => (Color::Red, *v).put(out),
            Shade::Label(v) => (Color::Green, v.clone()).put(out),
            Shade::Blue => Color::Blue.put(out),
        }
    }
}

impl Get for Shade {
    fn get(input: &mut InStream<'_>) -> Sys<Self> {
        Ok(match Color::get(input)? {
            Color::Red => Shade::Level(Get::get(input)?),
            Color::Green => Shade::Label(Get::get(input)?),
            Color::Blue => Shade::Blue,
        })
    }
}

pub const OOPS_ID: &str = "IDL:rustorb.org/InteropTest/Oops:1.0";
pub const DIVIDE_BY_ZERO_ID: &str = "IDL:rustorb.org/InteropTest/DivideByZero:1.0";
pub const ECHO_ID: &str = "IDL:rustorb.org/InteropTest/Echo:1.0";
impl<T: Put> Put for &T {
    fn put(&self, out: &mut OutStream<'_>) -> Sys<()> {
        (*self).put(out)
    }
}
