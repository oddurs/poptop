//! Declaring a metric once, and reading a file that declared a different set.
//!
//! Every field poptop retains used to be written down three times — the struct,
//! the writer, the reader — and a fourth time as a `VERSION` bump, because two
//! halves of a positional format cannot be changed independently. Measured
//! against the two most recent additions, `clock_ceiling` was one
//! `Option<f32>` and touched seven files; `cmd` touched ten.
//!
//! The bump was the expensive part. A reader that refuses any version but its
//! own turns every upgrade into "your history is gone", which is defensible for
//! a ten-minute ring buffer and indefensible for the multi-day logs v2.2 adds.
//!
//! # How a file describes itself
//!
//! The file carries its own schema in the header: for each record type, the
//! fields it was written with, each with a name, a hash of its type, and enough
//! of a type description to skip it. Bodies stay positional, in the order the
//! *file's* schema gives.
//!
//! Reading is then a merge rather than a match. For each field the file
//! declares, a reader either recognises the name and the type hash, and reads
//! it into that field — or does not, and skips exactly that field's bytes. A
//! field the reader expects and the file does not have keeps its blank value,
//! which for anything optional is `None`. So a newer poptop reads an older
//! file, an older poptop reads a newer one, and neither invents a number.
//!
//! **Why a schema block and not a tag on every value.** A tag and a length per
//! field is the obvious self-describing format and it costs a few bytes per
//! field — but the thing being stored is 600 samples of 400 processes, so a
//! per-field cost is paid 2.6 million times. Measured, that is roughly a fifth
//! of the file. Declaring the shape once per file costs about a kilobyte total
//! and nothing per sample.
//!
//! [`codec!`] generates the writer, the schema-driven reader, and the type
//! description from one list of fields placed beside the struct it describes.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// FNV-1a, as a `const fn` so a type's hash is computed at compile time.
const fn fnv(mut h: u64, bytes: &[u8]) -> u64 {
    let mut i = 0;
    while i < bytes.len() {
        h ^= bytes[i] as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
        i += 1;
    }
    h
}

const FNV_SEED: u64 = 0xcbf2_9ce4_8422_2325;

/// What a field looks like on the wire, in enough detail to skip one this
/// reader has never heard of.
///
/// Only ever used for skipping. Matching a field the reader *does* know is done
/// on [`Typed::HASH`], which the writer stores in the schema, so no runtime
/// walk of this structure has to agree with a compile-time constant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ty {
    U8,
    U16,
    U32,
    U64,
    I32,
    I64,
    F32,
    F64,
    Bool,
    Char,
    Usize,
    /// An index into the file's string table.
    Str,
    Time,
    Dur,
    Opt(Box<Ty>),
    List(Box<Ty>),
    Arr(Box<Ty>, u32),
    /// Another record, by name, whose fields the file's schema describes.
    Rec(Box<str>),
}

impl Ty {
    fn write(&self, out: &mut Out) {
        let tag: u8 = match self {
            Ty::U8 => 0,
            Ty::U16 => 1,
            Ty::U32 => 2,
            Ty::U64 => 3,
            Ty::I32 => 4,
            Ty::I64 => 5,
            Ty::F32 => 6,
            Ty::F64 => 7,
            Ty::Bool => 8,
            Ty::Char => 9,
            Ty::Usize => 10,
            Ty::Str => 11,
            Ty::Time => 12,
            Ty::Dur => 13,
            Ty::Opt(_) => 14,
            Ty::List(_) => 15,
            Ty::Arr(..) => 16,
            Ty::Rec(_) => 17,
        };
        Codec::write(&tag, out);
        match self {
            Ty::Opt(inner) | Ty::List(inner) => inner.write(out),
            Ty::Arr(inner, n) => {
                inner.write(out);
                Codec::write(n, out);
            }
            Ty::Rec(name) => out.raw_str(name),
            _ => {}
        }
    }

    /// Bounded so a corrupt header cannot recurse without end.
    fn read(r: &mut In<'_>, depth: u32) -> Option<Ty> {
        if depth > 16 {
            return None;
        }
        Some(match u8::read_raw(r)? {
            0 => Ty::U8,
            1 => Ty::U16,
            2 => Ty::U32,
            3 => Ty::U64,
            4 => Ty::I32,
            5 => Ty::I64,
            6 => Ty::F32,
            7 => Ty::F64,
            8 => Ty::Bool,
            9 => Ty::Char,
            10 => Ty::Usize,
            11 => Ty::Str,
            12 => Ty::Time,
            13 => Ty::Dur,
            14 => Ty::Opt(Box::new(Ty::read(r, depth + 1)?)),
            15 => Ty::List(Box::new(Ty::read(r, depth + 1)?)),
            16 => {
                let inner = Box::new(Ty::read(r, depth + 1)?);
                Ty::Arr(inner, u32::read_raw(r)?)
            }
            17 => Ty::Rec(r.raw_str()?),
            _ => return None,
        })
    }
}

/// A type's identity, as a compile-time constant.
pub trait Typed {
    /// Hashes the *structure* of the type, so a field whose type changed no
    /// longer matches by name alone and is skipped rather than misread.
    const HASH: u64;
    /// The same thing at runtime, written into the file so a reader that has
    /// never heard of this field can still skip it.
    fn ty() -> Ty;
}

/// One field, as the file declares it.
#[derive(Clone, Debug)]
pub struct Field {
    pub name: Box<str>,
    pub hash: u64,
    pub ty: Ty,
}

/// Every record type the file declares, by name.
#[derive(Default, Debug)]
pub struct Registry {
    records: HashMap<Box<str>, Vec<Field>>,
    /// Whether the file's shape is exactly this build's, decided once when the
    /// header is parsed. Every ordinary run takes the fast path; the merge is
    /// for the run right after an upgrade.
    pub exact: bool,
}

impl Registry {
    pub fn fields(&self, name: &str) -> Option<&[Field]> {
        self.records.get(name).map(Vec::as_slice)
    }
}

/// Consume one field of the given type without interpreting it.
///
/// This is what lets an older poptop read a file written by a newer one. Every
/// branch is bounds-checked and every length is the reader's own, so a corrupt
/// header cannot make this run away.
pub fn skip(ty: &Ty, reg: &Registry, r: &mut In<'_>) -> Option<()> {
    match ty {
        Ty::U8 | Ty::Bool | Ty::Char => r.take(1).map(|_| ()),
        Ty::U16 => r.take(2).map(|_| ()),
        Ty::U32 | Ty::I32 | Ty::F32 | Ty::Str => r.take(4).map(|_| ()),
        Ty::U64 | Ty::I64 | Ty::F64 | Ty::Usize | Ty::Dur => r.take(8).map(|_| ()),
        Ty::Time => r.take(12).map(|_| ()),
        Ty::Opt(inner) => {
            r.take(1)?;
            skip(inner, reg, r)
        }
        Ty::List(inner) => {
            let n = u32::read_raw(r)?;
            for _ in 0..n {
                skip(inner, reg, r)?;
            }
            Some(())
        }
        Ty::Arr(inner, n) => {
            for _ in 0..*n {
                skip(inner, reg, r)?;
            }
            Some(())
        }
        Ty::Rec(name) => {
            let fields = reg.fields(name)?;
            for f in fields {
                skip(&f.ty, reg, r)?;
            }
            Some(())
        }
    }
}

/// Derive a struct's wire format, its type description, and a reader that can
/// merge a file written with a different set of fields.
///
/// The declaration stays hand-written — the doc comments on those fields are
/// the documentation for what poptop measures, and they belong next to the
/// types they describe rather than inside a macro invocation.
///
/// What the macro guarantees is that the list cannot fall behind the struct.
/// `write` destructures `Self` exhaustively, so a field added to the struct and
/// forgotten here is a compile error naming the field, and the type written
/// beside each name has to be the field's real type or the generated reader
/// will not compile either.
///
/// Field *order* here is the order bodies are written in. Reordering is safe:
/// the file says what its own order was.
#[macro_export]
macro_rules! codec {
    ($($name:ident { $($field:ident : $ty:ty),* $(,)? })*) => {$(
        impl $crate::persist::Typed for $name {
            const HASH: u64 = $crate::persist::type_hash(stringify!($name).as_bytes());
            fn ty() -> $crate::persist::Ty {
                $crate::persist::Ty::Rec(stringify!($name).into())
            }
        }

        impl $crate::persist::Record for $name {
            const NAME: &'static str = stringify!($name);
            fn schema() -> Vec<$crate::persist::Field> {
                vec![$( $crate::persist::Field {
                    name: stringify!($field).into(),
                    hash: <$ty as $crate::persist::Typed>::HASH,
                    ty: <$ty as $crate::persist::Typed>::ty(),
                } ),*]
            }
        }

        impl $crate::persist::Codec for $name {
            fn write(&self, out: &mut $crate::persist::Out) {
                let Self { $($field),* } = self;
                $( $crate::persist::Codec::write($field, out); )*
            }

            fn read_exact(
                reg: &$crate::persist::Registry,
                r: &mut $crate::persist::In<'_>,
            ) -> Option<Self> {
                Some(Self {
                    $( $field: <$ty as $crate::persist::Codec>::read_exact(reg, r)?, )*
                })
            }

            fn read(
                ty: &$crate::persist::Ty,
                reg: &$crate::persist::Registry,
                r: &mut $crate::persist::In<'_>,
            ) -> Option<Self> {
                let $crate::persist::Ty::Rec(rec) = ty else {
                    return None;
                };
                let fields = reg.fields(rec)?;
                // Start from blank, so a field this reader expects and the file
                // does not have keeps whatever "unknown" means for it — `None`
                // for anything optional — rather than a value from nowhere.
                let mut out = <Self as Default>::default();
                'field: for f in fields {
                    $(
                        if &*f.name == stringify!($field) {
                            if f.hash == <$ty as $crate::persist::Typed>::HASH {
                                out.$field =
                                    <$ty as $crate::persist::Codec>::read(&f.ty, reg, r)?;
                            } else {
                                // Same name, different type. Skipping is the
                                // only honest option: reading it as the current
                                // type would put a number in the column that
                                // was never measured.
                                $crate::persist::skip(&f.ty, reg, r)?;
                            }
                            continue 'field;
                        }
                    )*
                    $crate::persist::skip(&f.ty, reg, r)?;
                }
                Some(out)
            }
        }
    )*};
}
pub(crate) use codec;

/// List every record type whose schema the file should carry.
///
/// A record reachable from `Sample` but missing here has no schema in the file
/// and cannot be read back. `every_reachable_record_has_a_schema` asserts the
/// list is complete rather than trusting it.
#[macro_export]
macro_rules! records {
    ($($name:ident),* $(,)?) => {
        /// Every record type, with the fields this build writes.
        pub fn schemas() -> Vec<(&'static str, Vec<$crate::persist::Field>)> {
            vec![$((
                <$name as $crate::persist::Record>::NAME,
                <$name as $crate::persist::Record>::schema(),
            )),*]
        }
    };
}
pub(crate) use records;

/// A struct that is written as a record: it has a name and a field list.
pub trait Record: Sized {
    const NAME: &'static str;
    fn schema() -> Vec<Field>;
}

/// Hash a record type's name. Public because [`codec!`] expands into other
/// modules.
pub const fn type_hash(name: &[u8]) -> u64 {
    fnv(fnv(FNV_SEED, b"rec"), name)
}

/// A value that can be written to the buffer and read back.
///
/// Deliberately not `serde`. The format is a cache poptop owns both ends of,
/// the string table is specific to what it stores, and the schema merge below
/// is the whole point of the file — none of which a derive would give.
pub trait Codec: Sized + Typed {
    fn write(&self, out: &mut Out);

    /// Read assuming the file's layout for this type is exactly this build's.
    ///
    /// The fast path, taken when the file was written by a build with the same
    /// schema — which is every ordinary run. Skips the per-field name and hash
    /// comparison, and for records skips constructing a blank to merge into.
    fn read_exact(reg: &Registry, r: &mut In<'_>) -> Option<Self>;

    /// Read a field the file described, merging it into this build's shape.
    fn read(ty: &Ty, reg: &Registry, r: &mut In<'_>) -> Option<Self>;
}

/// The buffer being written, plus the string table.
#[derive(Default)]
pub struct Out {
    pub bytes: Vec<u8>,
    /// What the string table will occupy, tracked as it grows so the trim loop
    /// can bound the *file* rather than the bodies. Counting only the bodies
    /// made the "ceiling on the file" untrue by the size of the table, which is
    /// exactly the part that varies between machines.
    pub table_bytes: usize,
    /// Names and users repeat across every process in every retained sample —
    /// the same reason they are `Arc<str>` in memory. Written once and
    /// referenced by index, or the file would be mostly repeated strings.
    pub strings: Vec<Arc<str>>,
    index: HashMap<Arc<str>, u32>,
}

impl Out {
    fn raw(&mut self, b: &[u8]) {
        self.bytes.extend_from_slice(b);
    }

    /// A string written inline rather than through the table. Used by the
    /// schema, which has to be readable before the table exists.
    pub fn raw_str(&mut self, s: &str) {
        self.raw(&(s.len() as u32).to_le_bytes());
        self.raw(s.as_bytes());
    }

    /// Write the schema of every record type, so the file describes itself.
    pub fn schema_block(&mut self, records: &[(&'static str, Vec<Field>)]) {
        self.raw(&(records.len() as u32).to_le_bytes());
        for (name, fields) in records {
            self.raw_str(name);
            self.raw(&(fields.len() as u32).to_le_bytes());
            for f in fields {
                self.raw_str(&f.name);
                self.raw(&f.hash.to_le_bytes());
                f.ty.write(self);
            }
        }
    }

    /// Intern a string and write its index.
    fn str(&mut self, s: &Arc<str>) {
        let id = match self.index.get(s) {
            Some(&id) => id,
            None => {
                let id = self.strings.len() as u32;
                self.table_bytes += 4 + s.len();
                self.strings.push(s.clone());
                self.index.insert(s.clone(), id);
                id
            }
        };
        self.raw(&id.to_le_bytes());
    }
}

/// A bounds-checked reader.
pub struct In<'a> {
    pub bytes: &'a [u8],
    pub at: usize,
    pub strings: Vec<Arc<str>>,
}

impl<'a> In<'a> {
    pub fn new(bytes: &'a [u8], at: usize, strings: Vec<Arc<str>>) -> Self {
        Self { bytes, at, strings }
    }

    pub fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.at.checked_add(n)?;
        let out = self.bytes.get(self.at..end)?;
        self.at = end;
        Some(out)
    }

    /// An inline string, as the schema writes them.
    pub fn raw_str(&mut self) -> Option<Box<str>> {
        let len = u32::read_raw(self)? as usize;
        Some(std::str::from_utf8(self.take(len)?).ok()?.into())
    }

    /// Parse the schema block into a registry, and decide whether it matches
    /// this build exactly.
    pub fn schema_block(&mut self, mine: &[(&'static str, Vec<Field>)]) -> Option<Registry> {
        let n = u32::read_raw(self)?;
        // A count from a corrupt header must not be able to ask for a million
        // record types; the real number is ten.
        let mut map: HashMap<Box<str>, Vec<Field>> = HashMap::with_capacity(n.min(64) as usize);
        for _ in 0..n {
            let name = self.raw_str()?;
            let n_fields = u32::read_raw(self)?;
            let mut fields = Vec::with_capacity(n_fields.min(256) as usize);
            for _ in 0..n_fields {
                fields.push(Field {
                    name: self.raw_str()?,
                    hash: u64::read_raw(self)?,
                    ty: Ty::read(self, 0)?,
                });
            }
            map.insert(name, fields);
        }

        // Every record this build knows, declared by the file with the same
        // fields in the same order. Anything less takes the merge path.
        let exact = mine.iter().all(|(name, want)| {
            map.get(*name).is_some_and(|got| {
                got.len() == want.len()
                    && got
                        .iter()
                        .zip(want)
                        .all(|(a, b)| a.name == b.name && a.hash == b.hash)
            })
        });
        Some(Registry {
            records: map,
            exact,
        })
    }
}

/// Reading without a schema, for the header — which has to be parsed before
/// there is a registry to consult.
pub trait Raw: Sized {
    fn read_raw(r: &mut In<'_>) -> Option<Self>;
}

/// Fixed-width little-endian, for the types that have a natural one.
macro_rules! fixed {
    ($($ty:ty => $name:literal, $variant:expr);* $(;)?) => {$(
        impl Typed for $ty {
            const HASH: u64 = fnv(FNV_SEED, $name);
            fn ty() -> Ty { $variant }
        }
        impl Raw for $ty {
            fn read_raw(r: &mut In<'_>) -> Option<Self> {
                Some(<$ty>::from_le_bytes(r.take(size_of::<$ty>())?.try_into().ok()?))
            }
        }
        impl Codec for $ty {
            fn write(&self, out: &mut Out) {
                out.raw(&self.to_le_bytes());
            }
            fn read_exact(_: &Registry, r: &mut In<'_>) -> Option<Self> {
                Self::read_raw(r)
            }
            fn read(_: &Ty, _: &Registry, r: &mut In<'_>) -> Option<Self> {
                Self::read_raw(r)
            }
        }
    )*};
}

fixed!(
    u8 => b"u8", Ty::U8;
    u16 => b"u16", Ty::U16;
    u32 => b"u32", Ty::U32;
    u64 => b"u64", Ty::U64;
    i32 => b"i32", Ty::I32;
    i64 => b"i64", Ty::I64;
    f32 => b"f32", Ty::F32;
    f64 => b"f64", Ty::F64;
);

impl Typed for bool {
    const HASH: u64 = fnv(FNV_SEED, b"bool");
    fn ty() -> Ty {
        Ty::Bool
    }
}
impl Codec for bool {
    fn write(&self, out: &mut Out) {
        Codec::write(&u8::from(*self), out);
    }
    fn read_exact(reg: &Registry, r: &mut In<'_>) -> Option<Self> {
        Some(u8::read_exact(reg, r)? != 0)
    }
    fn read(_: &Ty, reg: &Registry, r: &mut In<'_>) -> Option<Self> {
        Self::read_exact(reg, r)
    }
}

impl Typed for usize {
    const HASH: u64 = fnv(FNV_SEED, b"usize");
    fn ty() -> Ty {
        Ty::Usize
    }
}
impl Codec for usize {
    fn write(&self, out: &mut Out) {
        Codec::write(&(*self as u64), out);
    }
    fn read_exact(reg: &Registry, r: &mut In<'_>) -> Option<Self> {
        Some(u64::read_exact(reg, r)? as usize)
    }
    fn read(_: &Ty, reg: &Registry, r: &mut In<'_>) -> Option<Self> {
        Self::read_exact(reg, r)
    }
}

impl Typed for char {
    const HASH: u64 = fnv(FNV_SEED, b"char");
    fn ty() -> Ty {
        Ty::Char
    }
}
/// One byte, not four.
///
/// The only `char` retained is [`crate::sample::ProcSample::state`], a single
/// letter from the fixed set both backends agree on. Writing it as a full
/// `u32` scalar cost three bytes per process per sample — 0.7 MB of a 16.5 MB
/// store at 400 processes, measured, which is 4% of the retention that is the
/// whole reason this file exists.
///
/// Anything outside ASCII writes as `?`, which is already what an unrecognised
/// state renders as: `status_char` maps every status it does not know to `?`,
/// so this loses nothing that was not already lost.
impl Codec for char {
    fn write(&self, out: &mut Out) {
        let b = u32::from(*self);
        Codec::write(&if b < 128 { b as u8 } else { b'?' }, out);
    }
    fn read_exact(reg: &Registry, r: &mut In<'_>) -> Option<Self> {
        Some(char::from(u8::read_exact(reg, r)?))
    }
    fn read(_: &Ty, reg: &Registry, r: &mut In<'_>) -> Option<Self> {
        Self::read_exact(reg, r)
    }
}

impl<T: Typed> Typed for Option<T> {
    const HASH: u64 = fnv(fnv(FNV_SEED, b"opt"), &T::HASH.to_le_bytes());
    fn ty() -> Ty {
        Ty::Opt(Box::new(T::ty()))
    }
}
/// Tagged, always. `None` and `0` are different answers throughout this
/// codebase, and the file must not be the place they collapse.
///
/// The payload is written either way so the record's length does not depend on
/// its content — which is also what lets [`skip`] step over one without
/// reading it.
impl<T: Codec + Default> Codec for Option<T> {
    fn write(&self, out: &mut Out) {
        Codec::write(&self.is_some(), out);
        match self {
            Some(v) => v.write(out),
            None => T::default().write(out),
        }
    }
    fn read_exact(reg: &Registry, r: &mut In<'_>) -> Option<Self> {
        let present = bool::read_exact(reg, r)?;
        let v = T::read_exact(reg, r)?;
        Some(present.then_some(v))
    }
    fn read(ty: &Ty, reg: &Registry, r: &mut In<'_>) -> Option<Self> {
        let Ty::Opt(inner) = ty else { return None };
        let present = bool::read_exact(reg, r)?;
        let v = T::read(inner, reg, r)?;
        Some(present.then_some(v))
    }
}

impl<T: Typed> Typed for Vec<T> {
    const HASH: u64 = fnv(fnv(FNV_SEED, b"list"), &T::HASH.to_le_bytes());
    fn ty() -> Ty {
        Ty::List(Box::new(T::ty()))
    }
}
impl<T: Codec> Codec for Vec<T> {
    fn write(&self, out: &mut Out) {
        Codec::write(&(self.len() as u32), out);
        for v in self {
            v.write(out);
        }
    }
    fn read_exact(reg: &Registry, r: &mut In<'_>) -> Option<Self> {
        let n = u32::read_exact(reg, r)? as usize;
        // Capacity is bounded before allocating: a corrupt length field must
        // not be able to ask for a gigabyte.
        let mut out = Vec::with_capacity(n.min(1 << 16));
        for _ in 0..n {
            out.push(T::read_exact(reg, r)?);
        }
        Some(out)
    }
    fn read(ty: &Ty, reg: &Registry, r: &mut In<'_>) -> Option<Self> {
        let Ty::List(inner) = ty else { return None };
        let n = u32::read_raw(r)? as usize;
        let mut out = Vec::with_capacity(n.min(1 << 16));
        for _ in 0..n {
            out.push(T::read(inner, reg, r)?);
        }
        Some(out)
    }
}

impl<T: Typed, const N: usize> Typed for [T; N] {
    const HASH: u64 = fnv(
        fnv(fnv(FNV_SEED, b"arr"), &T::HASH.to_le_bytes()),
        &(N as u64).to_le_bytes(),
    );
    fn ty() -> Ty {
        Ty::Arr(Box::new(T::ty()), N as u32)
    }
}
impl<T: Codec, const N: usize> Codec for [T; N] {
    fn write(&self, out: &mut Out) {
        for v in self {
            v.write(out);
        }
    }
    fn read_exact(reg: &Registry, r: &mut In<'_>) -> Option<Self> {
        let mut out = Vec::with_capacity(N);
        for _ in 0..N {
            out.push(T::read_exact(reg, r)?);
        }
        out.try_into().ok()
    }
    fn read(ty: &Ty, reg: &Registry, r: &mut In<'_>) -> Option<Self> {
        let Ty::Arr(inner, n) = ty else { return None };
        // A file whose array is a different length than this build's is a
        // different type, and the hash comparison will already have sent it to
        // `skip`. Reaching here with a mismatch means a corrupt header.
        if *n as usize != N {
            return None;
        }
        let mut out = Vec::with_capacity(N);
        for _ in 0..N {
            out.push(T::read(inner, reg, r)?);
        }
        out.try_into().ok()
    }
}

impl Typed for Arc<str> {
    const HASH: u64 = fnv(FNV_SEED, b"str");
    fn ty() -> Ty {
        Ty::Str
    }
}
impl Codec for Arc<str> {
    fn write(&self, out: &mut Out) {
        out.str(self);
    }
    fn read_exact(reg: &Registry, r: &mut In<'_>) -> Option<Self> {
        let id = u32::read_exact(reg, r)? as usize;
        r.strings.get(id).cloned()
    }
    fn read(_: &Ty, reg: &Registry, r: &mut In<'_>) -> Option<Self> {
        Self::read_exact(reg, r)
    }
}

impl Typed for SystemTime {
    const HASH: u64 = fnv(FNV_SEED, b"time");
    fn ty() -> Ty {
        Ty::Time
    }
}
/// Seconds and nanoseconds since the epoch.
///
/// A time before the epoch writes as zero rather than failing: the only
/// producer is `SystemTime::now`, and a clock set to 1969 is not a reason to
/// refuse the whole cache.
impl Codec for SystemTime {
    fn write(&self, out: &mut Out) {
        let d = self.duration_since(UNIX_EPOCH).unwrap_or_default();
        Codec::write(&d.as_secs(), out);
        Codec::write(&d.subsec_nanos(), out);
    }
    /// Checked the whole way, because `at` is the first field of a `Sample` and
    /// a panic here is a crash at startup rather than an empty buffer.
    /// `Duration::new` panics when the nanosecond carry overflows the seconds
    /// counter, and adding a large enough `Duration` to `UNIX_EPOCH` panics
    /// too — both reachable from a file with intact magic and a corrupt body.
    fn read_exact(reg: &Registry, r: &mut In<'_>) -> Option<Self> {
        let secs = Duration::from_secs(u64::read_exact(reg, r)?);
        let nanos = Duration::from_nanos(u64::from(u32::read_exact(reg, r)?));
        UNIX_EPOCH.checked_add(secs.checked_add(nanos)?)
    }
    fn read(_: &Ty, reg: &Registry, r: &mut In<'_>) -> Option<Self> {
        Self::read_exact(reg, r)
    }
}

impl Typed for Duration {
    const HASH: u64 = fnv(FNV_SEED, b"dur");
    fn ty() -> Ty {
        Ty::Dur
    }
}
impl Codec for Duration {
    fn write(&self, out: &mut Out) {
        Codec::write(&self.as_secs(), out);
    }
    fn read_exact(reg: &Registry, r: &mut In<'_>) -> Option<Self> {
        Some(Duration::from_secs(u64::read_exact(reg, r)?))
    }
    fn read(_: &Ty, reg: &Registry, r: &mut In<'_>) -> Option<Self> {
        Self::read_exact(reg, r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A record written and read on its own, so the merge can be tested against
    /// two builds of the same struct rather than by editing bytes by hand.
    fn to_bytes<T: Codec + Record>(v: &T) -> Vec<u8> {
        let mut head = Out::default();
        head.schema_block(&[(T::NAME, T::schema())]);
        let mut body = Out::default();
        v.write(&mut body);
        let mut f = head.bytes;
        f.extend((body.strings.len() as u32).to_le_bytes());
        for s in &body.strings {
            f.extend((s.len() as u32).to_le_bytes());
            f.extend(s.as_bytes());
        }
        f.extend(body.bytes);
        f
    }

    fn from_bytes<T: Codec + Record>(bytes: &[u8]) -> Option<T> {
        let mut r = In::new(bytes, 0, Vec::new());
        let reg = r.schema_block(&[(T::NAME, T::schema())])?;
        let n = u32::read_raw(&mut r)?;
        let mut strings = Vec::new();
        for _ in 0..n {
            let len = u32::read_raw(&mut r)? as usize;
            strings.push(Arc::from(std::str::from_utf8(r.take(len)?).ok()?));
        }
        r.strings = strings;
        if reg.exact {
            T::read_exact(&reg, &mut r)
        } else {
            T::read(&Ty::Rec(T::NAME.into()), &reg, &mut r)
        }
    }

    // Two builds of one record. Same record name, because that is the
    // situation: a user upgrades and the file on disk was written by the other
    // build of the same thing.
    mod before {
        use std::sync::Arc;
        #[derive(Debug, Default, PartialEq)]
        pub struct Proc {
            pub pid: i32,
            pub name: Arc<str>,
            pub rss: u64,
        }
        crate::persist::codec! { Proc { pid: i32, name: Arc<str>, rss: u64 } }
    }

    /// The next release: a metric added in the middle, one dropped, and the
    /// order changed — every kind of drift at once.
    mod after {
        use std::sync::Arc;
        #[derive(Debug, Default, PartialEq)]
        pub struct Proc {
            pub name: Arc<str>,
            pub threads: Option<u32>,
            pub pid: i32,
        }
        crate::persist::codec! { Proc { name: Arc<str>, threads: Option<u32>, pid: i32 } }
    }

    #[test]
    fn a_newer_poptop_reads_an_older_file_without_inventing_the_field_it_added() {
        let file = to_bytes(&before::Proc {
            pid: 4021,
            name: Arc::from("postgres"),
            rss: 900 << 20,
        });
        let got: after::Proc = from_bytes(&file).expect("the older file did not decode");
        assert_eq!(got.pid, 4021, "a field that survived the reorder was lost");
        assert_eq!(&*got.name, "postgres", "an interned string was lost");
        assert_eq!(
            got.threads, None,
            "a field the file never had came back with a value in it"
        );
    }

    #[test]
    fn an_older_poptop_reads_a_newer_file_by_skipping_what_it_does_not_know() {
        let file = to_bytes(&after::Proc {
            name: Arc::from("postgres"),
            threads: Some(17),
            pid: 4021,
        });
        let got: before::Proc = from_bytes(&file).expect("the newer file did not decode");
        assert_eq!(got.pid, 4021, "the field after the unknown one was misread");
        assert_eq!(&*got.name, "postgres", "the field before it was misread");
        assert_eq!(got.rss, 0, "a field the newer build dropped was invented");
    }
}
