#[derive(Debug, Clone)]
pub enum Expr {
    Int(i64),
    Bool(bool),
    Str(String),
    Symbol(String),
    List(Vec<Expr>),
    Nil,
}
