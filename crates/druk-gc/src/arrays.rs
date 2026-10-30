use druk_diagnostics::ice;

use crate::{Handle, Heap, Object, Value};

impl Heap {
    pub fn alloc_array(&mut self, elements: Vec<Value>) -> Handle {
        self.alloc(Object::Array(elements))
    }

    pub fn get_array(&self, handle: Handle) -> &[Value] {
        match self.object(handle) {
            Object::Array(elements) => elements,
            Object::Str(_) => ice!("get_array called on a handle that holds a string"),
            Object::Struct(_) => ice!("get_array called on a handle that holds a struct"),
            Object::Closure { .. } => ice!("get_array called on a handle that holds a closure"),
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
        match self.object_mut(handle) {
            Object::Array(elements) => {
                if let Some(slot) = elements.get_mut(index) {
                    *slot = value;
                }
            }
            Object::Str(_) => ice!("array_set called on a handle that holds a string"),
            Object::Struct(_) => ice!("array_set called on a handle that holds a struct"),
            Object::Closure { .. } => ice!("array_set called on a handle that holds a closure"),
        }
    }

    /// Appends `value` to the end of the array at `handle`.
    pub fn array_push(&mut self, handle: Handle, value: Value) {
        match self.object_mut(handle) {
            Object::Array(elements) => elements.push(value),
            Object::Str(_) => ice!("array_push called on a handle that holds a string"),
            Object::Struct(_) => ice!("array_push called on a handle that holds a struct"),
            Object::Closure { .. } => ice!("array_push called on a handle that holds a closure"),
        }
    }

    /// Removes and returns the last element of the array at `handle`, or
    /// `Nil` if the array is empty, the same not-found-is-nil convention
    /// `array_get` uses for an out-of-bounds index.
    pub fn array_pop(&mut self, handle: Handle) -> Value {
        match self.object_mut(handle) {
            Object::Array(elements) => elements.pop().unwrap_or(Value::Nil),
            Object::Str(_) => ice!("array_pop called on a handle that holds a string"),
            Object::Struct(_) => ice!("array_pop called on a handle that holds a struct"),
            Object::Closure { .. } => ice!("array_pop called on a handle that holds a closure"),
        }
    }
}
