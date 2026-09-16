# Formula Grammar

This document provides the formal grammar and parsing rules for Finx formula expressions.

## 1. Formal EBNF Grammar

```ebnf
Expression         = BinaryExpr | UnaryExpr | FunctionCall | Primary ;
BinaryExpr         = Expression BinaryOp Expression ;
UnaryExpr          = UnaryOp Expression ;
FunctionCall       = FunctionName "(" [ ArgumentList ] ")" ;
ArgumentList       = Argument { "," Argument } ;
Argument           = Expression ;

Primary            = Number | String | Reference | NullLiteral | "(" Expression ")" ;

Reference          = TableRef ;
TableRef           = Identifier ( DotAccess | BracketAccess ) ;

DotAccess          = "." Identifier "[" ColSpec "]" ;
BracketAccess      = "[" RowSpec "," ColSpec "]" ;

RowSpec            = Key [ ".." Key ]
                   | SpecialRow ;

ColSpec            = "@"
                   | Key ".." Key
                   | Key
                   | SpecialCol ;

SpecialRow         = "rows." Identifier
                   | "rows." Identifier ".last"
                   | "rows." Identifier ".first" ;

SpecialCol         = "col.prev" [ "(" Integer ")" ]
                   | "col.next" [ "(" Integer ")" ]
                   | "cols." Identifier
                   | "cols." Identifier ".last"
                   | "cols." Identifier ".first" ;

Key                = String ;
BinaryOp           = "+" | "-" | "*" | "/" | "^" | "=" | "<>" | "<" | "<=" | ">" | ">=" ;
UnaryOp            = "+" | "-" ;
FunctionName       = [A-Z][A-Z0-9_]* ;
Identifier         = [a-zA-Z_][a-zA-Z0-9_]* ;
String             = '"' [^"\\]* '"' ;
Integer            = [0-9]+ ;
Number             = [0-9]+ ( "." [0-9]+ )? ;
NullLiteral        = "null" ;
```

---

## 2. Operator Precedence & Associativity

Formulas are parsed using a Pratt parser (`winnow`) with standard algebraic precedence from highest to lowest:

| Precedence | Operator | Description | Associativity |
| :--- | :--- | :--- | :--- |
| **5** | `^` | Exponentiation | Right |
| **4** | `+`, `-` (unary) | Unary positive and negative | Right |
| **3** | `*`, `/` | Multiplication and division | Left |
| **2** | `+`, `-` (binary) | Addition and subtraction | Left |
| **1** | `=`, `<>`, `<`, `<=`, `>`, `>=` | Relational comparisons | Left |

Parentheses `( ... )` override default operator precedence.

---

## 3. Lexical Rules

- **Keywords and functions**: Built-in Excel functions must be uppercase (`SUM`, `AVERAGE`, `IF`, `NPV`, `XIRR`, `MAX`, `MIN`, `XLOOKUP`). Supported functions and signatures are cataloged in the [Standard Function Catalog](../functions/catalog.md).
- **Identifiers**: Identifiers must match `[a-zA-Z_][a-zA-Z0-9_]*` (used for table and row dot-access).
- **Coordinate keys**: All row and column keys inside bracket selectors must be double-quoted strings (`Key = String`). Bare identifiers are prohibited as coordinate keys.
- **String literals**: Double-quoted strings with standard escaping (`"value"`, `"2024E"`).
- **Whitespace**: Whitespace between tokens is ignored during parsing.

See [Addressing Syntax](addressing.md) for reference coordinates, [Standard Function Catalog](../functions/catalog.md) for supported functions, and [Specification Format](spec-format.md) for embedding expressions within JSON table specifications.
