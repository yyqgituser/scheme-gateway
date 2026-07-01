# scheme-gateway

A lightweight API gateway built in Rust, with a Scheme-based DSL for defining plugin logic. Supports both a tree-walking interpreter and an LLVM JIT compiler backend.

## What is this?

Modern API gateways use scripting languages (Lua) to let users define custom logic — authentication, rate limiting, request routing, etc. **scheme-gateway** explores what this looks like when built from scratch in Rust, using a purpose-built DSL instead of a general-purpose scripting language.

## Auth Plugin Demo

This demonstrates the core use case: **an API gateway that authenticates requests by calling a remote auth server**.

### Setup: Three Processes

```
┌──────────────────────────────────────────────────────────────┐
│ Terminal 1: Mock Auth Server (port 9000)                     │
│                                                              │
│ $ scheme-gateway examples/mock_auth.scm --serve --port 9000  │
│                                                              │
│ Validates API keys. Returns 200 for "valid-key", 403 else.  │
│ Logic defined in DSL.                                        │
└──────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────┐
│ Terminal 2: API Gateway (port 8080)                          │
│                                                              │
│ $ scheme-gateway examples/auth_plugin.scm --serve --port 8080│
│                                                              │
│ Receives client requests, extracts auth header, calls        │
│ auth server, allows or rejects. Logic defined in DSL.        │
└──────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────┐
│ Terminal 3: Client                                           │
│                                                              │
│ $ curl http://localhost:8080/api/users                        │
│ → 401 Missing auth header                                    │
│                                                              │
│ $ curl -H "X-Api-Key: valid-key" localhost:8080/api/users    │
│ → 200 OK: /api/users authorized                              │
│                                                              │
│ $ curl -H "X-Api-Key: bad-key" localhost:8080/api/users      │
│ → 403 Forbidden: auth server rejected                        │
└──────────────────────────────────────────────────────────────┘
```

### Async Request Flow

```
Client                    Gateway (8080)                         Auth Server (9000)
  │                           │                                        │
  │  GET /api/users           │                                        │
  │  X-Api-Key: valid-key     │                                        │
  │──────────────────────────→│                                        │
  │                           │                                        │
  │                    ┌──────┴──────────────────────┐                 │
  │                    │  Rust: tokio HTTP server    │                 │
  │                    │  parse HTTP, build DSL table│                 │
  │                    └──────┬──────────────────────┘                 │
  │                           │                                        │
  │                    ┌──────┴──────────────────────┐                  │
  │                    │  DSL: (on-request req)       │                  │
  │                    │                              │                  │
  │                    │  1. extract header value      │                 │
  │                    │     (table-get headers        │                 │
  │                    │       "x-api-key")            │                 │
  │                    │     → "valid-key"             │                 │
  │                    │                              │                  │
  │                    │  2. call auth server (async)  │                 │
  │                    │     (http-get auth-url        │  GET /auth      │
  │                    │       (table                  │  x-api-key:     │
  │                    │         ("x-api-key"          │    valid-key    │
  │                    │           "valid-key")))      │────────────────→│
  │                    │                              │                  │
  │                    │                              │         ┌────────┴──────────┐
  │                    │                              │         │ DSL: (on-request) │
  │                    │                              │         │ check key value   │
  │                    │                              │         │ → (respond 200    │
  │                    │                              │         │     "authenticated│
  │                    │                              │         └────────┬──────────┘
  │                    │                              │                  │
  │                    │                              │  HTTP 200        │
  │                    │     ← response table         │←─────────────────│
  │                    │     {"status": 200,           │                 │
  │                    │      "body": "authenticated"} │                 │
  │                    │                               │                 │
  │                    │  3. check status              │                 │
  │                    │     (= status 200) → #t       │                 │
  │                    │                               │                 │
  │                    │  4. return response           │                 │
  │                    │     (respond 200              │                 │
  │                    │       "OK: /api/users         │                 │
  │                    │         authorized")          │                 │
  │                    └──────┬──────────────────────┘                   │
  │                           │                                          │
  │  HTTP 200                 │                                          │
  │  OK: /api/users authorized│                                          │
  │←──────────────────────────│                                          │
  │                           │                                          │
```

## Why Rust + DSL

Traditional API gateways often embed a scripting language (typically Lua) inside a C-based HTTP server (Nginx). This is proven at scale but has structural limitations:

| | Rust + DSL | Nginx + Lua |
|---|---|---|
| **Host language** | Rust — compile-time memory safety | C — manual memory management |
| **GC pauses** | None (Rust ownership) | Lua GC, incremental but present |
| **Async model** | tokio async/await, native to language | C-level coroutine yield, invisible to script |
| **Deployment** | Single static binary | Nginx + scripting runtime + library ecosystem |
| **Language evolution** | Self-owned DSL, evolves with project needs | Tied to third-party scripting language release cycle |
| **JIT backend** | LLVM — industrial optimizer, actively maintained | Scripting language JIT, limited evolution |
| **Plugin isolation** | DSL has no capabilities unless explicitly granted | Script can access filesystem, network, OS by default |
| **Ecosystem** | DSL builtins backed by Rust crates (tokio, reqwest, rustls, ...) | Dedicated scripting libraries, limited reuse outside gateway |

**Trade-offs**: This is a proof-of-concept. Traditional Nginx + Lua stacks are battle-tested at scale with rich ecosystems. This project demonstrates the architectural direction, not production readiness.

### What Rust Does vs What DSL Does

```
┌─────────────────────────────────────────────────────────────────┐
│                                                                 │
│  Rust (src/server.rs + tokio)              DSL (plugin.scm)     │
│  ─────────────────────────                 ──────────────────   │
│  TCP listen/accept                         Plugin config         │
│  HTTP parsing                              Auth logic            │
│  Build request table                       Call remote service   │
│  Async I/O (tokio + reqwest)               Route decisions       │
│  Send HTTP response                        Response construction │
│                                                                 │
│  "How to move bytes"                       "What to do with     │
│                                              the request"       │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Plugin Source Code

**`auth_plugin.scm`** — Gateway plugin (port 8080):

```scheme
;; Plugin configuration
(define config (table
  ("auth-server-url" "http://localhost:9000/auth")
  ("auth-header-name" "x-api-key")))

;; Called for every incoming HTTP request
(define (on-request req)
  (let ((header-name  (table-get config "auth-header-name"))
        (auth-url     (table-get config "auth-server-url"))
        (headers      (table-get req "headers"))
        (path         (table-get req "path")))
    (let ((header-value (table-get headers header-name)))
      (if (not header-value)
          (respond 401 "Missing auth header")
          (let ((auth-resp (http-get auth-url (table (header-name header-value)))))
            (if (= (table-get auth-resp "status") 200)
                (respond 200 (string-append "OK: " path " authorized"))
                (respond 403 "Forbidden: auth server rejected")))))))
```

**`mock_auth.scm`** — Auth server (port 9000):

```scheme
;; Returns 200 if x-api-key is "valid-key", 403 otherwise
(define (on-request req)
  (let ((headers (table-get req "headers")))
    (let ((key (table-get headers "x-api-key")))
      (if (not key)
          (respond 403 "missing key")
          (if (string-eq? key "valid-key")
              (respond 200 "authenticated")
              (respond 403 "invalid key"))))))
```

**Both servers are running scheme-gateway** — the same binary, same DSL, different plugin scripts. The auth server itself is also defined in DSL, demonstrating that the gateway is general enough to serve any HTTP logic.

## Pipeline

```
                         ┌─────────────────────────────────────────────────┐
                         │              scheme-gateway                     │
                         └─────────────────────────────────────────────────┘

  source.scm             ┌───────────┐  tokens   ┌───────────┐  parse tree
 ─────────────────────→  │   Lexer   │ ────────→  │  Parser   │ ────→
                         │  (DFA)    │            │  (LR(1))  │     │
                         └───────────┘            └───────────┘     │
                          generated/                generated/      │
                          tiny_scheme_              tiny_scheme_    │
                          scanner.rs                parser.rs       │
                                                                    │
                                                                    │
                                                          ┌─────────┴─────────┐
                                                          │    AST Builder    │
                                                          │ (Visitor pattern) │
                                                          └─────────┬─────────┘
                                                                    │
                                                              Expr enum
                                                                    │
                                ┌────────────────────────────┼──────────────────────┐
                                │                            │                      │
                          --serve mode                 default mode            --jit mode
                                │                            │                      │
                   ┌────────────┴────────────┐   ┌───────────┴──────────┐  ┌────────┴────────┐
                   │  tokio HTTP server      │   │   Tree-walking      │  │  LLVM CodeGen   │
                   │                         │   │   Evaluator         │  │                 │
                   │  per request:           │   │                     │  │  Tagged Value   │
                   │  parse HTTP → DSL table │   │  • Rc<RefCell> env  │  │  {i8, i64}     │
                   │  call (on-request req)  │   │  • full language    │  │  int/bool      │
                   │  DSL table → HTTP resp  │   │    support          │  │  subset        │
                   │                         │   └───────────┬──────────┘  └────────┬────────┘
                   │  async builtins:        │               │                      │
                   │  • http-get (reqwest)   │               │               ┌──────┴──────┐
                   │  • respond              │               │               │  LLVM ORC   │
                   └────────────┬────────────┘               │               │  JIT Engine │
                                │                            │               └──────┬──────┘
                                └────────────────┬───────────┴──────────────────────┘
                                                 │
                                              result
```

## Project Structure

```
scheme-gateway/
├── grammar/
│   ├── TinyScheme.ls           # Scanner grammar
│   └── TinyScheme.lrp          # Parser grammar
├── generated/
│   ├── tiny_scheme_scanner.rs  # Generated DFA scanner (checked in)
│   └── tiny_scheme_parser.rs   # Generated LR(1) parser (checked in)
├── src/
│   ├── main.rs                 # CLI: --serve / --jit / interpreter
│   ├── ast.rs                  # Expr enum
│   ├── ast_builder.rs          # Parse tree → AST (visitor)
│   ├── value.rs                # Runtime Value (Int/Bool/Str/Table/Func/...)
│   ├── env.rs                  # Lexical scope chain (Rc<RefCell>)
│   ├── evaluator.rs            # Async tree-walking evaluator
│   ├── builtins.rs             # Built-in functions
│   ├── server.rs               # tokio HTTP server + request dispatch
│   ├── codegen.rs              # LLVM IR code generation
│   └── runtime.rs              # JIT runtime bridge (extern "C")
├── examples/
│   ├── auth_plugin.scm         # Auth gateway plugin
│   ├── mock_auth.scm           # Mock auth server
│   ├── gateway_test.scm        # Static routing demo
│   └── ...                     # Language feature demos
└── Cargo.toml
```

## Gateway Plugin Model

A plugin is a `.scm` file that defines an `on-request` function. The gateway calls this function for every incoming HTTP request.

### Request Table

The `on-request` function receives a table with:

| Key | Type | Example |
|-----|------|---------|
| `"method"` | String | `"GET"` |
| `"path"` | String | `"/api/users"` |
| `"query"` | String | `"id=42"` |
| `"remote-addr"` | String | `"127.0.0.1"` |
| `"headers"` | Table | `{"host": "localhost", "x-api-key": "xxx"}` |

### Response

Return a response using `(respond status body)`:

```scheme
(respond 200 "OK")
(respond 403 "Forbidden")
```

### Async HTTP Client

Call external services from within a plugin:

```scheme
(define resp (http-get "http://auth-server/verify"
               (table ("x-api-key" api-key))))
(table-get resp "status")  ;; → 200
(table-get resp "body")    ;; → response body string
```

## DSL Reference

### Data Types

| Type | Examples |
|------|----------|
| Integer | `42`, `-7` |
| Boolean | `#t`, `#f` |
| String | `"hello"` |
| Table | `(table ("key" value) ...)` |
| List | `(list 1 2 3)` |
| Function | `(lambda (x) (* x x))` |
| Nil | `nil` |

### Special Forms

`define`, `lambda`, `if`, `cond`, `let`, `begin`, `and`, `or`, `table`

### Built-in Functions

| Category | Functions |
|----------|-----------|
| Arithmetic | `+`, `-`, `*`, `/`, `%` |
| Comparison | `=`, `<`, `>`, `<=`, `>=` |
| Logical | `not` |
| String | `string-length`, `string-eq?`, `string-append`, `substring`, `starts-with?`, `ends-with?`, `contains?`, `split` |
| Table | `table-get`, `table-set!`, `table-has?`, `table-keys` |
| List | `list`, `car`, `cdr`, `cons`, `null?`, `length` |
| Gateway | `respond`, `http-get` (async) |
| Network | `ip-address?` |
| IO | `print` |

## Lexer & Parser

The lexer and parser (`generated/tiny_scheme_scanner.rs` and `generated/tiny_scheme_parser.rs`) are generated by external tools and checked into the repository as zero-dependency Rust source.

The lexer is a DFA-based scanner that tokenizes input in O(n) time — one state transition per character, no backtracking. The parser is an LR(1) bottom-up parser that builds a concrete parse tree. The grammar is intentionally minimal — S-expressions need almost no syntactic complexity. All semantic structure is handled in the evaluator, not the parser.

## JIT Compiler

The `--jit` flag compiles DSL code to native machine code via LLVM's ORC JIT engine, using Tagged Value representation `{i8 tag, i64 payload}` — the same approach used by Lua/LuaJIT internally. Currently supports integer/boolean subset.

| Feature | Interpreter | JIT |
|---------|-------------|-----|
| Integer / Boolean / Nil | ✅ | ✅ |
| define / if / cond / let / begin | ✅ | ✅ |
| and / or / not | ✅ | ✅ |
| Arithmetic & comparison | ✅ | ✅ |
| Recursion | ✅ | ✅ |
| print | ✅ | ✅ |
| String / Table / List | ✅ | — |
| Lambda / closures | ✅ | — |
| http-get / respond | ✅ | — |

## Limitations

The JIT compiler supports pure computation only — integer arithmetic, comparisons, conditionals, and recursion are compiled to native machine code via LLVM. Functions that involve async I/O (e.g., `http-get`) run in the async tree-walking interpreter. The two coexist at the function boundary: an interpreter-driven `on-request` handler can call JIT-compiled pure functions, but async and JIT cannot be mixed within a single function. Compiling async operations to native code would require generating state machines at the machine code level — the same problem LuaJIT solves by falling back to its interpreter when a trace hits a coroutine yield.

## Environment Setup

### Rust

Install Rust 1.70+ via [rustup](https://rustup.rs/).

### LLVM 18 (required for JIT mode)

**Prerequisites**: CMake, Ninja, and a C/C++ toolchain — GCC 13+ on Linux, or Visual Studio 2022+ on Windows (use the **x64 Native Tools Command Prompt**).

Set the environment variable before building:

```bash
export LLVM_SYS_181_PREFIX=<LLVM18_INSTALL_DIR>  # Linux/macOS
set LLVM_SYS_181_PREFIX=<LLVM18_INSTALL_DIR>     # Windows
```

## Prior Art

**tiny** — A mini-C language compiler implemented in C++ with self-implemented lexer/parser generators, LLVM backend, custom linker and loader. 
