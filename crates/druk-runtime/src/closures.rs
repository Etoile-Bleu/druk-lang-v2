use druk_gc::{handle_from_raw_index, Value};

use crate::{heap, maybe_collect, RtValue, TAG_CLOSURE};

/// # Safety
/// `out` must point to a valid, writable `RtValue` slot. `captures` must
/// point to at least `len` valid, initialized contiguous [`RtValue`]
/// structs, alive for the duration of this call. Codegen only ever passes
/// a pointer to a stack buffer it just finished writing.
#[no_mangle]
pub unsafe extern "C" fn druk_rt_make_closure(
    out: *mut RtValue,
    function_ptr: i64,
    captures: *const RtValue,
    len: usize,
) {
    let slice = std::slice::from_raw_parts(captures, len);
    let values: Vec<Value> = slice.iter().map(|value| value.to_value()).collect();
    let mut guard = heap()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    maybe_collect(&mut guard, &values);
    let handle = guard.alloc_closure(function_ptr as usize, values);
    *out = RtValue {
        tag: TAG_CLOSURE,
        payload: handle.index() as i64,
    };
}

/// Resolves the raw code pointer to call for `object`: a closure's stored
/// function pointer, or (for a plain function value, which carries no
/// environment) its payload directly, the same raw pointer convention
/// [`RtValue::function`] already uses. This is the one place
/// `druk-codegen-llvm` needs to tell a closure and a plain function value
/// apart; every other call-site detail (the leading `self` argument, the
/// rest of the call's arguments) is identical for both.
///
/// # Safety
/// `object` must point to a valid `RtValue`, alive for the duration of this
/// call.
#[no_mangle]
pub unsafe extern "C" fn druk_rt_closure_function_ptr(object: *const RtValue) -> i64 {
    let object = *object;
    if object.tag == TAG_CLOSURE {
        let handle = handle_from_raw_index(object.payload as u32);
        let guard = heap()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.closure_function_ptr(handle) as i64
    } else {
        object.payload
    }
}

/// # Safety
/// `object` must point to a valid `RtValue`, alive for the duration of this
/// call.
#[no_mangle]
pub unsafe extern "C" fn druk_rt_closure_get_capture(
    out: *mut RtValue,
    object: *const RtValue,
    index: i64,
) {
    let object = *object;
    if object.tag != TAG_CLOSURE {
        *out = RtValue::nil();
        return;
    }
    let handle = handle_from_raw_index(object.payload as u32);
    let guard = heap()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *out = RtValue::from_value(guard.closure_get_capture(handle, index.max(0) as usize));
}
