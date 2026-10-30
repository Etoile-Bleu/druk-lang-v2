use druk_diagnostics::ice;

use crate::{Handle, Heap, Object};

impl Heap {
    pub fn alloc_string(&mut self, value: String) -> Handle {
        self.alloc(Object::Str(value))
    }

    pub fn get_string(&self, handle: Handle) -> &str {
        match self.object(handle) {
            Object::Str(value) => value,
            Object::Array(_) => ice!("get_string called on a handle that holds an array"),
            Object::Struct(_) => ice!("get_string called on a handle that holds a struct"),
            Object::Closure { .. } => ice!("get_string called on a handle that holds a closure"),
        }
    }
}
