#[path = "../generated/tiny_scheme_parser.rs"]
mod tiny_scheme_parser;
#[path = "../generated/tiny_scheme_scanner.rs"]
mod tiny_scheme_scanner;
mod ast;
mod ast_builder;
mod value;
mod env;
mod builtins;
mod evaluator;
mod runtime;
mod codegen;
mod server;

use std::env as std_env;
use std::fs;

struct ScannerAdapter(tiny_scheme_scanner::TinySchemeScanner);

impl tiny_scheme_parser::TinySchemeScanner for ScannerAdapter {
    fn next(&mut self) -> Result<i32, Box<dyn std::error::Error>> {
        match self.0.next() {
            Ok(tok) => Ok(tok as i32),
            Err(e) => Err(format!(
                "Lexical error at {}:{}: {}",
                e.line, e.column, e.message
            )
            .into()),
        }
    }
    fn text(&self) -> String {
        self.0.text()
    }
    fn line(&self) -> i32 {
        self.0.line()
    }
    fn column(&self) -> i32 {
        self.0.column()
    }
}

fn parse_source(source: &str) -> Vec<ast::Expr> {
    let scanner = tiny_scheme_scanner::TinySchemeScanner::new(source);
    let token_scanner = tiny_scheme_parser::TinySchemeTokenScanner::new(ScannerAdapter(scanner));
    let mut parser = tiny_scheme_parser::TinySchemeParser::new(token_scanner);
    let root = parser.parse().expect("parse error");
    ast_builder::AstBuilder::new().build(&root)
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args: Vec<String> = std_env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: scheme-gateway <file.scm> [--jit | --serve [--port PORT]]");
        std::process::exit(1);
    }

    let jit_mode = args.iter().any(|a| a == "--jit");
    let serve_mode = args.iter().any(|a| a == "--serve");

    let source = fs::read_to_string(&args[1]).expect("cannot read source file");
    let exprs = parse_source(&source);

    if serve_mode {
        let port = args.iter()
            .position(|a| a == "--port")
            .and_then(|i| args.get(i + 1))
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(8080);
        server::serve(&exprs, port).await;
    } else if jit_mode {
        let context = inkwell::context::Context::create();
        let codegen = codegen::Codegen::new(&context);
        codegen.compile_and_run(&exprs);
    } else {
        let env = evaluator::default_env();
        evaluator::eval_program(&exprs, &env).await;
    }
}
