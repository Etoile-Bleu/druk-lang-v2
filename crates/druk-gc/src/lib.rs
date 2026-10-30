//! A handle-based heap for Druk's two garbage-collected types, arrays and
//! strings. Objects are referenced by [`Handle`] (an index), never by a raw
//! pointer, so a Druk value can never forge or outlive a heap slot.
//!
//! [`Heap::collect`] is a real mark-and-sweep: it traces every [`Value`]
//! reachable from the `roots` slice it is given, transitively through array
//! elements, and reclaims every heap slot that tracing didn't reach onto an
//! internal free list so later allocations reuse the space instead of
//! growing the heap forever. This is deliberately the same shape as a
//! textbook mark-and-sweep over an object list (see `craftinginterpreters.com`'s
//! garbage collection chapter) rather than a moving/compacting collector:
//! since objects are already addressed by index rather than by raw pointer,
//! there's no pointer-invalidation problem a moving collector would need to
//! solve.
//!
//! Correctness here rests entirely on `roots` actually naming every heap
//! value the running program can still read at the moment of collection.
//! This crate has no way to discover that on its own: LLVM-generated code
//! carries no stack maps `collect` could scan, so finding and supplying a
//! complete root set across every active call frame (including recursive
//! calls) is the caller's job, done at the `druk-runtime`/`druk-codegen-llvm`
//! boundary, and is deliberately out of scope for this crate.
//! [`Heap::should_collect`] exposes a threshold-and-grow heuristic (the same
//! one `clox` and Lua use) so a caller knows *when* to collect; it is not
//! called automatically by `alloc_string`/`alloc_array`.

mod arrays;
mod closures;
mod strings;
mod structs;

use druk_diagnostics::ice;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Handle(u32);

impl Handle {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// Reconstructs a handle from the raw index a [`Value`] was packed into to
/// cross the FFI boundary into generated code. This exists for
/// `druk-runtime` to unpack a handle this same heap already produced from
/// one of its `alloc_*` methods. It is not a general way to fabricate a
/// handle to an arbitrary slot, and nothing outside the runtime layer calls it.
pub fn handle_from_raw_index(index: u32) -> Handle {
    Handle(index)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value {
    Nil,
    Number(i64),
    Bool(bool),
    Str(Handle),
    Array(Handle),
    Struct(Handle),
    Closure(Handle),
    /// A raw native function pointer, cast back to a callable at the call
    /// site. Functions are static code, not heap objects, so this never
    /// needs a `Handle` or GC tracing.
    Function(usize),
}

impl Value {
    fn as_handle(&self) -> Option<Handle> {
        match self {
            Value::Str(handle)
            | Value::Array(handle)
            | Value::Struct(handle)
            | Value::Closure(handle) => Some(*handle),
            Value::Nil | Value::Number(_) | Value::Bool(_) | Value::Function(_) => None,
        }
    }
}

#[derive(Debug, Clone)]
enum Object {
    Str(String),
    Array(Vec<Value>),
    Struct(Vec<(String, Value)>),
    /// A lambda that captured a variable from its enclosing scope: the raw
    /// function pointer to call (static code, not itself traced, the same
    /// way [`Value::Function`] isn't), plus the captured values (traced).
    Closure {
        function_ptr: usize,
        captures: Vec<Value>,
    },
}

#[derive(Debug)]
enum Slot {
    Free { next: Option<u32> },
    Live { object: Object, marked: bool },
}

/// The starting point for [`Heap::should_collect`]'s threshold, and the
/// floor `next_gc` never shrinks below. Chosen to be large enough that a
/// short-lived script never collects at all, the same reasoning `clox` uses
/// for its own starting threshold.
const INITIAL_GC_THRESHOLD: usize = 256;

/// After a collection, the next one is due once the live set grows to this
/// multiple of its current size, the same grow-proportionally-to-live-set
/// heuristic `clox` and Lua use, so collection frequency adapts to how much
/// of the heap is actually garbage instead of firing on a fixed interval.
const GC_GROWTH_FACTOR: usize = 2;

#[derive(Debug)]
pub struct Heap {
    slots: Vec<Slot>,
    free_list: Option<u32>,
    live_count: usize,
    next_gc: usize,
}

impl Default for Heap {
    fn default() -> Self {
        Self::new()
    }
}

impl Heap {
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            free_list: None,
            live_count: 0,
            next_gc: INITIAL_GC_THRESHOLD,
        }
    }

    /// Whether the live set has grown enough since the last collection (or
    /// since the heap was created) to be worth collecting again. The caller
    /// decides when to actually check this and call [`Heap::collect`],
    /// typically at a point where it can also supply a complete root set.
    pub fn should_collect(&self) -> bool {
        self.live_count >= self.next_gc
    }

    fn alloc(&mut self, object: Object) -> Handle {
        self.live_count += 1;
        if let Some(index) = self.free_list {
            let next = match &self.slots[index as usize] {
                Slot::Free { next } => *next,
                Slot::Live { .. } => ice!("heap free list points at a live slot"),
            };
            self.free_list = next;
            self.slots[index as usize] = Slot::Live {
                object,
                marked: false,
            };
            return Handle(index);
        }
        let index = self.slots.len() as u32;
        self.slots.push(Slot::Live {
            object,
            marked: false,
        });
        Handle(index)
    }

    fn object(&self, handle: Handle) -> &Object {
        match &self.slots[handle.index()] {
            Slot::Live { object, .. } => object,
            Slot::Free { .. } => ice!("handle refers to a freed heap slot"),
        }
    }

    fn object_mut(&mut self, handle: Handle) -> &mut Object {
        match &mut self.slots[handle.index()] {
            Slot::Live { object, .. } => object,
            Slot::Free { .. } => ice!("handle refers to a freed heap slot"),
        }
    }

    /// Reclaims every heap slot not reachable from `roots`, tracing
    /// transitively through array elements, and resets the
    /// [`Heap::should_collect`] threshold from the surviving live set.
    pub fn collect(&mut self, roots: &[Value]) {
        let mut worklist: Vec<Handle> = roots.iter().filter_map(Value::as_handle).collect();

        while let Some(handle) = worklist.pop() {
            let already_marked = match &mut self.slots[handle.index()] {
                Slot::Live { marked, .. } if *marked => true,
                Slot::Live { marked, .. } => {
                    *marked = true;
                    false
                }
                Slot::Free { .. } => ice!("GC root points at an already-freed heap slot"),
            };
            if already_marked {
                continue;
            }
            match &self.slots[handle.index()] {
                Slot::Live {
                    object: Object::Array(elements),
                    ..
                } => worklist.extend(elements.iter().filter_map(Value::as_handle)),
                Slot::Live {
                    object: Object::Struct(fields),
                    ..
                } => worklist.extend(fields.iter().filter_map(|(_, value)| value.as_handle())),
                Slot::Live {
                    object: Object::Closure { captures, .. },
                    ..
                } => worklist.extend(captures.iter().filter_map(Value::as_handle)),
                _ => {}
            }
        }

        for index in 0..self.slots.len() {
            match &mut self.slots[index] {
                Slot::Live { marked, .. } if *marked => *marked = false,
                Slot::Live { .. } => {
                    self.slots[index] = Slot::Free {
                        next: self.free_list,
                    };
                    self.free_list = Some(index as u32);
                    self.live_count -= 1;
                }
                Slot::Free { .. } => {}
            }
        }

        self.next_gc = (self.live_count * GC_GROWTH_FACTOR).max(INITIAL_GC_THRESHOLD);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocates_and_reads_back_a_string() {
        let mut heap = Heap::new();
        let handle = heap.alloc_string("hello".to_string());
        assert_eq!(heap.get_string(handle), "hello");
    }

    #[test]
    fn allocates_and_indexes_an_array() {
        let mut heap = Heap::new();
        let handle = heap.alloc_array(vec![Value::Number(1), Value::Number(2), Value::Number(3)]);
        assert_eq!(heap.array_len(handle), 3);
        assert_eq!(heap.array_get(handle, 1), Value::Number(2));
    }

    #[test]
    fn writes_through_array_set() {
        let mut heap = Heap::new();
        let handle = heap.alloc_array(vec![Value::Number(1), Value::Number(2)]);
        heap.array_set(handle, 1, Value::Number(99));
        assert_eq!(heap.array_get(handle, 1), Value::Number(99));
    }

    #[test]
    fn array_push_appends_and_grows_the_length() {
        let mut heap = Heap::new();
        let handle = heap.alloc_array(vec![Value::Number(1)]);
        heap.array_push(handle, Value::Number(2));
        assert_eq!(heap.array_len(handle), 2);
        assert_eq!(heap.array_get(handle, 1), Value::Number(2));
    }

    #[test]
    fn array_pop_removes_and_returns_the_last_element() {
        let mut heap = Heap::new();
        let handle = heap.alloc_array(vec![Value::Number(1), Value::Number(2)]);
        assert_eq!(heap.array_pop(handle), Value::Number(2));
        assert_eq!(heap.array_len(handle), 1);
    }

    #[test]
    fn array_pop_on_an_empty_array_returns_nil() {
        let mut heap = Heap::new();
        let handle = heap.alloc_array(Vec::new());
        assert_eq!(heap.array_pop(handle), Value::Nil);
    }

    #[test]
    fn out_of_bounds_array_get_returns_nil_instead_of_panicking() {
        let mut heap = Heap::new();
        let handle = heap.alloc_array(vec![Value::Number(1)]);
        assert_eq!(heap.array_get(handle, 5), Value::Nil);
    }

    #[test]
    fn collect_with_no_roots_frees_every_object() {
        let mut heap = Heap::new();
        heap.alloc_string("unreachable".to_string());
        heap.alloc_array(vec![Value::Number(1)]);
        heap.collect(&[]);

        // The freed slots must be handed back out before the heap grows.
        let reused = heap.alloc_string("reused".to_string());
        assert!(reused.index() < 2, "expected a freed slot to be reused");
    }

    #[test]
    fn collect_keeps_a_rooted_string_and_reuses_the_rest() {
        let mut heap = Heap::new();
        let kept = heap.alloc_string("kept".to_string());
        heap.alloc_string("garbage".to_string());
        heap.collect(&[Value::Str(kept)]);

        assert_eq!(heap.get_string(kept), "kept");
        let reused = heap.alloc_string("new".to_string());
        assert_eq!(reused.index(), 1, "the garbage slot should be reused");
    }

    #[test]
    fn collect_traces_through_nested_arrays() {
        let mut heap = Heap::new();
        let inner = heap.alloc_array(vec![Value::Number(42)]);
        let outer = heap.alloc_array(vec![Value::Array(inner)]);
        heap.collect(&[Value::Array(outer)]);

        assert_eq!(heap.array_get(inner, 0), Value::Number(42));
    }

    #[test]
    fn collect_frees_an_array_once_its_only_reference_is_gone() {
        let mut heap = Heap::new();
        let inner = heap.alloc_array(vec![Value::Number(1)]);
        heap.alloc_array(vec![Value::Array(inner)]);
        // Root nothing: both the outer array and, transitively, `inner`
        // become unreachable even though nothing explicitly frees `inner`.
        heap.collect(&[]);

        let reused = heap.alloc_array(vec![Value::Number(2)]);
        assert!(reused.index() < 2, "expected a freed slot to be reused");
    }

    #[test]
    fn allocates_and_reads_back_a_struct_field() {
        let mut heap = Heap::new();
        let handle = heap.alloc_struct(vec![
            ("x".to_string(), Value::Number(1)),
            ("y".to_string(), Value::Number(2)),
        ]);
        assert_eq!(heap.struct_get(handle, "y"), Value::Number(2));
    }

    #[test]
    fn struct_get_on_a_missing_field_returns_nil() {
        let mut heap = Heap::new();
        let handle = heap.alloc_struct(vec![("x".to_string(), Value::Number(1))]);
        assert_eq!(heap.struct_get(handle, "missing"), Value::Nil);
    }

    #[test]
    fn writes_through_struct_set() {
        let mut heap = Heap::new();
        let handle = heap.alloc_struct(vec![("x".to_string(), Value::Number(1))]);
        heap.struct_set(handle, "x", Value::Number(99));
        assert_eq!(heap.struct_get(handle, "x"), Value::Number(99));
    }

    #[test]
    fn struct_set_on_a_missing_field_is_a_no_op() {
        let mut heap = Heap::new();
        let handle = heap.alloc_struct(vec![("x".to_string(), Value::Number(1))]);
        heap.struct_set(handle, "missing", Value::Number(99));
        assert_eq!(heap.struct_get(handle, "missing"), Value::Nil);
        assert_eq!(heap.struct_get(handle, "x"), Value::Number(1));
    }

    #[test]
    fn collect_traces_through_a_struct_field() {
        let mut heap = Heap::new();
        let inner = heap.alloc_string("nested".to_string());
        let outer = heap.alloc_struct(vec![("s".to_string(), Value::Str(inner))]);
        heap.collect(&[Value::Struct(outer)]);

        assert_eq!(heap.get_string(inner), "nested");
    }

    #[test]
    fn collect_frees_a_struct_once_its_only_reference_is_gone() {
        let mut heap = Heap::new();
        heap.alloc_struct(vec![("x".to_string(), Value::Number(1))]);
        heap.collect(&[]);

        let reused = heap.alloc_struct(vec![("y".to_string(), Value::Number(2))]);
        assert!(reused.index() < 1, "expected the freed slot to be reused");
    }

    #[test]
    fn allocates_and_reads_a_closure_capture() {
        let mut heap = Heap::new();
        let handle = heap.alloc_closure(0x1234, vec![Value::Number(42)]);
        assert_eq!(heap.closure_function_ptr(handle), 0x1234);
        assert_eq!(heap.closure_get_capture(handle, 0), Value::Number(42));
    }

    #[test]
    fn closure_get_capture_out_of_range_returns_nil() {
        let mut heap = Heap::new();
        let handle = heap.alloc_closure(0x1234, vec![Value::Number(1)]);
        assert_eq!(heap.closure_get_capture(handle, 5), Value::Nil);
    }

    #[test]
    fn collect_traces_through_a_closure_capture() {
        let mut heap = Heap::new();
        let inner = heap.alloc_string("captured".to_string());
        let closure = heap.alloc_closure(0x1234, vec![Value::Str(inner)]);
        heap.collect(&[Value::Closure(closure)]);

        assert_eq!(heap.get_string(inner), "captured");
    }

    #[test]
    fn collect_frees_a_closure_once_its_only_reference_is_gone() {
        let mut heap = Heap::new();
        heap.alloc_closure(0x1234, vec![Value::Number(1)]);
        heap.collect(&[]);

        let reused = heap.alloc_closure(0x5678, vec![Value::Number(2)]);
        assert!(reused.index() < 1, "expected the freed slot to be reused");
    }

    #[test]
    fn should_collect_tracks_the_live_set_against_a_growing_threshold() {
        let mut heap = Heap::new();
        assert!(!heap.should_collect());

        for i in 0..INITIAL_GC_THRESHOLD {
            heap.alloc_string(format!("s{i}"));
        }
        assert!(heap.should_collect());

        heap.collect(&[]);
        assert!(
            !heap.should_collect(),
            "an empty heap should not be due again right after collecting"
        );
    }
}
