use super::*;

/// `HEAP` and `FRAME_STACK` are process-wide statics shared by every
/// test in this binary, and now that `Heap::collect` actually reclaims
/// memory, two of these tests running concurrently on different threads
/// (the default `cargo test` behavior) could collect an object another
/// test is still using. Every test that touches the heap or the frame
/// stack takes this lock first, serializing them against each other.
static TEST_HEAP_LOCK: Mutex<()> = Mutex::new(());

unsafe fn make_string(text: &str) -> RtValue {
    let mut out = RtValue::nil();
    druk_rt_make_string(&mut out, text.as_ptr(), text.len());
    out
}

unsafe fn make_array(elements: &[RtValue]) -> RtValue {
    let mut out = RtValue::nil();
    druk_rt_make_array(&mut out, elements.as_ptr(), elements.len());
    out
}

unsafe fn array_get(array: RtValue, index: RtValue) -> RtValue {
    let mut out = RtValue::nil();
    druk_rt_array_get(&mut out, &array, &index);
    out
}

unsafe fn array_set(array: RtValue, index: RtValue, value: RtValue) -> RtValue {
    let mut out = RtValue::nil();
    druk_rt_array_set(&mut out, &array, &index, &value);
    out
}

unsafe fn values_equal(a: RtValue, b: RtValue) -> RtValue {
    let mut out = RtValue::nil();
    druk_rt_values_equal(&mut out, &a, &b);
    out
}

unsafe fn make_struct(fields: &[(&str, RtValue)]) -> RtValue {
    let entries: Vec<RtField> = fields
        .iter()
        .map(|(name, value)| RtField {
            name_ptr: name.as_ptr(),
            name_len: name.len(),
            value: *value,
        })
        .collect();
    let mut out = RtValue::nil();
    druk_rt_make_struct(&mut out, entries.as_ptr(), entries.len());
    out
}

unsafe fn struct_get(object: RtValue, name: &str) -> RtValue {
    let mut out = RtValue::nil();
    druk_rt_struct_get(&mut out, &object, name.as_ptr(), name.len());
    out
}

unsafe fn struct_set(object: RtValue, name: &str, value: RtValue) -> RtValue {
    let mut out = RtValue::nil();
    druk_rt_struct_set(&mut out, &object, name.as_ptr(), name.len(), &value);
    out
}

unsafe fn make_closure(function_ptr: i64, captures: &[RtValue]) -> RtValue {
    let mut out = RtValue::nil();
    druk_rt_make_closure(&mut out, function_ptr, captures.as_ptr(), captures.len());
    out
}

unsafe fn closure_get_capture(object: RtValue, index: i64) -> RtValue {
    let mut out = RtValue::nil();
    druk_rt_closure_get_capture(&mut out, &object, index);
    out
}

unsafe fn len(object: RtValue) -> RtValue {
    let mut out = RtValue::nil();
    druk_rt_len(&mut out, &object);
    out
}

unsafe fn push(array: RtValue, value: RtValue) -> RtValue {
    let mut out = RtValue::nil();
    druk_rt_push(&mut out, &array, &value);
    out
}

unsafe fn pop(array: RtValue) -> RtValue {
    let mut out = RtValue::nil();
    druk_rt_pop(&mut out, &array);
    out
}

unsafe fn type_of(value: RtValue) -> RtValue {
    let mut out = RtValue::nil();
    druk_rt_typeof(&mut out, &value);
    out
}

unsafe fn argv() -> RtValue {
    let mut out = RtValue::nil();
    druk_rt_argv(&mut out);
    out
}

unsafe fn argc() -> RtValue {
    let mut out = RtValue::nil();
    druk_rt_argc(&mut out);
    out
}

#[test]
fn prints_a_number_and_a_string() {
    let _guard = TEST_HEAP_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    assert_eq!(format_value(RtValue::number(42)), "42");
    let string_value = unsafe { make_string("hi") };
    assert_eq!(format_value(string_value), "hi");
}

#[test]
fn builds_and_indexes_an_array() {
    let _guard = TEST_HEAP_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let elements = [
        RtValue::number(10),
        RtValue::number(20),
        RtValue::number(30),
    ];
    let array = unsafe { make_array(&elements) };
    let got = unsafe { array_get(array, RtValue::number(1)) };
    assert_eq!(got.tag, TAG_NUMBER);
    assert_eq!(got.payload, 20);
}

#[test]
fn array_set_writes_through() {
    let _guard = TEST_HEAP_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let elements = [RtValue::number(1), RtValue::number(2)];
    let array = unsafe { make_array(&elements) };
    unsafe { array_set(array, RtValue::number(1), RtValue::number(99)) };
    let got = unsafe { array_get(array, RtValue::number(1)) };
    assert_eq!(got.payload, 99);
}

#[test]
fn builds_and_reads_a_struct_field() {
    let _guard = TEST_HEAP_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let point = unsafe { make_struct(&[("x", RtValue::number(1)), ("y", RtValue::number(2))]) };
    let got = unsafe { struct_get(point, "y") };
    assert_eq!(got.tag, TAG_NUMBER);
    assert_eq!(got.payload, 2);
}

#[test]
fn struct_get_on_a_missing_field_returns_nil() {
    let _guard = TEST_HEAP_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let point = unsafe { make_struct(&[("x", RtValue::number(1))]) };
    let got = unsafe { struct_get(point, "missing") };
    assert_eq!(got.tag, TAG_NIL);
}

#[test]
fn struct_set_writes_through() {
    let _guard = TEST_HEAP_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let point = unsafe { make_struct(&[("x", RtValue::number(1))]) };
    unsafe { struct_set(point, "x", RtValue::number(99)) };
    let got = unsafe { struct_get(point, "x") };
    assert_eq!(got.payload, 99);
}

#[test]
fn builds_and_reads_a_closure_capture() {
    let _guard = TEST_HEAP_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let closure = unsafe { make_closure(0x1234, &[RtValue::number(42)]) };
    assert_eq!(closure.tag, TAG_CLOSURE);
    let function_ptr = unsafe { druk_rt_closure_function_ptr(&closure) };
    assert_eq!(function_ptr, 0x1234);
    let captured = unsafe { closure_get_capture(closure, 0) };
    assert_eq!(captured.payload, 42);
}

#[test]
fn closure_function_ptr_on_a_plain_function_value_is_its_payload() {
    let function_value = RtValue::function(0x5678);
    let function_ptr = unsafe { druk_rt_closure_function_ptr(&function_value) };
    assert_eq!(function_ptr, 0x5678);
}

#[test]
fn collection_triggered_by_unrelated_allocations_spares_a_rooted_local() {
    let _guard = TEST_HEAP_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let kept = unsafe { make_string("kept") };
    let frame = [kept];
    unsafe { druk_rt_push_frame(frame.as_ptr(), frame.len() as i64) };

    for i in 0..300 {
        unsafe { make_string(&format!("garbage{i}")) };
    }

    assert_eq!(format_value(kept), "kept");
    druk_rt_pop_frame();
}

#[test]
fn pending_siblings_buffer_protects_earlier_values_during_later_allocations() {
    let _guard = TEST_HEAP_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    // Mirrors what `druk-codegen-llvm`'s `compile_rooted_siblings` emits
    // for something like an array literal `["first", noisy(), ...]`: a
    // buffer holding every sibling evaluated so far, registered as a
    // frame for the whole span, with each slot nil until its own value
    // is computed and stored into it.
    let mut pending = [RtValue::nil(), RtValue::nil()];
    unsafe { druk_rt_push_frame(pending.as_ptr(), pending.len() as i64) };

    pending[0] = unsafe { make_string("first") };
    let first = pending[0];

    for i in 0..300 {
        pending[1] = unsafe { make_string(&format!("garbage{i}")) };
    }

    assert_eq!(format_value(first), "first");
    druk_rt_pop_frame();
}

#[test]
fn values_equal_compares_strings_by_content_and_numbers_by_payload() {
    let _guard = TEST_HEAP_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let a = unsafe { make_string("same") };
    let b = unsafe { make_string("same") };
    assert!(unsafe { values_equal(a, b) }.is_truthy());
    assert!(unsafe { values_equal(RtValue::number(5), RtValue::number(5)) }.is_truthy());
    assert!(!unsafe { values_equal(RtValue::number(5), RtValue::number(6)) }.is_truthy());
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
    assert_eq!(unsafe { druk_rt_is_truthy(&RtValue::nil()) }, 0);
    assert_eq!(unsafe { druk_rt_is_truthy(&RtValue::number(7)) }, 1);
}

#[test]
fn len_reports_an_arrays_element_count() {
    let _guard = TEST_HEAP_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let array = unsafe { make_array(&[RtValue::number(1), RtValue::number(2)]) };
    let got = unsafe { len(array) };
    assert_eq!(got.tag, TAG_NUMBER);
    assert_eq!(got.payload, 2);
}

#[test]
fn len_reports_a_strings_character_count() {
    let _guard = TEST_HEAP_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let string = unsafe { make_string("hello") };
    assert_eq!(unsafe { len(string) }.payload, 5);
}

#[test]
fn len_on_a_non_sized_value_is_zero() {
    assert_eq!(unsafe { len(RtValue::number(42)) }.payload, 0);
}

#[test]
fn push_appends_and_pop_removes_the_last_element() {
    let _guard = TEST_HEAP_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let array = unsafe { make_array(&[RtValue::number(1)]) };
    let push_result = unsafe { push(array, RtValue::number(2)) };
    assert_eq!(push_result.tag, TAG_NIL);
    assert_eq!(unsafe { len(array) }.payload, 2);

    let popped = unsafe { pop(array) };
    assert_eq!(popped.payload, 2);
    assert_eq!(unsafe { len(array) }.payload, 1);
}

#[test]
fn pop_on_an_empty_array_returns_nil() {
    let _guard = TEST_HEAP_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let array = unsafe { make_array(&[]) };
    assert_eq!(unsafe { pop(array) }.tag, TAG_NIL);
}

#[test]
fn typeof_names_every_tag() {
    let _guard = TEST_HEAP_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let string = unsafe { make_string("hi") };
    let array = unsafe { make_array(&[]) };
    let closure = unsafe { make_closure(0x1, &[]) };
    let cases: &[(RtValue, &str)] = &[
        (RtValue::nil(), "nil"),
        (RtValue::number(1), "number"),
        (RtValue::boolean(true), "bool"),
        (string, "string"),
        (array, "array"),
        (RtValue::function(0x1), "function"),
        (closure, "function"),
    ];
    for (value, expected) in cases {
        let got = unsafe { type_of(*value) };
        assert_eq!(got.tag, TAG_STR);
        let guard = heap()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let handle = handle_from_raw_index(got.payload as u32);
        assert_eq!(guard.get_string(handle), *expected);
    }
}

#[test]
fn input_always_returns_a_string_even_at_end_of_input() {
    let _guard = TEST_HEAP_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut out = RtValue::nil();
    unsafe { druk_rt_input(&mut out) };
    assert_eq!(out.tag, TAG_STR);
}

#[test]
fn argv_and_argc_are_empty_until_set_args_is_called_in_this_process() {
    let _guard = TEST_HEAP_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let count = unsafe { argc() };
    assert_eq!(count.tag, TAG_NUMBER);
    let array = unsafe { argv() };
    assert_eq!(array.tag, TAG_ARRAY);
    let guard = heap()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let handle = handle_from_raw_index(array.payload as u32);
    assert_eq!(guard.array_len(handle) as i64, count.payload);
}
