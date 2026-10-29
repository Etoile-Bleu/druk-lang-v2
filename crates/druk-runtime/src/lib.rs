//! The `extern "C"` ABI the LLVM-generated code calls into: a tagged value
//! representation ([`RtValue`]) stable enough to build and read directly
//! from LLVM IR, plus the operations (printing, string/array construction,
//! array indexing, equality) that need the GC heap behind them.
//!
//! Scope for this first version: the operations the ten bundled example
//! programs actually exercise. Printing an array's own contents, and the
//! dzongkha builtins (`len`, `push`, `pop`, ...), are not implemented yet
//! since no bundled example calls them.

use std::sync::{Mutex, OnceLock};

use druk_diagnostics::ice;
use druk_gc::{handle_from_raw_index, Heap, Value};

pub const TAG_NIL: i64 = 0;
pub const TAG_NUMBER: i64 = 1;
pub const TAG_BOOL: i64 = 2;
pub const TAG_STR: i64 = 3;
pub const TAG_ARRAY: i64 = 4;
pub const TAG_FUNCTION: i64 = 5;

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
            Value::Function(pointer) => RtValue::function(pointer),
        }
    }
}

static HEAP: OnceLock<Mutex<Heap>> = OnceLock::new();

fn heap() -> &'static Mutex<Heap> {
    HEAP.get_or_init(|| Mutex::new(Heap::new()))
}

/// Every currently active Druk call frame's locals buffer, as
/// `(pointer, element count)`, registered by [`druk_rt_push_frame`] and
/// unregistered by [`druk_rt_pop_frame`]. This is the GC's root set: codegen
/// pairs every function entry with a push and every one of that function's
/// return points with a pop, so at any moment this holds exactly the locals
/// of every frame currently on the call stack, including recursive calls.
///
/// This roots named local variables, not sub-expression temporaries that
/// never get stored into one (for example, an earlier element of an array
/// literal still being assembled when evaluating a later element triggers a
/// collection). Closing that gap needs precise stack maps from LLVM, which
/// `druk-codegen-llvm` does not have; the risk is bounded by how rarely a
/// collection's threshold is crossed in the middle of evaluating a single
/// expression.
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
        // SAFETY: see `FRAME_STACK`'s doc comment — every registered frame's
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
        _ => "<function>".to_string(),
    }
}

#[no_mangle]
pub extern "C" fn druk_rt_print(value: RtValue) {
    println!("{}", format_value(value));
}

/// A raw `0`/`1` rather than an `RtValue`, so codegen can compare it
/// directly against zero instead of round-tripping through the tagged
/// representation just to branch on it.
#[no_mangle]
pub extern "C" fn druk_rt_is_truthy(value: RtValue) -> i64 {
    value.is_truthy() as i64
}

/// # Safety
/// `bytes` must point to at least `len` valid, initialized bytes, alive for
/// the duration of this call. Codegen only ever passes a pointer into a
/// static byte buffer embedded in the compiled module, which satisfies this.
#[no_mangle]
pub unsafe extern "C" fn druk_rt_make_string(bytes: *const u8, len: usize) -> RtValue {
    let slice = std::slice::from_raw_parts(bytes, len);
    let owned = String::from_utf8_lossy(slice).into_owned();
    let mut guard = heap()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    maybe_collect(&mut guard, &[]);
    let handle = guard.alloc_string(owned);
    RtValue {
        tag: TAG_STR,
        payload: handle.index() as i64,
    }
}

/// # Safety
/// `elements` must point to at least `len` valid, initialized contiguous
/// [`RtValue`] structs, alive for the duration of this call. Codegen only
/// ever passes a pointer to a stack buffer it just finished writing.
#[no_mangle]
pub unsafe extern "C" fn druk_rt_make_array(elements: *const RtValue, len: usize) -> RtValue {
    let slice = std::slice::from_raw_parts(elements, len);
    let values: Vec<Value> = slice.iter().map(|element| element.to_value()).collect();
    let mut guard = heap()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    maybe_collect(&mut guard, &values);
    let handle = guard.alloc_array(values);
    RtValue {
        tag: TAG_ARRAY,
        payload: handle.index() as i64,
    }
}

#[no_mangle]
pub extern "C" fn druk_rt_array_get(array: RtValue, index: RtValue) -> RtValue {
    if array.tag != TAG_ARRAY {
        return RtValue::nil();
    }
    let handle = handle_from_raw_index(array.payload as u32);
    let guard = heap()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let index = index.payload.max(0) as usize;
    RtValue::from_value(guard.array_get(handle, index))
}

#[no_mangle]
pub extern "C" fn druk_rt_array_set(array: RtValue, index: RtValue, value: RtValue) -> RtValue {
    if array.tag != TAG_ARRAY {
        return value;
    }
    let handle = handle_from_raw_index(array.payload as u32);
    let mut guard = heap()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let index = index.payload.max(0) as usize;
    guard.array_set(handle, index, value.to_value());
    value
}

#[no_mangle]
pub extern "C" fn druk_rt_values_equal(a: RtValue, b: RtValue) -> RtValue {
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
    RtValue::boolean(equal)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `HEAP` and `FRAME_STACK` are process-wide statics shared by every
    /// test in this binary, and now that `Heap::collect` actually reclaims
    /// memory, two of these tests running concurrently on different threads
    /// (the default `cargo test` behavior) could collect an object another
    /// test is still using. Every test that touches the heap or the frame
    /// stack takes this lock first, serializing them against each other.
    static TEST_HEAP_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn prints_a_number_and_a_string() {
        let _guard = TEST_HEAP_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        assert_eq!(format_value(RtValue::number(42)), "42");
        let string_value = unsafe { druk_rt_make_string(b"hi".as_ptr(), 2) };
        assert_eq!(format_value(string_value), "hi");
    }

    #[test]
    fn builds_and_indexes_an_array() {
        let _guard = TEST_HEAP_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let elements = [
            RtValue::number(10),
            RtValue::number(20),
            RtValue::number(30),
        ];
        let array = unsafe { druk_rt_make_array(elements.as_ptr(), elements.len()) };
        let got = druk_rt_array_get(array, RtValue::number(1));
        assert_eq!(got.tag, TAG_NUMBER);
        assert_eq!(got.payload, 20);
    }

    #[test]
    fn array_set_writes_through() {
        let _guard = TEST_HEAP_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let elements = [RtValue::number(1), RtValue::number(2)];
        let array = unsafe { druk_rt_make_array(elements.as_ptr(), elements.len()) };
        druk_rt_array_set(array, RtValue::number(1), RtValue::number(99));
        let got = druk_rt_array_get(array, RtValue::number(1));
        assert_eq!(got.payload, 99);
    }

    #[test]
    fn collection_triggered_by_unrelated_allocations_spares_a_rooted_local() {
        let _guard = TEST_HEAP_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

        let kept = unsafe { druk_rt_make_string(b"kept".as_ptr(), 4) };
        let frame = [kept];
        unsafe { druk_rt_push_frame(frame.as_ptr(), frame.len() as i64) };

        for i in 0..300 {
            let garbage = format!("garbage{i}");
            unsafe { druk_rt_make_string(garbage.as_bytes().as_ptr(), garbage.len()) };
        }

        assert_eq!(format_value(kept), "kept");
        druk_rt_pop_frame();
    }

    #[test]
    fn values_equal_compares_strings_by_content_and_numbers_by_payload() {
        let _guard = TEST_HEAP_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let a = unsafe { druk_rt_make_string(b"same".as_ptr(), 4) };
        let b = unsafe { druk_rt_make_string(b"same".as_ptr(), 4) };
        assert!(druk_rt_values_equal(a, b).is_truthy());
        assert!(druk_rt_values_equal(RtValue::number(5), RtValue::number(5)).is_truthy());
        assert!(!druk_rt_values_equal(RtValue::number(5), RtValue::number(6)).is_truthy());
    }

    #[test]
    fn truthiness_matches_druk_semantics() {
        assert!(!RtValue::nil().is_truthy());
        assert!(!RtValue::number(0).is_truthy());
        assert!(RtValue::number(1).is_truthy());
        assert!(!RtValue::boolean(false).is_truthy());
        assert!(RtValue::boolean(true).is_truthy());
    }

    #[test]
    fn is_truthy_extern_matches_the_method() {
        assert_eq!(druk_rt_is_truthy(RtValue::nil()), 0);
        assert_eq!(druk_rt_is_truthy(RtValue::number(7)), 1);
    }
}
