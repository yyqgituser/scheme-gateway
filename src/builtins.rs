use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::value::Value;

fn expect_int(val: &Value, op: &str) -> i64 {
    match val {
        Value::Int(n) => *n,
        _ => panic!("{}: expected integer, got {}", op, val),
    }
}

fn expect_str(val: &Value, op: &str) -> String {
    match val {
        Value::Str(s) => s.clone(),
        _ => panic!("{}: expected string, got {}", op, val),
    }
}

pub fn builtin_print(args: &[Value]) -> Value {
    if args.len() != 1 {
        panic!("print: expected 1 argument, got {}", args.len());
    }
    println!("{}", args[0]);
    Value::Nil
}

pub fn builtin_add(args: &[Value]) -> Value {
    Value::Int(expect_int(&args[0], "+") + expect_int(&args[1], "+"))
}

pub fn builtin_sub(args: &[Value]) -> Value {
    Value::Int(expect_int(&args[0], "-") - expect_int(&args[1], "-"))
}

pub fn builtin_mul(args: &[Value]) -> Value {
    Value::Int(expect_int(&args[0], "*") * expect_int(&args[1], "*"))
}

pub fn builtin_div(args: &[Value]) -> Value {
    let b = expect_int(&args[1], "/");
    if b == 0 { panic!("/: division by zero"); }
    Value::Int(expect_int(&args[0], "/") / b)
}

pub fn builtin_mod(args: &[Value]) -> Value {
    let b = expect_int(&args[1], "%");
    if b == 0 { panic!("%: division by zero"); }
    Value::Int(expect_int(&args[0], "%") % b)
}

pub fn builtin_eq(args: &[Value]) -> Value {
    Value::Bool(expect_int(&args[0], "=") == expect_int(&args[1], "="))
}

pub fn builtin_lt(args: &[Value]) -> Value {
    Value::Bool(expect_int(&args[0], "<") < expect_int(&args[1], "<"))
}

pub fn builtin_gt(args: &[Value]) -> Value {
    Value::Bool(expect_int(&args[0], ">") > expect_int(&args[1], ">"))
}

pub fn builtin_le(args: &[Value]) -> Value {
    Value::Bool(expect_int(&args[0], "<=") <= expect_int(&args[1], "<="))
}

pub fn builtin_ge(args: &[Value]) -> Value {
    Value::Bool(expect_int(&args[0], ">=") >= expect_int(&args[1], ">="))
}

pub fn builtin_not(args: &[Value]) -> Value {
    match &args[0] {
        Value::Bool(false) | Value::Nil => Value::Bool(true),
        _ => Value::Bool(false),
    }
}

pub fn builtin_string_length(args: &[Value]) -> Value {
    Value::Int(expect_str(&args[0], "string-length").len() as i64)
}

pub fn builtin_string_eq(args: &[Value]) -> Value {
    Value::Bool(expect_str(&args[0], "string-eq?") == expect_str(&args[1], "string-eq?"))
}

pub fn builtin_string_append(args: &[Value]) -> Value {
    let mut result = String::new();
    for arg in args {
        result.push_str(&expect_str(arg, "string-append"));
    }
    Value::Str(result)
}

pub fn builtin_substring(args: &[Value]) -> Value {
    let s = expect_str(&args[0], "substring");
    let start = expect_int(&args[1], "substring") as usize;
    let end = expect_int(&args[2], "substring") as usize;
    Value::Str(s[start..end].to_string())
}

pub fn builtin_starts_with(args: &[Value]) -> Value {
    let s = expect_str(&args[0], "starts-with?");
    let prefix = expect_str(&args[1], "starts-with?");
    Value::Bool(s.starts_with(&prefix))
}

pub fn builtin_ends_with(args: &[Value]) -> Value {
    let s = expect_str(&args[0], "ends-with?");
    let suffix = expect_str(&args[1], "ends-with?");
    Value::Bool(s.ends_with(&suffix))
}

pub fn builtin_contains(args: &[Value]) -> Value {
    let s = expect_str(&args[0], "contains?");
    let sub = expect_str(&args[1], "contains?");
    Value::Bool(s.contains(&sub))
}

pub fn builtin_split(args: &[Value]) -> Value {
    let s = expect_str(&args[0], "split");
    let delim = expect_str(&args[1], "split");
    let parts: Vec<Value> = s.split(&delim)
        .map(|p| Value::Str(p.to_string()))
        .collect();
    Value::List(parts)
}

pub fn builtin_list(args: &[Value]) -> Value {
    Value::List(args.to_vec())
}

pub fn builtin_car(args: &[Value]) -> Value {
    match &args[0] {
        Value::List(items) => {
            if items.is_empty() { panic!("car: empty list"); }
            items[0].clone()
        }
        _ => panic!("car: expected list, got {}", args[0]),
    }
}

pub fn builtin_cdr(args: &[Value]) -> Value {
    match &args[0] {
        Value::List(items) => {
            if items.is_empty() { panic!("cdr: empty list"); }
            Value::List(items[1..].to_vec())
        }
        _ => panic!("cdr: expected list, got {}", args[0]),
    }
}

pub fn builtin_cons(args: &[Value]) -> Value {
    match &args[1] {
        Value::List(items) => {
            let mut new_list = vec![args[0].clone()];
            new_list.extend(items.iter().cloned());
            Value::List(new_list)
        }
        _ => panic!("cons: second argument must be list, got {}", args[1]),
    }
}

pub fn builtin_null(args: &[Value]) -> Value {
    match &args[0] {
        Value::List(items) => Value::Bool(items.is_empty()),
        Value::Nil => Value::Bool(true),
        _ => Value::Bool(false),
    }
}

pub fn builtin_length(args: &[Value]) -> Value {
    match &args[0] {
        Value::List(items) => Value::Int(items.len() as i64),
        _ => panic!("length: expected list, got {}", args[0]),
    }
}

pub fn builtin_ip_address(args: &[Value]) -> Value {
    let s = expect_str(&args[0], "ip-address?");
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 4 {
        return Value::Bool(false);
    }
    for part in &parts {
        match part.parse::<u32>() {
            Ok(n) if n <= 255 => {}
            _ => return Value::Bool(false),
        }
    }
    Value::Bool(true)
}

pub fn builtin_table_get(args: &[Value]) -> Value {
    match &args[0] {
        Value::Table(map) => {
            let key = expect_str(&args[1], "table-get");
            map.borrow().get(&key).cloned().unwrap_or(Value::Nil)
        }
        _ => panic!("table-get: expected table, got {}", args[0]),
    }
}

pub fn builtin_table_set(args: &[Value]) -> Value {
    match &args[0] {
        Value::Table(map) => {
            let key = expect_str(&args[1], "table-set!");
            map.borrow_mut().insert(key, args[2].clone());
            Value::Nil
        }
        _ => panic!("table-set!: expected table, got {}", args[0]),
    }
}

pub fn builtin_table_has(args: &[Value]) -> Value {
    match &args[0] {
        Value::Table(map) => {
            let key = expect_str(&args[1], "table-has?");
            Value::Bool(map.borrow().contains_key(&key))
        }
        _ => panic!("table-has?: expected table, got {}", args[0]),
    }
}

pub fn builtin_table_keys(args: &[Value]) -> Value {
    match &args[0] {
        Value::Table(map) => {
            let keys: Vec<Value> = map.borrow().keys()
                .map(|k| Value::Str(k.clone()))
                .collect();
            Value::List(keys)
        }
        _ => panic!("table-keys: expected table, got {}", args[0]),
    }
}

pub fn builtin_respond(args: &[Value]) -> Value {
    let status = expect_int(&args[0], "respond");
    let body = expect_str(&args[1], "respond");
    let mut map = HashMap::new();
    map.insert("status".to_string(), Value::Int(status));
    map.insert("body".to_string(), Value::Str(body));
    Value::Table(Rc::new(RefCell::new(map)))
}

pub fn builtin_names() -> Vec<&'static str> {
    vec![
        "print",
        "+", "-", "*", "/", "%",
        "=", "<", ">", "<=", ">=",
        "not",
        "string-length", "string-eq?", "string-append", "substring",
        "starts-with?", "ends-with?", "contains?", "split",
        "list", "car", "cdr", "cons", "null?", "length",
        "ip-address?",
        "table-get", "table-set!", "table-has?", "table-keys",
        "respond",
    ]
}

pub fn call_builtin(name: &str, args: &[Value]) -> Value {
    match name {
        "print" => builtin_print(args),
        "+" => builtin_add(args),
        "-" => builtin_sub(args),
        "*" => builtin_mul(args),
        "/" => builtin_div(args),
        "%" => builtin_mod(args),
        "=" => builtin_eq(args),
        "<" => builtin_lt(args),
        ">" => builtin_gt(args),
        "<=" => builtin_le(args),
        ">=" => builtin_ge(args),
        "not" => builtin_not(args),
        "string-length" => builtin_string_length(args),
        "string-eq?" => builtin_string_eq(args),
        "string-append" => builtin_string_append(args),
        "substring" => builtin_substring(args),
        "starts-with?" => builtin_starts_with(args),
        "ends-with?" => builtin_ends_with(args),
        "contains?" => builtin_contains(args),
        "split" => builtin_split(args),
        "list" => builtin_list(args),
        "car" => builtin_car(args),
        "cdr" => builtin_cdr(args),
        "cons" => builtin_cons(args),
        "null?" => builtin_null(args),
        "length" => builtin_length(args),
        "ip-address?" => builtin_ip_address(args),
        "table-get" => builtin_table_get(args),
        "table-set!" => builtin_table_set(args),
        "table-has?" => builtin_table_has(args),
        "table-keys" => builtin_table_keys(args),
        "respond" => builtin_respond(args),
        _ => panic!("unknown builtin: {}", name),
    }
}
