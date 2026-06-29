use std::collections::HashMap;
use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use crate::ast::Expr;
use crate::value::Value;
use crate::env::Env;
use crate::builtins;

pub fn eval<'a>(expr: &'a Expr, env: &'a Env) -> Pin<Box<dyn Future<Output = Value> + 'a>> {
    Box::pin(async move {
        match expr {
            Expr::Int(n) => Value::Int(*n),
            Expr::Bool(b) => Value::Bool(*b),
            Expr::Str(s) => Value::Str(s.clone()),
            Expr::Nil => Value::Nil,
            Expr::Symbol(name) => {
                env.lookup(name)
                    .unwrap_or_else(|| panic!("undefined symbol: {}", name))
            }
            Expr::List(elems) => eval_list(elems, env).await,
        }
    })
}

async fn eval_list(elems: &[Expr], env: &Env) -> Value {
    if elems.is_empty() {
        return Value::Nil;
    }

    if let Expr::Symbol(name) = &elems[0] {
        match name.as_str() {
            "define" => return eval_define(elems, env).await,
            "if" => return eval_if(elems, env).await,
            "lambda" => return eval_lambda(elems, env),
            "let" => return eval_let(elems, env).await,
            "cond" => return eval_cond(elems, env).await,
            "begin" => return eval_begin(elems, env).await,
            "and" => return eval_and(elems, env).await,
            "or" => return eval_or(elems, env).await,
            "table" => return eval_table(elems, env).await,
            _ => {}
        }
    }

    let func_val = eval(&elems[0], env).await;
    let mut args = Vec::new();
    for e in &elems[1..] {
        args.push(eval(e, env).await);
    }
    call_func(func_val, &args).await
}

async fn eval_define(elems: &[Expr], env: &Env) -> Value {
    match &elems[1] {
        Expr::Symbol(name) => {
            let val = eval(&elems[2], env).await;
            env.define(name.clone(), val);
        }
        Expr::List(parts) => {
            let name = match &parts[0] {
                Expr::Symbol(s) => s.clone(),
                _ => panic!("define: expected function name"),
            };
            let params: Vec<String> = parts[1..]
                .iter()
                .map(|p| match p {
                    Expr::Symbol(s) => s.clone(),
                    _ => panic!("define: expected parameter name"),
                })
                .collect();
            let func = Value::Func {
                params,
                body: elems[2].clone(),
                env: env.clone(),
            };
            env.define(name, func);
        }
        _ => panic!("define: expected symbol or list"),
    }
    Value::Nil
}

async fn eval_if(elems: &[Expr], env: &Env) -> Value {
    let cond = eval(&elems[1], env).await;
    match cond {
        Value::Bool(false) | Value::Nil => eval(&elems[3], env).await,
        _ => eval(&elems[2], env).await,
    }
}

fn eval_lambda(elems: &[Expr], env: &Env) -> Value {
    let params = match &elems[1] {
        Expr::List(parts) => parts
            .iter()
            .map(|p| match p {
                Expr::Symbol(s) => s.clone(),
                _ => panic!("lambda: expected parameter name"),
            })
            .collect(),
        _ => panic!("lambda: expected parameter list"),
    };
    Value::Func {
        params,
        body: elems[2].clone(),
        env: env.clone(),
    }
}

pub async fn call_func(func: Value, args: &[Value]) -> Value {
    match func {
        Value::BuiltinFunc(name) => {
            match name.as_str() {
                "http-get" => call_http_get(args).await,
                _ => builtins::call_builtin(&name, args),
            }
        }
        Value::Func { params, body, env } => {
            let child = env.child();
            for (param, arg) in params.iter().zip(args.iter()) {
                child.define(param.clone(), arg.clone());
            }
            eval(&body, &child).await
        }
        _ => panic!("not callable: {}", func),
    }
}

async fn call_http_get(args: &[Value]) -> Value {
    // (http-get url headers-table)
    let url = match &args[0] {
        Value::Str(s) => s.clone(),
        _ => panic!("http-get: expected string url, got {}", args[0]),
    };

    let client = reqwest::Client::new();
    let mut request = client.get(&url);

    // Optional: second arg is a headers table
    if args.len() > 1 {
        if let Value::Table(map) = &args[1] {
            for (k, v) in map.borrow().iter() {
                if let Value::Str(val) = v {
                    request = request.header(k.as_str(), val.as_str());
                }
            }
        }
    }

    match request.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16() as i64;
            let body = resp.text().await.unwrap_or_default();
            let mut map = std::collections::HashMap::new();
            map.insert("status".to_string(), Value::Int(status));
            map.insert("body".to_string(), Value::Str(body));
            Value::Table(std::rc::Rc::new(std::cell::RefCell::new(map)))
        }
        Err(e) => {
            let mut map = std::collections::HashMap::new();
            map.insert("status".to_string(), Value::Int(0));
            map.insert("body".to_string(), Value::Str(format!("http-get error: {}", e)));
            Value::Table(std::rc::Rc::new(std::cell::RefCell::new(map)))
        }
    }
}

async fn eval_let(elems: &[Expr], env: &Env) -> Value {
    let bindings = match &elems[1] {
        Expr::List(pairs) => pairs,
        _ => panic!("let: expected bindings list"),
    };
    let child = env.child();
    for pair in bindings {
        match pair {
            Expr::List(kv) => {
                let name = match &kv[0] {
                    Expr::Symbol(s) => s.clone(),
                    _ => panic!("let: expected symbol in binding"),
                };
                let val = eval(&kv[1], env).await;
                child.define(name, val);
            }
            _ => panic!("let: expected (name value) pair"),
        }
    }
    eval(&elems[2], &child).await
}

async fn eval_cond(elems: &[Expr], env: &Env) -> Value {
    for clause in &elems[1..] {
        match clause {
            Expr::List(parts) => {
                if let Expr::Symbol(s) = &parts[0] {
                    if s == "else" {
                        return eval(&parts[1], env).await;
                    }
                }
                let test = eval(&parts[0], env).await;
                match test {
                    Value::Bool(false) | Value::Nil => continue,
                    _ => return eval(&parts[1], env).await,
                }
            }
            _ => panic!("cond: expected clause list"),
        }
    }
    Value::Nil
}

async fn eval_begin(elems: &[Expr], env: &Env) -> Value {
    let mut result = Value::Nil;
    for expr in &elems[1..] {
        result = eval(expr, env).await;
    }
    result
}

async fn eval_and(elems: &[Expr], env: &Env) -> Value {
    let mut result = Value::Bool(true);
    for expr in &elems[1..] {
        result = eval(expr, env).await;
        match &result {
            Value::Bool(false) | Value::Nil => return result,
            _ => {}
        }
    }
    result
}

async fn eval_or(elems: &[Expr], env: &Env) -> Value {
    let mut result = Value::Bool(false);
    for expr in &elems[1..] {
        result = eval(expr, env).await;
        match &result {
            Value::Bool(false) | Value::Nil => {}
            _ => return result,
        }
    }
    result
}

async fn eval_table(elems: &[Expr], env: &Env) -> Value {
    let mut map = HashMap::new();
    for pair_expr in &elems[1..] {
        match pair_expr {
            Expr::List(kv) => {
                if kv.len() != 2 {
                    panic!("table: each pair must have exactly 2 elements");
                }
                let key = match eval(&kv[0], env).await {
                    Value::Str(s) => s,
                    other => panic!("table: key must be string, got {}", other),
                };
                let val = eval(&kv[1], env).await;
                map.insert(key, val);
            }
            _ => panic!("table: expected (key value) pair"),
        }
    }
    Value::Table(Rc::new(RefCell::new(map)))
}

pub async fn eval_program(exprs: &[Expr], env: &Env) -> Value {
    let mut result = Value::Nil;
    for expr in exprs {
        result = eval(expr, env).await;
    }
    result
}

pub fn default_env() -> Env {
    let env = Env::new();
    for name in builtins::builtin_names() {
        env.define(name.to_string(), Value::BuiltinFunc(name.to_string()));
    }
    env.define("http-get".to_string(), Value::BuiltinFunc("http-get".to_string()));
    env
}
