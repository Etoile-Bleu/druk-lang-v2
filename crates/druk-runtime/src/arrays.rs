use druk_gc::{handle_from_raw_index, Value};

use crate::{heap, maybe_collect, RtValue, TAG_ARRAY};

/// # Safety
/// `out` must point to a valid, writable `RtValue` slot. `elements` must
/// point to at least `len` valid, initialized contiguous [`RtValue`]
/// structs, alive for the duration of this call. Codegen only ever passes a
/// pointer to a stack buffer it just finished writing.
#[no_mangle]
pub unsafe extern "C" fn druk_rt_make_array(
    out: *mut RtValue,
    elements: *const RtValue,
    len: usize,
) {
    let slice = std::slice::from_raw_parts(elements, len);
    let values: Vec<Value> = slice.iter().map(|element| element.to_value()).collect();
    let mut guard = heap()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    maybe_collect(&mut guard, &values);
    let handle = guard.alloc_array(values);
    *out = RtValue {
        tag: TAG_ARRAY,
        payload: handle.index() as i64,
    };
}

/// # Safety
/// `out`, `array`, and `index` must each point to a valid `RtValue` (`out`
/// writable), alive for the duration of this call.
#[no_mangle]
pub unsafe extern "C" fn druk_rt_array_get(
    out: *mut RtValue,
    array: *const RtValue,
    index: *const RtValue,
) {
    let array = *array;
    let index = *index;
    if array.tag != TAG_ARRAY {
        *out = RtValue::nil();
        return;
    }
    let handle = handle_from_raw_index(array.payload as u32);
    let guard = heap()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let index = index.payload.max(0) as usize;
    *out = RtValue::from_value(guard.array_get(handle, index));
}

/// # Safety
/// `out`, `array`, `index`, and `value` must each point to a valid `RtValue`
/// (`out` writable), alive for the duration of this call.
#[no_mangle]
pub unsafe extern "C" fn druk_rt_array_set(
    out: *mut RtValue,
    array: *const RtValue,
    index: *const RtValue,
    value: *const RtValue,
) {
    let array = *array;
    let index = *index;
    let value = *value;
    if array.tag != TAG_ARRAY {
        *out = value;
        return;
    }
    let handle = handle_from_raw_index(array.payload as u32);
    let mut guard = heap()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let index = index.payload.max(0) as usize;
    guard.array_set(handle, index, value.to_value());
    *out = value;
}
