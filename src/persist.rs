//! Declaring a metric once.
//!
//! Every field poptop retains used to be written down four times: in the struct,
//! in the writer, in the reader, and in a `VERSION` bump because the two halves
//! of a positional format cannot be changed independently. Measured against the
//! two most recent additions, `clock_ceiling` was one `Option<f32>` and touched
//! seven files; `cmd` touched ten.
//!
//! atop reports on the order of two hundred metrics. At four edits each that is
//! not a scaling problem, it is a different project — so the declaration and the
//! wire format come from the same text now.
//!
//! [`persist!`] takes a struct exactly as it would be written by hand,
//! attributes and doc comments and all, and emits it together with a [`Codec`]
//! that walks its fields in order. A field that exists cannot be forgotten by
//! the writer, and the reader cannot disagree with the writer about what order
//! they are in, because neither is written by a person.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A value that can be written to the buffer and read back.
///
/// Deliberately not `serde`. The format is a cache poptop owns both ends of, the
/// string table below is specific to what it stores, and a derive that produced
/// a different layout than the one documented here would be harder to reason
/// about than sixty lines of explicit impls.
pub trait Codec: Sized {
    fn write(&self, out: &mut Out);
    /// `None` for a truncated or corrupt buffer. Every read is bounds-checked:
    /// this is a cache, and the worst honest outcome is starting empty.
    fn read(r: &mut In<'_>) -> Option<Self>;
}

/// Derive a struct's wire format from a list of its fields.
///
/// The declaration stays hand-written — the doc comments on those fields are
/// the documentation for what poptop measures, and they belong next to the
/// types they describe rather than inside a macro invocation.
///
/// What the macro guarantees instead is that the list cannot fall behind the
/// struct. `write` destructures `Self` exhaustively, so a field added to the
/// struct and forgotten here is a compile error naming the field, not a value
/// that silently stops being retained. Writer and reader are generated from
/// the one list, so they cannot disagree about the order either.
///
/// Fields are written in the order listed, so reordering the list is a format
/// change and needs [`crate::store::VERSION`]. What it can no longer be is a
/// change nobody noticed.
macro_rules! codec {
    ($($name:ident { $($field:ident),* $(,)? })*) => {$(
        impl $crate::persist::Codec for $name {
            fn write(&self, out: &mut $crate::persist::Out) {
                let Self { $($field),* } = self;
                $( $crate::persist::Codec::write($field, out); )*
            }
            fn read(r: &mut $crate::persist::In<'_>) -> Option<Self> {
                Some(Self {
                    $( $field: $crate::persist::Codec::read(r)?, )*
                })
            }
        }
    )*};
}
pub(crate) use codec;

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
    index: std::collections::HashMap<Arc<str>, u32>,
}

impl Out {
    fn raw(&mut self, b: &[u8]) {
        self.bytes.extend_from_slice(b);
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
        Codec::write(&id, self);
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
}

/// Fixed-width little-endian, for the types that have a natural one.
macro_rules! fixed {
    ($($ty:ty),*) => {$(
        impl Codec for $ty {
            fn write(&self, out: &mut Out) {
                out.raw(&self.to_le_bytes());
            }
            fn read(r: &mut In<'_>) -> Option<Self> {
                Some(<$ty>::from_le_bytes(
                    r.take(size_of::<$ty>())?.try_into().ok()?,
                ))
            }
        }
    )*};
}
fixed!(u8, u16, u32, u64, i32, i64, f32, f64);

impl Codec for bool {
    fn write(&self, out: &mut Out) {
        Codec::write(&u8::from(*self), out);
    }
    fn read(r: &mut In<'_>) -> Option<Self> {
        Some(u8::read(r)? != 0)
    }
}

impl Codec for usize {
    fn write(&self, out: &mut Out) {
        Codec::write(&(*self as u64), out);
    }
    fn read(r: &mut In<'_>) -> Option<Self> {
        Some(u64::read(r)? as usize)
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
    fn read(r: &mut In<'_>) -> Option<Self> {
        Some(char::from(u8::read(r)?))
    }
}

/// Tagged, always. `None` and `0` are different answers throughout this
/// codebase, and the file must not be the place they collapse.
///
/// The payload is written either way so the record's length does not depend on
/// its content — a positional format cannot skip a field it did not write.
impl<T: Codec + Default> Codec for Option<T> {
    fn write(&self, out: &mut Out) {
        Codec::write(&self.is_some(), out);
        match self {
            Some(v) => v.write(out),
            None => T::default().write(out),
        }
    }
    fn read(r: &mut In<'_>) -> Option<Self> {
        let present = bool::read(r)?;
        let v = T::read(r)?;
        Some(present.then_some(v))
    }
}

impl<T: Codec> Codec for Vec<T> {
    fn write(&self, out: &mut Out) {
        Codec::write(&(self.len() as u32), out);
        for v in self {
            v.write(out);
        }
    }
    fn read(r: &mut In<'_>) -> Option<Self> {
        let n = u32::read(r)? as usize;
        // Capacity is bounded before allocating: a corrupt length field must
        // not be able to ask for a gigabyte.
        let mut out = Vec::with_capacity(n.min(1 << 16));
        for _ in 0..n {
            out.push(T::read(r)?);
        }
        Some(out)
    }
}

impl<T: Codec, const N: usize> Codec for [T; N] {
    fn write(&self, out: &mut Out) {
        for v in self {
            v.write(out);
        }
    }
    fn read(r: &mut In<'_>) -> Option<Self> {
        let mut out = Vec::with_capacity(N);
        for _ in 0..N {
            out.push(T::read(r)?);
        }
        out.try_into().ok()
    }
}

impl Codec for Arc<str> {
    fn write(&self, out: &mut Out) {
        out.str(self);
    }
    fn read(r: &mut In<'_>) -> Option<Self> {
        let id = u32::read(r)? as usize;
        r.strings.get(id).cloned()
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
    fn read(r: &mut In<'_>) -> Option<Self> {
        let secs = Duration::from_secs(u64::read(r)?);
        let nanos = Duration::from_nanos(u64::from(u32::read(r)?));
        UNIX_EPOCH.checked_add(secs.checked_add(nanos)?)
    }
}

impl Codec for Duration {
    fn write(&self, out: &mut Out) {
        Codec::write(&self.as_secs(), out);
    }
    fn read(r: &mut In<'_>) -> Option<Self> {
        Some(Duration::from_secs(u64::read(r)?))
    }
}
