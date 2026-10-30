use druk_diagnostics::ice;

use crate::{Handle, Heap, Object, Value};

impl Heap {
    pub fn alloc_closure(&mut self, function_ptr: usize, captures: Vec<Value>) -> Handle {
        self.alloc(Object::Closure {
            function_ptr,
            captures,
        })
    }

    /// The raw function pointer of the closure at `handle`, to be cast back
    /// to a callable at the codegen call site.
    pub fn closure_function_ptr(&self, handle: Handle) -> usize {
        match self.object(handle) {
            Object::Closure { function_ptr, .. } => *function_ptr,
            Object::Str(_) => ice!("closure_function_ptr called on a handle that holds a string"),
            Object::Array(_) => ice!("closure_function_ptr called on a handle that holds an array"),
            Object::Struct(_) => {
                ice!("closure_function_ptr called on a handle that holds a struct")
            }
        }
    }

    /// Returns the value of capture `index` on the closure at `handle`, or
    /// `Nil` if out of range, the same not-found-is-nil convention
    /// `array_get` uses for an out-of-bounds index.
    pub fn closure_get_capture(&self, handle: Handle, index: usize) -> Value {
        match self.object(handle) {
            Object::Closure { captures, .. } => captures.get(index).copied().unwrap_or(Value::Nil),
            Object::Str(_) => ice!("closure_get_capture called on a handle that holds a string"),
            Object::Array(_) => ice!("closure_get_capture called on a handle that holds an array"),
            Object::Struct(_) => ice!("closure_get_capture called on a handle that holds a struct"),
        }
    }
}
