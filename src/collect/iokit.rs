//! The IO registry, read the way `ioreg` reads it.
//!
//! macOS publishes the hardware's own counters here to any user: a disk's
//! bytes, operations and service time under its storage driver, the GPU's
//! utilisation under its accelerator, the battery's charge and current under
//! the smart battery. sysinfo reaches the first of those for bytes alone, and
//! the other two not at all, so poptop reads the registry itself.
//!
//! Declared by hand like the rest of this backend's FFI. Every object that
//! comes back owned is wrapped so it is released exactly once; every value
//! borrowed out of a dictionary is copied into a Rust value before the
//! dictionary can go.

use std::ffi::{CStr, CString, c_char, c_void};

type CFTypeRef = *const c_void;
type CFStringRef = *const c_void;
type CFDictionaryRef = *const c_void;
type IoObject = u32;

const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
const K_CF_NUMBER_SINT64: i32 = 4;
/// `kIOMainPortDefault`, which is `MACH_PORT_NULL`.
const MAIN_PORT: u32 = 0;
const KERN_SUCCESS: i32 = 0;

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOServiceMatching(name: *const c_char) -> CFDictionaryRef;
    fn IOServiceGetMatchingServices(
        main_port: u32,
        matching: CFDictionaryRef,
        existing: *mut IoObject,
    ) -> i32;
    fn IOIteratorNext(iterator: IoObject) -> IoObject;
    fn IOObjectRelease(object: IoObject) -> i32;
    fn IORegistryEntryCreateCFProperty(
        entry: IoObject,
        key: CFStringRef,
        allocator: *const c_void,
        options: u32,
    ) -> CFTypeRef;
    fn IORegistryEntryGetChildEntry(
        entry: IoObject,
        plane: *const c_char,
        child: *mut IoObject,
    ) -> i32;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFStringCreateWithCString(
        allocator: *const c_void,
        cstr: *const c_char,
        encoding: u32,
    ) -> CFStringRef;
    fn CFStringGetCString(s: CFStringRef, buf: *mut c_char, len: isize, encoding: u32) -> u8;
    fn CFDictionaryGetValue(dict: CFDictionaryRef, key: *const c_void) -> *const c_void;
    fn CFNumberGetValue(number: CFTypeRef, kind: i32, out: *mut c_void) -> u8;
    fn CFBooleanGetValue(boolean: CFTypeRef) -> u8;
    fn CFGetTypeID(cf: CFTypeRef) -> usize;
    fn CFNumberGetTypeID() -> usize;
    fn CFBooleanGetTypeID() -> usize;
    fn CFStringGetTypeID() -> usize;
    fn CFDictionaryGetTypeID() -> usize;
    fn CFRelease(cf: CFTypeRef);
}

/// An owned Core Foundation object, released on drop.
struct Cf(CFTypeRef);

impl Drop for Cf {
    fn drop(&mut self) {
        // SAFETY: only constructed from a non-null pointer returned by a
        // Create or Copy function, which transfers one reference to us; this
        // is the one release of it.
        unsafe { CFRelease(self.0) }
    }
}

impl Cf {
    fn owned(p: CFTypeRef) -> Option<Cf> {
        (!p.is_null()).then_some(Cf(p))
    }

    fn string(s: &str) -> Option<Cf> {
        let c = CString::new(s).ok()?;
        // SAFETY: `c` is a NUL-terminated buffer that outlives the call, which
        // copies it; a null allocator means the default one.
        Cf::owned(unsafe {
            CFStringCreateWithCString(std::ptr::null(), c.as_ptr(), K_CF_STRING_ENCODING_UTF8)
        })
    }
}

/// A value read out of the registry, copied into Rust so nothing borrowed
/// outlives the object it came from.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Int(i64),
    Bool(bool),
    Str(String),
    Dict(Vec<(String, Value)>),
    Other,
}

impl Value {
    pub fn int(&self) -> Option<i64> {
        match self {
            Value::Int(n) => Some(*n),
            _ => None,
        }
    }

    pub fn bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    /// A key of a dictionary value.
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Dict(kv) => kv.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
}

/// Copy a borrowed Core Foundation value, following dictionaries only for the
/// keys asked for.
///
/// # Safety
///
/// `p` must be null or a live Core Foundation object for the duration of the
/// call.
unsafe fn copy(p: CFTypeRef, keys: &[&str]) -> Value {
    if p.is_null() {
        return Value::Other;
    }
    // SAFETY: `p` is a live CF object, per this function's contract, and the
    // type-ID functions take no arguments.
    let (ty, number, boolean, string, dict) = unsafe {
        (
            CFGetTypeID(p),
            CFNumberGetTypeID(),
            CFBooleanGetTypeID(),
            CFStringGetTypeID(),
            CFDictionaryGetTypeID(),
        )
    };
    if ty == number {
        let mut n: i64 = 0;
        // SAFETY: `p` is a CFNumber, checked above, and `n` is eight writable
        // bytes, which is what SInt64 asks for. A value that does not fit is
        // reported by the return and refused.
        let ok = unsafe { CFNumberGetValue(p, K_CF_NUMBER_SINT64, (&raw mut n).cast()) };
        return if ok != 0 { Value::Int(n) } else { Value::Other };
    }
    if ty == boolean {
        // SAFETY: `p` is a CFBoolean, checked above.
        return Value::Bool(unsafe { CFBooleanGetValue(p) } != 0);
    }
    if ty == string {
        let mut buf = [0 as c_char; 256];
        // SAFETY: `p` is a CFString, checked above; `buf` is 256 writable
        // bytes and that length is passed, and the call writes a terminated
        // string within it or reports failure.
        let ok = unsafe {
            CFStringGetCString(
                p,
                buf.as_mut_ptr(),
                buf.len() as isize,
                K_CF_STRING_ENCODING_UTF8,
            )
        };
        if ok == 0 {
            return Value::Other;
        }
        // SAFETY: the call succeeded, so `buf` holds a NUL-terminated string.
        let s = unsafe { CStr::from_ptr(buf.as_ptr()) };
        return Value::Str(s.to_string_lossy().into_owned());
    }
    if ty == dict {
        let mut out = Vec::new();
        for key in keys {
            let Some(k) = Cf::string(key) else { continue };
            // SAFETY: `p` is a CFDictionary, checked above, and `k` a live
            // CFString. The value is borrowed from the dictionary, which
            // outlives this loop, and is copied before it returns.
            let v = unsafe { CFDictionaryGetValue(p, k.0) };
            if !v.is_null() {
                // SAFETY: a value held by a live dictionary.
                out.push((key.to_string(), unsafe { copy(v, &[]) }));
            }
        }
        return Value::Dict(out);
    }
    Value::Other
}

/// A registry entry, released on drop.
pub struct Entry(IoObject);

impl Drop for Entry {
    fn drop(&mut self) {
        // SAFETY: an object handed to us by the iterator or by a child
        // lookup, each of which transfers one reference; this is the release.
        unsafe { IOObjectRelease(self.0) };
    }
}

impl Entry {
    /// A property, copied. For a dictionary, only `keys` are copied out of it.
    pub fn property(&self, name: &str, keys: &[&str]) -> Option<Value> {
        let key = Cf::string(name)?;
        // SAFETY: `self.0` is a live registry entry and `key` a live CFString;
        // a null allocator is the default. The result is owned, and wrapped.
        let v = Cf::owned(unsafe {
            IORegistryEntryCreateCFProperty(self.0, key.0, std::ptr::null(), 0)
        })?;
        // SAFETY: `v` is live until the end of this function.
        Some(unsafe { copy(v.0, keys) })
    }

    /// The first child in the service plane.
    pub fn child(&self) -> Option<Entry> {
        let mut child: IoObject = 0;
        // SAFETY: `self.0` is a live entry, the plane name is a static
        // NUL-terminated string, and `child` a writable local.
        let rc = unsafe { IORegistryEntryGetChildEntry(self.0, c"IOService".as_ptr(), &mut child) };
        (rc == KERN_SUCCESS && child != 0).then_some(Entry(child))
    }
}

/// Every registered service of `class`.
pub fn services(class: &CStr) -> Vec<Entry> {
    // SAFETY: `class` is NUL-terminated and outlives the call.
    let matching = unsafe { IOServiceMatching(class.as_ptr()) };
    if matching.is_null() {
        return Vec::new();
    }
    let mut it: IoObject = 0;
    // SAFETY: `matching` is a fresh dictionary, whose one reference this call
    // consumes whatever it returns; `it` is a writable local.
    if unsafe { IOServiceGetMatchingServices(MAIN_PORT, matching, &mut it) } != KERN_SUCCESS {
        return Vec::new();
    }
    let iterator = Entry(it);
    let mut out = Vec::new();
    loop {
        // SAFETY: `iterator.0` is the live iterator just returned.
        let next = unsafe { IOIteratorNext(iterator.0) };
        if next == 0 {
            break;
        }
        out.push(Entry(next));
    }
    out
}

/// Cumulative counters for one whole disk.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DiskCounters {
    pub read_bytes: u64,
    pub write_bytes: u64,
    pub reads: u64,
    pub writes: u64,
    /// Nanoseconds spent servicing requests, summed over requests: the
    /// quantity Little's law divides by elapsed time to give mean queue depth.
    pub busy_ns: u64,
}

/// Every whole disk's counters, by BSD name.
pub fn disks() -> Vec<(String, DiskCounters)> {
    const KEYS: [&str; 6] = [
        "Bytes (Read)",
        "Bytes (Write)",
        "Operations (Read)",
        "Operations (Write)",
        "Total Time (Read)",
        "Total Time (Write)",
    ];
    services(c"IOBlockStorageDriver")
        .into_iter()
        .filter_map(|driver| {
            let stats = driver.property("Statistics", &KEYS)?;
            let name = driver
                .child()?
                .property("BSD Name", &[])?
                .str()?
                .to_string();
            let n = |k: &str| stats.get(k).and_then(Value::int).unwrap_or(0).max(0) as u64;
            Some((
                name,
                DiskCounters {
                    read_bytes: n("Bytes (Read)"),
                    write_bytes: n("Bytes (Write)"),
                    reads: n("Operations (Read)"),
                    writes: n("Operations (Write)"),
                    busy_ns: n("Total Time (Read)").saturating_add(n("Total Time (Write)")),
                },
            ))
        })
        .collect()
}

/// The battery, or `None` on a Mac without one.
///
/// A desktop Mac publishes an `AppleSmartBattery` anyway, every figure zero
/// and `BatteryInstalled` false — which is the check, so a Mac Studio does not
/// report a battery at 0% and discharging.
pub fn battery() -> Option<crate::sample::Power> {
    let b = services(c"AppleSmartBattery").into_iter().next()?;
    let p = |k: &str| b.property(k, &[]);
    if p("BatteryInstalled").and_then(|v| v.bool()) != Some(true) {
        return None;
    }
    let current = p("CurrentCapacity")?.int()?;
    let max = p("MaxCapacity")?.int().filter(|m| *m > 0)?;
    let charging = p("IsCharging").and_then(|v| v.bool()).unwrap_or(false);
    let plugged = p("ExternalConnected")
        .and_then(|v| v.bool())
        .unwrap_or(false);
    let state = match (charging, plugged) {
        (true, _) => "charging",
        (false, true) => "charged",
        (false, false) => "discharging",
    };
    // Millivolts and milliamps, the current negative while discharging. The
    // instantaneous figure where the firmware has one: `Amperage` is a
    // minute's average and lags a load that just started.
    let mv = p("Voltage").and_then(|v| v.int());
    let ma = p("InstantAmperage")
        .or_else(|| p("Amperage"))
        .and_then(|v| v.int());
    let watts = mv
        .zip(ma)
        .map(|(v, a)| -(v as f64 * a as f64 / 1e6) as f32)
        .filter(|w| *w != 0.0 && state != "charged");
    // `65535` is the firmware's "still estimating".
    let minutes = p("TimeRemaining")
        .and_then(|v| v.int())
        .filter(|m| (1..65535).contains(m) && state != "charged")
        .map(|m| m as u32);
    Some(crate::sample::Power {
        charge: (current as f32 / max as f32 * 100.0).clamp(0.0, 100.0),
        state: std::sync::Arc::from(state),
        watts,
        minutes,
    })
}

/// Every GPU that publishes its load: the accelerator's performance
/// statistics, which the Apple GPU keeps and any user may read.
pub fn gpus() -> Option<Vec<crate::sample::Gpu>> {
    const KEYS: [&str; 2] = ["Device Utilization %", "In use system memory"];
    let out: Vec<crate::sample::Gpu> = services(c"IOAccelerator")
        .into_iter()
        .filter_map(|a| {
            let stats = a.property("PerformanceStatistics", &KEYS)?;
            let util = stats.get("Device Utilization %")?.int()?;
            let name = a
                .property("model", &[])
                .and_then(|v| v.str().map(str::to_string))
                .unwrap_or_else(|| "GPU".to_string());
            Some(crate::sample::Gpu {
                name: std::sync::Arc::from(name),
                util: (util as f32).clamp(0.0, 100.0),
                mem_used: stats
                    .get("In use system memory")
                    .and_then(Value::int)
                    .map(|b| b.max(0) as u64),
                mem_total: None,
            })
        })
        .collect();
    (!out.is_empty()).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_macs_boot_disk_has_counters() {
        let disks = disks();
        let (name, c) = disks
            .iter()
            .find(|(n, _)| n == "disk0")
            .expect("no disk0 in the registry");
        assert_eq!(name, "disk0");
        // A running machine has read its own boot disk.
        assert!(c.read_bytes > 0 && c.reads > 0, "{c:?}");
    }

    #[test]
    fn this_macs_gpu_publishes_its_load() {
        let g = gpus().expect("no GPU in the registry");
        assert!((0.0..=100.0).contains(&g[0].util), "{g:?}");
        assert_ne!(&*g[0].name, "GPU", "the model was not read");
    }

    #[test]
    fn a_class_that_does_not_exist_has_no_services() {
        assert!(services(c"PoptopNoSuchClass").is_empty());
    }
}
