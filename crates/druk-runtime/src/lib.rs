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

    #[test]
    fn prints_a_number_and_a_string() {
        assert_eq!(format_value(RtValue::number(42)), "42");
        let string_value = unsafe { druk_rt_make_string(b"hi".as_ptr(), 2) };
        assert_eq!(format_value(string_value), "hi");
    }

    #[test]
    fn builds_and_indexes_an_array() {
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
        let elements = [RtValue::number(1), RtValue::number(2)];
        let array = unsafe { druk_rt_make_array(elements.as_ptr(), elements.len()) };
        druk_rt_array_set(array, RtValue::number(1), RtValue::number(99));
        let got = druk_rt_array_get(array, RtValue::number(1));
        assert_eq!(got.payload, 99);
    }

    #[test]
    fn values_equal_compares_strings_by_content_and_numbers_by_payload() {
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
