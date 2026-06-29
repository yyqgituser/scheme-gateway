use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::ast::Expr;
use crate::env::Env;
use crate::evaluator;
use crate::value::Value;

pub async fn serve(exprs: &[Expr], port: u16) {
    let env = evaluator::default_env();

    // Execute top-level definitions (define on-request, define config, etc.)
    evaluator::eval_program(exprs, &env).await;

    // Verify that on-request is defined
    if env.lookup("on-request").is_none() {
        eprintln!("Error: plugin must define (on-request req) function");
        std::process::exit(1);
    }

    let listener = TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .expect("failed to bind port");
    println!("tiny-scheme gateway listening on port {}", port);

    loop {
        let (mut stream, addr) = match listener.accept().await {
            Ok(v) => v,
            Err(e) => {
                eprintln!("accept error: {}", e);
                continue;
            }
        };

        let mut buf = vec![0u8; 8192];
        let n = match stream.read(&mut buf).await {
            Ok(0) => continue,
            Ok(n) => n,
            Err(_) => continue,
        };
        let raw = String::from_utf8_lossy(&buf[..n]);

        let req_table = parse_http_request(&raw, &addr.ip().to_string());

        let func = env.lookup("on-request").unwrap();
        let result = evaluator::call_func(func, &[req_table]).await;

        let (status, body) = extract_response(&result);
        let response = format!(
            "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n{}",
            status,
            status_text(status),
            body.len(),
            body
        );
        let _ = stream.write_all(response.as_bytes()).await;
    }
}

fn parse_http_request(raw: &str, remote_addr: &str) -> Value {
    let mut lines = raw.lines();

    // Parse request line: "GET /path HTTP/1.1"
    let request_line = lines.next().unwrap_or("");
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    let method = parts.first().copied().unwrap_or("GET");
    let full_path = parts.get(1).copied().unwrap_or("/");

    // Split path and query string
    let (path, query) = match full_path.split_once('?') {
        Some((p, q)) => (p, q),
        None => (full_path, ""),
    };

    // Parse headers
    let mut headers_map: HashMap<String, Value> = HashMap::new();
    for line in lines {
        if line.is_empty() {
            break;
        }
        if let Some((key, value)) = line.split_once(':') {
            headers_map.insert(
                key.trim().to_lowercase(),
                Value::Str(value.trim().to_string()),
            );
        }
    }

    // Build request table
    let mut req_map: HashMap<String, Value> = HashMap::new();
    req_map.insert("method".to_string(), Value::Str(method.to_string()));
    req_map.insert("path".to_string(), Value::Str(path.to_string()));
    req_map.insert("query".to_string(), Value::Str(query.to_string()));
    req_map.insert("remote-addr".to_string(), Value::Str(remote_addr.to_string()));
    req_map.insert(
        "headers".to_string(),
        Value::Table(Rc::new(RefCell::new(headers_map))),
    );

    Value::Table(Rc::new(RefCell::new(req_map)))
}

fn extract_response(result: &Value) -> (i64, String) {
    match result {
        Value::Table(map) => {
            let map = map.borrow();
            let status = match map.get("status") {
                Some(Value::Int(n)) => *n,
                _ => 500,
            };
            let body = match map.get("body") {
                Some(Value::Str(s)) => s.clone(),
                Some(other) => format!("{}", other),
                None => String::new(),
            };
            (status, body)
        }
        _ => (500, "Internal Server Error: on-request must return (respond status body)".to_string()),
    }
}

fn status_text(code: i64) -> &'static str {
    match code {
        200 => "OK",
        201 => "Created",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        _ => "Unknown",
    }
}
