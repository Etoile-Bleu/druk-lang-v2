//! The Dzongkha builtin functions (`len`, `push`, `pop`, `typeof`, `input`)
//! and the `argv`/`argc` globals. `keys`/`values`/`contains` are not here:
//! they depend on a dictionary type `druk-ir` doesn't lower a call to yet.

use std::sync::OnceLock;

use druk_gc::handle_from_raw_index;

use crate::{
    heap, maybe_collect, RtValue, TAG_ARRAY, TAG_BOOL, TAG_CLOSURE, TAG_FUNCTION, TAG_NIL,
    TAG_NUMBER, TAG_STR, TAG_STRUCT,
};

/// The Druk program's own command-line arguments (everything after the
/// script path), set once by the driver right before the JIT-compiled
/// `main` runs. Read by `druk_rt_argv`/`druk_rt_argc`; empty if never set
/// (e.g. in a unit test that never calls [`set_args`]).
static ARGS: OnceLock<Vec<String>> = OnceLock::new();

/// Records the Druk program's own command-line arguments. Called exactly
/// once by `druk-codegen-llvm::run`, before it invokes the compiled `main`;
/// a second call (e.g. from a test harness that runs several programs in
/// the same process) is a harmless no-op, since every program in a given
/// process run gets the same arguments anyway.
pub fn set_args(args: Vec<String>) {
    let _ = ARGS.set(args);
}

/// Records a natively-compiled program's own command-line arguments from
/// the real `argc`/`argv` its OS process entry point received, skipping
/// `argv[0]` (the executable's own path) so a native binary's `argv`/`argc`
/// agree with `druk run script.druk a b`'s (which never includes `druk`
/// itself or the script path either). Called once, from the generated
/// `main`'s own prologue, before any user code runs; see
/// `druk-codegen-llvm`'s `MainKind::Native`.
///
/// # Safety
/// `argv` must be null, or point to at least `argc.max(0)` valid pointers,
/// each either null or pointing to a null-terminated byte string, for the
/// duration of this call. A real OS process entry point's `argv` always
/// satisfies this.
#[no_mangle]
pub unsafe extern "C" fn druk_rt_init_args(argc: i32, argv: *const *const std::ffi::c_char) {
    let mut args = Vec::new();
    if !argv.is_null() {
        for index in 1..argc.max(0) {
            let arg_ptr = *argv.add(index as usize);
            if arg_ptr.is_null() {
                continue;
            }
            args.push(
                std::ffi::CStr::from_ptr(arg_ptr)
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    set_args(args);
}

/// # Safety
/// `out` and `object` must each point to a valid `RtValue` (`out`
/// writable), alive for the duration of this call.
#[no_mangle]
pub unsafe extern "C" fn druk_rt_len(out: *mut RtValue, object: *const RtValue) {
    let object = *object;
    let len = match object.tag {
        TAG_ARRAY => {
            let handle = handle_from_raw_index(object.payload as u32);
            let guard = heap()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.array_len(handle) as i64
        }
        TAG_STR => {
            let handle = handle_from_raw_index(object.payload as u32);
            let guard = heap()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.get_string(handle).chars().count() as i64
        }
        _ => 0,
    };
    *out = RtValue::number(len);
}

/// Appends `value` to `array` in place and yields `nil`, matching
/// `druk-hir`'s `push` signature (`(array, value) -> nil`). A non-array
/// `array` is a silent no-op, the same not-an-array convention
/// `druk_rt_array_set` uses.
///
/// # Safety
/// `out`, `array`, and `value` must each point to a valid `RtValue` (`out`
/// writable), alive for the duration of this call.
#[no_mangle]
pub unsafe extern "C" fn druk_rt_push(
    out: *mut RtValue,
    array: *const RtValue,
    value: *const RtValue,
) {
    let array = *array;
    let value = *value;
    if array.tag == TAG_ARRAY {
        let handle = handle_from_raw_index(array.payload as u32);
        let mut guard = heap()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.array_push(handle, value.to_value());
    }
    *out = RtValue::nil();
}

/// Removes and returns `array`'s last element, or `nil` if `array` isn't an
/// array or is empty.
///
/// # Safety
/// `out` and `array` must each point to a valid `RtValue` (`out` writable),
/// alive for the duration of this call.
#[no_mangle]
pub unsafe extern "C" fn druk_rt_pop(out: *mut RtValue, array: *const RtValue) {
    let array = *array;
    if array.tag != TAG_ARRAY {
        *out = RtValue::nil();
        return;
    }
    let handle = handle_from_raw_index(array.payload as u32);
    let mut guard = heap()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *out = RtValue::from_value(guard.array_pop(handle));
}

fn type_name(tag: i64) -> &'static str {
    match tag {
        TAG_NIL => "nil",
        TAG_NUMBER => "number",
        TAG_BOOL => "bool",
        TAG_STR => "string",
        TAG_ARRAY => "array",
        TAG_STRUCT => "struct",
        TAG_FUNCTION | TAG_CLOSURE => "function",
        _ => "unknown",
    }
}

/// # Safety
/// `out` and `value` must each point to a valid `RtValue` (`out` writable),
/// alive for the duration of this call.
#[no_mangle]
pub unsafe extern "C" fn druk_rt_typeof(out: *mut RtValue, value: *const RtValue) {
    let value = *value;
    let mut guard = heap()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    maybe_collect(&mut guard, &[]);
    let handle = guard.alloc_string(type_name(value.tag).to_string());
    *out = RtValue {
        tag: TAG_STR,
        payload: handle.index() as i64,
    };
}

/// Reads one line from stdin, trimming the trailing newline, and returns it
/// as a string; an end-of-input or I/O error yields an empty string rather
/// than failing the program.
///
/// # Safety
/// `out` must point to a valid, writable `RtValue` slot.
#[no_mangle]
pub unsafe extern "C" fn druk_rt_input(out: *mut RtValue) {
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
    while line.ends_with(['\n', '\r']) {
        line.pop();
    }
    let mut guard = heap()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    maybe_collect(&mut guard, &[]);
    let handle = guard.alloc_string(line);
    *out = RtValue {
        tag: TAG_STR,
        payload: handle.index() as i64,
    };
}

/// # Safety
/// `out` must point to a valid, writable `RtValue` slot.
#[no_mangle]
pub unsafe extern "C" fn druk_rt_argv(out: *mut RtValue) {
    let args = ARGS.get().map(Vec::as_slice).unwrap_or(&[]);
    let mut guard = heap()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut values = Vec::with_capacity(args.len());
    for arg in args {
        maybe_collect(&mut guard, &values);
        let handle = guard.alloc_string(arg.clone());
        values.push(druk_gc::Value::Str(handle));
    }
    maybe_collect(&mut guard, &values);
    let handle = guard.alloc_array(values);
    *out = RtValue {
        tag: TAG_ARRAY,
        payload: handle.index() as i64,
    };
}

/// # Safety
/// `out` must point to a valid, writable `RtValue` slot.
#[no_mangle]
pub unsafe extern "C" fn druk_rt_argc(out: *mut RtValue) {
    let count = ARGS.get().map(Vec::len).unwrap_or(0);
    *out = RtValue::number(count as i64);
}
