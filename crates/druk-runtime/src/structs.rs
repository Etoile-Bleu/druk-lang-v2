use druk_gc::{handle_from_raw_index, Value};

use crate::{heap, maybe_collect, RtField, RtValue, TAG_STRUCT};

/// # Safety
/// `out` must point to a valid, writable `RtValue` slot. `fields` must
/// point to at least `len` valid, initialized contiguous [`RtField`]
/// entries, each with `name_ptr` valid for `name_len` bytes, alive for the
/// duration of this call. Codegen only ever passes a pointer to a stack
/// buffer it just finished writing, with each field's name pointing at a
/// compile-time constant embedded in the module.
#[no_mangle]
pub unsafe extern "C" fn druk_rt_make_struct(
    out: *mut RtValue,
    fields: *const RtField,
    len: usize,
) {
    let slice = std::slice::from_raw_parts(fields, len);
    let values: Vec<(String, Value)> = slice
        .iter()
        .map(|field| {
            let name_bytes = std::slice::from_raw_parts(field.name_ptr, field.name_len);
            let name = String::from_utf8_lossy(name_bytes).into_owned();
            (name, field.value.to_value())
        })
        .collect();
    let mut guard = heap()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let extra: Vec<Value> = values.iter().map(|(_, value)| *value).collect();
    maybe_collect(&mut guard, &extra);
    let handle = guard.alloc_struct(values);
    *out = RtValue {
        tag: TAG_STRUCT,
        payload: handle.index() as i64,
    };
}

/// # Safety
/// `out` and `object` must each point to a valid `RtValue` (`out`
/// writable). `name_ptr` must be valid for `name_len` bytes. All alive for
/// the duration of this call.
#[no_mangle]
pub unsafe extern "C" fn druk_rt_struct_get(
    out: *mut RtValue,
    object: *const RtValue,
    name_ptr: *const u8,
    name_len: usize,
) {
    let object = *object;
    if object.tag != TAG_STRUCT {
        *out = RtValue::nil();
        return;
    }
    let name_bytes = std::slice::from_raw_parts(name_ptr, name_len);
    let name = String::from_utf8_lossy(name_bytes);
    let handle = handle_from_raw_index(object.payload as u32);
    let guard = heap()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *out = RtValue::from_value(guard.struct_get(handle, &name));
}

/// # Safety
/// `out`, `object`, and `value` must each point to a valid `RtValue` (`out`
/// writable). `name_ptr` must be valid for `name_len` bytes. All alive for
/// the duration of this call.
#[no_mangle]
pub unsafe extern "C" fn druk_rt_struct_set(
    out: *mut RtValue,
    object: *const RtValue,
    name_ptr: *const u8,
    name_len: usize,
    value: *const RtValue,
) {
    let object = *object;
    let value = *value;
    if object.tag != TAG_STRUCT {
        *out = value;
        return;
    }
    let name_bytes = std::slice::from_raw_parts(name_ptr, name_len);
    let name = String::from_utf8_lossy(name_bytes);
    let handle = handle_from_raw_index(object.payload as u32);
    let mut guard = heap()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.struct_set(handle, &name, value.to_value());
    *out = value;
}
