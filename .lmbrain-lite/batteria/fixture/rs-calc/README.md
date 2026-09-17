# calc

A tiny arithmetic evaluator: `calc::eval("2 * (3 + 4)") == Ok(14.0)`.

Grammar, from lowest to highest precedence:

| operators | associativity |
|---|---|
| `+` `-` (binary) | left |
| `*` `/` | left |
| `-` (unary) | prefix |
| `^` (power) | right |

So `2^3^2` is `2^(3^2)` = 512, `-2^2` is `-(2^2)` = -4, and `2^-1` is 0.5.
Numbers are decimal literals such as `3`, `3.25` or `.5`.

Layout: `lexer.rs` turns text into tokens, `parser.rs` builds an `Expr` (a Pratt parser
driven by binding powers), `eval.rs` computes it.
