use druk_diagnostics::ice;

use crate::{Handle, Heap, Object, Value};

impl Heap {
    pub fn alloc_struct(&mut self, fields: Vec<(String, Value)>) -> Handle {
        self.alloc(Object::Struct(fields))
    }

    /// Returns the value of `field` on the struct at `handle`, or `Nil` if
    /// the struct has no field by that name, the same not-found-is-nil
    /// convention `array_get` uses for an out-of-bounds index.
    pub fn struct_get(&self, handle: Handle, field: &str) -> Value {
        match self.object(handle) {
            Object::Struct(fields) => fields
                .iter()
                .find(|(name, _)| name == field)
                .map(|(_, value)| *value)
                .unwrap_or(Value::Nil),
            Object::Str(_) => ice!("struct_get called on a handle that holds a string"),
            Object::Array(_) => ice!("struct_get called on a handle that holds an array"),
            Object::Closure { .. } => ice!("struct_get called on a handle that holds a closure"),
        }
    }

    /// Overwrites `field` on the struct at `handle`. A struct's fields are
    /// fixed at construction (its literal), so setting a field that doesn't
    /// exist is a no-op, the same convention `array_set` uses for an
    /// out-of-bounds index, rather than growing the struct dynamically.
    pub fn struct_set(&mut self, handle: Handle, field: &str, value: Value) {
        match self.object_mut(handle) {
            Object::Struct(fields) => {
                if let Some(slot) = fields.iter_mut().find(|(name, _)| name == field) {
                    slot.1 = value;
                }
            }
            Object::Str(_) => ice!("struct_set called on a handle that holds a string"),
            Object::Array(_) => ice!("struct_set called on a handle that holds an array"),
            Object::Closure { .. } => ice!("struct_set called on a handle that holds a closure"),
        }
    }
}
