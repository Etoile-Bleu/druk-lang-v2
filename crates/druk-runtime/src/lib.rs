//! The `extern "C"` ABI the LLVM-generated code calls into: a tagged value
//! representation ([`RtValue`]) stable enough to build and read directly
//! from LLVM IR, plus the operations (printing, string/array/struct/closure
//! construction, indexing, equality, the Dzongkha builtins) that need the
//! GC heap behind them.
//!
//! Every function that takes or returns an [`RtValue`] does so through a
//! pointer (`*const RtValue` in, `*mut RtValue` out), never by value. A
//! 16-byte struct crosses the Windows x64 "C" ABI through a hidden `sret`
//! return slot and `byval` argument copies, but the x86-64 SysV ABI (Linux,
//! macOS) returns and passes the same struct directly in two registers:
//! two genuinely different calling conventions for the same Rust signature.
//! Passing everything by pointer sidesteps that difference entirely: a
//! pointer is passed in a register the same way on every platform, so
//! `druk-codegen-llvm` never needs to know which ABI it's targeting.
//!
//! Split by responsibility into sibling files, each re-exported here so
//! every `druk_rt_*` function keeps the same `druk_runtime::druk_rt_*` path
//! `druk-codegen-llvm` already calls it by: `strings`, `arrays`, `structs`,
//! `closures`, and `builtins` (the Dzongkha builtins and the `argv`/`argc`
//! globals). What stays in this file is the ABI types themselves
//! (`RtValue`, `RtField`), the GC heap/frame-stack singletons every other
//! file reaches through `heap()`/`maybe_collect`, and the handful of
//! operations that span every type (`print`, `is_truthy`, `values_equal`).

mod arrays;
mod builtins;
mod closures;
mod strings;
mod structs;

pub use arrays::*;
pub use builtins::*;
pub use closures::*;
pub use strings::*;
pub use structs::*;

use std::sync::{Mutex, OnceLock};

use druk_diagnostics::ice;
use druk_gc::{handle_from_raw_index, Heap, Value};

pub const TAG_NIL: i64 = 0;
pub const TAG_NUMBER: i64 = 1;
pub const TAG_BOOL: i64 = 2;
pub const TAG_STR: i64 = 3;
pub const TAG_ARRAY: i64 = 4;
pub const TAG_FUNCTION: i64 = 5;
pub const TAG_STRUCT: i64 = 6;
pub const TAG_CLOSURE: i64 = 7;

/// The ABI boundary type: a fixed `{i64, i64}` layout so codegen can build
/// and read one directly as an LLVM struct, with no padding ambiguity.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RtValue {
    pub tag: i64,
    pub payload: i64,
}

impl RtValue {
    pub fn nil() -> Self {
        Self {
            tag: TAG_NIL,
            payload: 0,
        }
    }

    pub fn number(value: i64) -> Self {
        Self {
            tag: TAG_NUMBER,
            payload: value,
        }
    }

    pub fn boolean(value: bool) -> Self {
        Self {
            tag: TAG_BOOL,
            payload: value as i64,
        }
    }

    pub fn function(pointer: usize) -> Self {
        Self {
            tag: TAG_FUNCTION,
            payload: pointer as i64,
        }
    }

    pub fn is_truthy(self) -> bool {
        match self.tag {
            TAG_NIL => false,
            TAG_NUMBER | TAG_BOOL => self.payload != 0,
            _ => true,
        }
    }

    fn to_value(self) -> Value {
        match self.tag {
            TAG_NUMBER => Value::Number(self.payload),
            TAG_BOOL => Value::Bool(self.payload != 0),
            TAG_STR => Value::Str(handle_from_raw_index(self.payload as u32)),
            TAG_ARRAY => Value::Array(handle_from_raw_index(self.payload as u32)),
            TAG_STRUCT => Value::Struct(handle_from_raw_index(self.payload as u32)),
            TAG_CLOSURE => Value::Closure(handle_from_raw_index(self.payload as u32)),
            TAG_FUNCTION => Value::Function(self.payload as usize),
            _ => Value::Nil,
        }
    }

    fn from_value(value: Value) -> Self {
        match value {
            Value::Nil => RtValue::nil(),
            Value::Number(number) => RtValue::number(number),
            Value::Bool(flag) => RtValue::boolean(flag),
            Value::Str(handle) => RtValue {
                tag: TAG_STR,
                payload: handle.index() as i64,
            },
            Value::Array(handle) => RtValue {
                tag: TAG_ARRAY,
                payload: handle.index() as i64,
            },
            Value::Struct(handle) => RtValue {
                tag: TAG_STRUCT,
                payload: handle.index() as i64,
            },
            Value::Closure(handle) => RtValue {
                tag: TAG_CLOSURE,
                payload: handle.index() as i64,
            },
            Value::Function(pointer) => RtValue::function(pointer),
        }
    }
}

/// One `name: value` pair of a struct literal as codegen assembles it: a
/// pointer to the field name's bytes (a compile-time constant, embedded the
/// same way a string literal's bytes are) alongside the already-computed
/// `RtValue`. `#[repr(C)]` so `druk-codegen-llvm` can build an array of
/// these as a plain LLVM struct type, the same way [`RtValue`] itself
/// crosses the FFI boundary.
#[repr(C)]
pub struct RtField {
    pub name_ptr: *const u8,
    pub name_len: usize,
    pub value: RtValue,
}

static HEAP: OnceLock<Mutex<Heap>> = OnceLock::new();

fn heap() -> &'static Mutex<Heap> {
    HEAP.get_or_init(|| Mutex::new(Heap::new()))
}

/// Every currently active root buffer, as `(pointer, element count)`,
/// registered by [`druk_rt_push_frame`] and unregistered by
/// [`druk_rt_pop_frame`]. This is the GC's root set, and it covers two kinds
/// of buffer, both pushed/popped the same way:
///
/// - A function's locals array, pushed once right after it's set up and
///   popped immediately before every one of that function's return points,
///   so at any moment this holds the locals of every frame currently on the
///   call stack, including recursive calls.
/// - A "pending siblings" buffer (`druk-codegen-llvm`'s
///   `compile_rooted_siblings`) spanning the evaluation of a list of
///   sub-expressions that are each computed before any of them is consumed
///   (an array literal's elements, a call's arguments, a binary
///   expression's operands), so a later sibling's own allocation can't
///   collect an earlier one that was never stored into a named local.
static FRAME_STACK: OnceLock<Mutex<Vec<(usize, usize)>>> = OnceLock::new();

fn frame_stack() -> &'static Mutex<Vec<(usize, usize)>> {
    FRAME_STACK.get_or_init(|| Mutex::new(Vec::new()))
}

/// # Safety
/// `ptr` must stay valid for exactly as long as this frame is registered.
/// Codegen only ever passes a pointer to the current function's own locals
/// array (a stack alloca) and calls [`druk_rt_pop_frame`] immediately before
/// every one of that function's return points, so the pointer outlives its
/// registration.
#[no_mangle]
pub unsafe extern "C" fn druk_rt_push_frame(ptr: *const RtValue, count: i64) {
    let mut guard = frame_stack()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.push((ptr as usize, count as usize));
}

#[no_mangle]
pub extern "C" fn druk_rt_pop_frame() {
    let mut guard = frame_stack()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if guard.pop().is_none() {
        ice!("druk_rt_pop_frame called with no matching druk_rt_push_frame");
    }
}

/// Flattens every registered frame's locals, plus `extra` (values about to
/// be stored into the object a caller is in the middle of allocating, which
/// otherwise wouldn't be reachable from any frame yet), into one root list
/// for [`Heap::collect`].
fn gather_roots(extra: &[Value]) -> Vec<Value> {
    let frames = frame_stack()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut roots = Vec::new();
    for &(ptr, count) in frames.iter() {
        // SAFETY: see `FRAME_STACK`'s doc comment. Every registered frame's
        // pointer is still valid for exactly `count` `RtValue`s.
        let slice = unsafe { std::slice::from_raw_parts(ptr as *const RtValue, count) };
        roots.extend(slice.iter().map(|value| value.to_value()));
    }
    roots.extend_from_slice(extra);
    roots
}

fn maybe_collect(heap: &mut Heap, extra_roots: &[Value]) {
    if heap.should_collect() {
        heap.collect(&gather_roots(extra_roots));
    }
}

fn format_value(value: RtValue) -> String {
    match value.tag {
        TAG_NIL => "nil".to_string(),
        TAG_NUMBER => value.payload.to_string(),
        TAG_BOOL => (value.payload != 0).to_string(),
        TAG_STR => {
            let handle = handle_from_raw_index(value.payload as u32);
            let guard = heap()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.get_string(handle).to_string()
        }
        TAG_ARRAY => "[array]".to_string(),
        TAG_STRUCT => "[struct]".to_string(),
        _ => "<function>".to_string(),
    }
}

/// # Safety
/// `value` must point to a valid, initialized `RtValue`, alive for the
/// duration of this call. Codegen only ever passes a pointer to a stack slot
/// it just finished writing.
#[no_mangle]
pub unsafe extern "C" fn druk_rt_print(value: *const RtValue) {
    println!("{}", format_value(*value));
}

/// A raw `0`/`1` rather than an `RtValue`, so codegen can compare it
/// directly against zero instead of round-tripping through the tagged
/// representation just to branch on it.
///
/// # Safety
/// `value` must point to a valid, initialized `RtValue`, alive for the
/// duration of this call.
#[no_mangle]
pub unsafe extern "C" fn druk_rt_is_truthy(value: *const RtValue) -> i64 {
    (*value).is_truthy() as i64
}

/// # Safety
/// `out`, `a`, and `b` must each point to a valid `RtValue` (`out`
/// writable), alive for the duration of this call.
#[no_mangle]
pub unsafe extern "C" fn druk_rt_values_equal(
    out: *mut RtValue,
    a: *const RtValue,
    b: *const RtValue,
) {
    let a = *a;
    let b = *b;
    let equal = if a.tag != b.tag {
        false
    } else if a.tag == TAG_STR {
        let handle_a = handle_from_raw_index(a.payload as u32);
        let handle_b = handle_from_raw_index(b.payload as u32);
        let guard = heap()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.get_string(handle_a) == guard.get_string(handle_b)
    } else {
        a.payload == b.payload
    };
    *out = RtValue::boolean(equal);
}

#[cfg(test)]
mod tests;
