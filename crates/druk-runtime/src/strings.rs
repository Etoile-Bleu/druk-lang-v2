use crate::{heap, maybe_collect, RtValue, TAG_STR};

/// # Safety
/// `out` must point to a valid, writable `RtValue` slot. `bytes` must point
/// to at least `len` valid, initialized bytes, alive for the duration of
/// this call. Codegen only ever passes a pointer into a static byte buffer
/// embedded in the compiled module, which satisfies this.
#[no_mangle]
pub unsafe extern "C" fn druk_rt_make_string(out: *mut RtValue, bytes: *const u8, len: usize) {
    let slice = std::slice::from_raw_parts(bytes, len);
    let owned = String::from_utf8_lossy(slice).into_owned();
    let mut guard = heap()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    maybe_collect(&mut guard, &[]);
    let handle = guard.alloc_string(owned);
    *out = RtValue {
        tag: TAG_STR,
        payload: handle.index() as i64,
    };
}
