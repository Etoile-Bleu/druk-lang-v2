//! A handle-based heap for Druk's two garbage-collected types, arrays and
//! strings. Objects are referenced by [`Handle`] (an index), never by a raw
//! pointer, so a Druk value can never forge or outlive a heap slot.
//!
//! Scope for this first version: a bump allocator with no collection at
//! all. Every `druk` invocation is a short-lived process that exits after
//! running one program, so leaking the heap for the process's lifetime is
//! observationally identical to collecting it — the OS reclaims everything
//! on exit. A real mark-and-sweep pass (tracing from the active locals of
//! every live call frame) is a later addition once a long-running program
//! actually needs bounded memory, not a default to build on day one.

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
/// one of its `alloc_*` methods — it is not a general way to fabricate a
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
    /// A raw native function pointer, cast back to a callable at the call
    /// site. Functions are static code, not heap objects, so this never
    /// needs a `Handle` or GC tracing.
    Function(usize),
}

#[derive(Debug, Clone)]
enum Object {
    Str(String),
    Array(Vec<Value>),
}

#[derive(Debug, Default)]
pub struct Heap {
    objects: Vec<Object>,
}

impl Heap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn alloc_string(&mut self, value: String) -> Handle {
        let handle = Handle(self.objects.len() as u32);
        self.objects.push(Object::Str(value));
        handle
    }

    pub fn alloc_array(&mut self, elements: Vec<Value>) -> Handle {
        let handle = Handle(self.objects.len() as u32);
        self.objects.push(Object::Array(elements));
        handle
    }

    pub fn get_string(&self, handle: Handle) -> &str {
        match &self.objects[handle.index()] {
            Object::Str(value) => value,
            Object::Array(_) => ice!("get_string called on a handle that holds an array"),
        }
    }

    pub fn get_array(&self, handle: Handle) -> &[Value] {
        match &self.objects[handle.index()] {
            Object::Array(elements) => elements,
            Object::Str(_) => ice!("get_array called on a handle that holds a string"),
        }
    }

    pub fn array_len(&self, handle: Handle) -> usize {
        self.get_array(handle).len()
    }

    pub fn array_get(&self, handle: Handle, index: usize) -> Value {
        self.get_array(handle)
            .get(index)
            .copied()
            .unwrap_or(Value::Nil)
    }

    pub fn array_set(&mut self, handle: Handle, index: usize, value: Value) {
        match &mut self.objects[handle.index()] {
            Object::Array(elements) => {
                if let Some(slot) = elements.get_mut(index) {
                    *slot = value;
                }
            }
            Object::Str(_) => ice!("array_set called on a handle that holds a string"),
        }
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
    fn out_of_bounds_array_get_returns_nil_instead_of_panicking() {
        let mut heap = Heap::new();
        let handle = heap.alloc_array(vec![Value::Number(1)]);
        assert_eq!(heap.array_get(handle, 5), Value::Nil);
    }
}
