scanner: {
  name: TinyScheme,
  minimization: on,
  regular_expressions: [
    { name: Digit,      expression: ['0'-'9'] },
    { name: SymbolChar, expression: ['a'-'z', 'A'-'Z', '_', '+', '-', '*', '/', '%', '<', '>', '=', '!', '?', '&', '.'] },
    { name: WhiteSpace, expression: [' ', '\t', '\r', '\n'] }
  ],
  rules: [
    { token: LPAREN,  expression: '(' },
    { token: RPAREN,  expression: ')' },
    { token: TRUE,    expression: "#t" },
    { token: FALSE,   expression: "#f" },
    { token: NIL,     expression: "nil" },
    { token: INTEGER, expression: '-'? $Digit ($Digit)* },
    { token: STRING,  expression: '"' (~['"', '\\'] | '\\' .)* '"' },
    { token: SYMBOL,  expression: $SymbolChar ($SymbolChar | $Digit)* },
    { expression: ($WhiteSpace)+, skip: on },
    { expression: ';' ~['\r', '\n']*, skip: on }
  ],
  outputs: [
    {
      language: rust,
      external_token_type: "tiny_scheme_parser::TinySchemeTokenType"
    }
  ]
}
