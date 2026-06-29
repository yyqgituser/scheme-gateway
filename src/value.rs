use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;
use crate::ast::Expr;
use crate::env::Env;

#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Bool(bool),
    Str(String),
    List(Vec<Value>),
    Table(Rc<RefCell<HashMap<String, Value>>>),
    Func {
        params: Vec<String>,
        body: Expr,
        env: Env,
    },
    BuiltinFunc(String),
    Nil,
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{}", n),
            Value::Bool(true) => write!(f, "#t"),
            Value::Bool(false) => write!(f, "#f"),
            Value::Str(s) => write!(f, "{}", s),
            Value::List(items) => {
                write!(f, "(")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 { write!(f, " ")?; }
                    write!(f, "{}", item)?;
                }
                write!(f, ")")
            }
            Value::Table(map) => {
                let map = map.borrow();
                write!(f, "{{")?;
                for (i, (k, v)) in map.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "\"{}\": {}", k, v)?;
                }
                write!(f, "}}")
            }
            Value::Func { .. } => write!(f, "#<function>"),
            Value::BuiltinFunc(name) => write!(f, "#<builtin:{}>", name),
            Value::Nil => write!(f, "nil"),
        }
    }
}
