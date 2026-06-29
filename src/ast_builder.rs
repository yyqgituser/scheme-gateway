use crate::tiny_scheme_parser::*;
use crate::ast::Expr;

pub struct AstBuilder {
    stack: Vec<Expr>,
}

impl AstBuilder {
    pub fn new() -> Self {
        AstBuilder { stack: Vec::new() }
    }

    /// Consume the builder, walk the parse tree, return top-level expressions.
    pub fn build(mut self, root: &TinySchemeSyntaxNode) -> Vec<Expr> {
        root.accept(&mut self);
        self.stack
    }

    fn terminal_text(node: &TinySchemeSyntaxNode) -> String {
        match node {
            TinySchemeSyntaxNode::Terminal(t) => t.token.text.clone(),
            _ => panic!("expected terminal node"),
        }
    }

    fn unescape(s: &str) -> String {
        let mut result = String::new();
        let mut chars = s.chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next() {
                    Some('n') => result.push('\n'),
                    Some('t') => result.push('\t'),
                    Some('\\') => result.push('\\'),
                    Some('"') => result.push('"'),
                    Some(other) => {
                        result.push('\\');
                        result.push(other);
                    }
                    None => result.push('\\'),
                }
            } else {
                result.push(c);
            }
        }
        result
    }
}

impl TinySchemeSyntaxNodeVisitor for AstBuilder {
    fn handle_terminal(&mut self, _node: &TinySchemeTerminalNode) {
        // Terminals are read directly by atom visitors via terminal_text()
    }

    fn visit_ProgramExprs(&mut self, node: &TinySchemeNonterminalNode) {
        // rhs: [ExprList]
        node.rhs[0].accept(self);
    }

    fn visit_ExprListAppend(&mut self, node: &TinySchemeNonterminalNode) {
        // rhs: [ExprList, Expr]
        node.rhs[0].accept(self);
        node.rhs[1].accept(self);
    }

    fn visit_ExprListEmpty(&mut self, _node: &TinySchemeNonterminalNode) {
        // rhs: [] — nothing to push
    }

    fn visit_ExprAtom(&mut self, node: &TinySchemeNonterminalNode) {
        // rhs: [Atom]
        node.rhs[0].accept(self);
    }

    fn visit_ExprList_(&mut self, node: &TinySchemeNonterminalNode) {
        // rhs: [LPAREN, Contents, RPAREN]
        // Mark stack position, visit contents, drain into a List
        let mark = self.stack.len();
        node.rhs[1].accept(self);
        let elements: Vec<Expr> = self.stack.drain(mark..).collect();
        self.stack.push(Expr::List(elements));
    }

    fn visit_ContentsAppend(&mut self, node: &TinySchemeNonterminalNode) {
        // rhs: [Contents, Expr]
        node.rhs[0].accept(self);
        node.rhs[1].accept(self);
    }

    fn visit_ContentsEmpty(&mut self, _node: &TinySchemeNonterminalNode) {
        // rhs: [] — nothing to push
    }

    fn visit_AtomInt(&mut self, node: &TinySchemeNonterminalNode) {
        // rhs: [INTEGER]
        let text = Self::terminal_text(&node.rhs[0]);
        let n: i64 = text.parse().expect("invalid integer literal");
        self.stack.push(Expr::Int(n));
    }

    fn visit_AtomStr(&mut self, node: &TinySchemeNonterminalNode) {
        // rhs: [STRING]
        let text = Self::terminal_text(&node.rhs[0]);
        let inner = &text[1..text.len() - 1];
        self.stack.push(Expr::Str(Self::unescape(inner)));
    }

    fn visit_AtomSym(&mut self, node: &TinySchemeNonterminalNode) {
        // rhs: [SYMBOL]
        let text = Self::terminal_text(&node.rhs[0]);
        self.stack.push(Expr::Symbol(text));
    }

    fn visit_AtomTrue(&mut self, _node: &TinySchemeNonterminalNode) {
        self.stack.push(Expr::Bool(true));
    }

    fn visit_AtomFalse(&mut self, _node: &TinySchemeNonterminalNode) {
        self.stack.push(Expr::Bool(false));
    }

    fn visit_AtomNil(&mut self, _node: &TinySchemeNonterminalNode) {
        self.stack.push(Expr::Nil);
    }
}
