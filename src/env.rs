use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;
use crate::value::Value;

/// Shared mutable environment. Clone is cheap (Rc refcount +1).
/// Java analogy: Env behaves like a normal Java object reference —
/// multiple variables can point to the same Env, and any of them
/// can call define() to mutate it, visible to all holders.
#[derive(Clone)]
pub struct Env {
    inner: Rc<RefCell<EnvInner>>,
}

struct EnvInner {
    bindings: HashMap<String, Value>,
    parent: Option<Env>,
}

impl Env {
    pub fn new() -> Self {
        Env {
            inner: Rc::new(RefCell::new(EnvInner {
                bindings: HashMap::new(),
                parent: None,
            })),
        }
    }

    /// Create a child scope whose parent is self.
    /// Like Java: new Env(this) where child.parent = this.
    pub fn child(&self) -> Self {
        Env {
            inner: Rc::new(RefCell::new(EnvInner {
                bindings: HashMap::new(),
                parent: Some(self.clone()),
            })),
        }
    }

    pub fn define(&self, name: String, value: Value) {
        self.inner.borrow_mut().bindings.insert(name, value);
    }

    pub fn lookup(&self, name: &str) -> Option<Value> {
        let inner = self.inner.borrow();
        if let Some(val) = inner.bindings.get(name) {
            Some(val.clone())
        } else if let Some(parent) = &inner.parent {
            parent.lookup(name)
        } else {
            None
        }
    }
}

impl fmt::Debug for Env {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#<env>")
    }
}
