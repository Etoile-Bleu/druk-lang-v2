# Druk Language: Complete Dzongkha Reference

> **Druk** (འབྲུག་) is a programming language whose keywords are entirely written in **Dzongkha**, the national language of Bhutan.  
> It uses the Tibetan script (Uchen), which is shared by both languages.

---

## 1. Implemented Keywords

### Declarations

| Dzongkha Keyword | Romanization | Meaning | Equivalent |
|---|---|---|---|
| `ལས་འགན་` | *las 'gan* | "task, duty" | `function` |
| `གྲངས་` | *grangs* | "number, digit" | `number` (int) |
| `ཡིག་འབྲུ་` | *yig 'bru* | "character, letter" | `string` |
| `བདེན་རྫུན་` | *bden rdzun* | "true-false" | `boolean` |

### Literal Values

| Dzongkha Keyword | Romanization | Meaning | Equivalent |
|---|---|---|---|
| `བདེན་པ་` | *bden pa* | "truth" | `true` |
| `རྫུན་མ་` | *rdzun ma* | "falsehood" | `false` |

### Control Flow

| Dzongkha Keyword | Romanization | Meaning | Equivalent |
|---|---|---|---|
| `གལ་སྲིད་` | *gal srid* | "if possible, in case" | `if` |
| `མེད་ན་` | *med na* | "otherwise, if not" | `else` |
| `ཡང་བསྐྱར་` | *yang bskyar* | "again, once more" | `while` |
| `རེ་རེར་` | *re rer* | "one by one, each" | `for` |
| `ནང་` | *nang* | "inside, within" | `in` (in `for ... in`) |
| `ཆད་` | *chad* | "to interrupt, stop" | `break` |
| `མུ་མཐུད་` | *mu mthud* | "to continue" | `continue` |
| `སླར་ལོག་` | *slar log* | "to return, come back" | `return` |

### Input / Output

| Dzongkha Keyword | Romanization | Meaning | Equivalent |
|---|---|---|---|
| `བཀོད་` | *bkod* | "to display, write" | `print` |

---

## 2. Built-in Functions (Builtins)

These functions are called like regular identifiers.

| Identifier | Meaning | Equivalent |
|---|---|---|
| `རིང་ཚད` | *ring tshad*: "length, size" | `len()` |
| `སྣོན་` | *snon*: "to add" | `push()` |
| `བཏོན་` | *bton*: "to extract, remove" | `pop()` |
| `རིགས་` | *rigs*: "type, category" | `typeof()` |
| `ལྡེ་མིག་` | *lde mig*: "keys" | `keys()` (not yet implemented) |
| `གནས་གོང་` | *gnas gong*: "values" | `values()` (not yet implemented) |
| `ནང་འདུས་` | *nang 'dus*: "contains" | `contains()` (not yet implemented) |
| `ནང་འཇུག་` | *nang 'jug*: "input" | `input()` |

The last three functions depend on a dictionary type that the compiler does not yet support. Calling them produces a clear compilation error: `druk check` rejects them directly, rather than allowing them to cause a silent failure or runtime crash.

---

## 3. Predefined Global Variables

| Dzongkha Variable | Equivalent | Description |
|---|---|---|
| `ནང་འཇུག་ཐོ་` | `argv` | Command-line arguments (array) |
| `ནང་འཇུག་གྲངས་` | `argc` | Number of command-line arguments |

---

## 4. Operators

All operators use standard ASCII symbols.

| Operator | Description |
|---|---|
| `+` `-` `*` `/` `%` | Integer arithmetic (`%` is remainder) |
| `==` `!=` | Equality |
| `<` `<=` `>` `>=` | Comparison |
| `&&` `||` | Logical AND / OR |
| `!` | Logical NOT |
| `=` | Assignment |
| `.` | Member access (struct) |
| `[]` | Array indexing |

---

## 5. Data Types

| Type | Keyword | Description | Example |
|---|---|---|---|
| Integer | `གྲངས་` | Signed 64-bit integer | `༡༢༣` |
| String | `ཡིག་འབྲུ་` | UTF-8 string | `"བཀྲ་ཤིས་"` |
| Boolean | `བདེན་རྫུན་` | True / false | `བདེན་པ་` |
| Array | *(literal)* | Dynamic, garbage-collected | `[༡, ༢, ༣]` |
| Struct | *(literal)* | Named fields, garbage-collected | `{key: value}` |
| Nil | `ཅི་མེད` | Null value | N/A |

---

## 6. Tibetan / Dzongkha Numerals

Numeric literals use Tibetan digits (U+0F20–U+0F29).

| Digit | Value |
|---|---|
| `༠` | 0 |
| `༡` | 1 |
| `༢` | 2 |
| `༣` | 3 |
| `༤` | 4 |
| `༥` | 5 |
| `༦` | 6 |
| `༧` | 7 |
| `༨` | 8 |
| `༩` | 9 |

Multi-digit numbers are written normally: `༡༢༣` = 123, `༡༠༠༠` = 1000.

---

## 7. Complete Syntax: Examples

### Variable Declaration

```druk
གྲངས་ ཨང་ = ༡༠;
ཡིག་འབྲུ་ མིང་ = "བཀྲ་ཤིས་";
བདེན་རྫུན་ ཡིན་མིན་ = བདེན་པ་;
```

### Conditional

```druk
གལ་སྲིད་ (ཨང་ > ༠) {
    བཀོད་ "ཡར་རྒྱས་";
} མེད་ན་ {
    བཀོད་ "མར་འབབ་";
}
```

### While Loop

```druk
གྲངས་ ཨང་ = ༠;
ཡང་བསྐྱར་ (ཨང་ < ༥) {
    བཀོད་ ཨང་;
    ཨང་ = ཨང་ + ༡;
}
```

### For Loop

```druk
རེ་རེར་ (གྲངས་ ཨང་ = ༠; ཨང་ < ༥; ཨང་ = ཨང་ + ༡) {
    བཀོད་ ཨང་;
}
```

### For-Each Loop

`རེ་རེར་` also accepts a second form: a variable, `ནང་` ("in"), then an array. It iterates every
element without managing an index by hand.

```druk
གྲངས་[] numbers = [༡, ༢, ༣];
རེ་རེར་ (n ནང་ numbers) {
    བཀོད་ n;
}
```

### Break and Continue

```druk
གྲངས་ ཨང་ = ༠;
ཡང་བསྐྱར་ (བདེན་པ་) {
    ཨང་ = ཨང་ + ༡;
    གལ་སྲིད་ (ཨང་ == ༣) {
        མུ་མཐུད་;
    }
    གལ་སྲིད་ (ཨང་ > ༥) {
        ཆད་;
    }
    བཀོད་ ཨང་;
}
```

### Function

```druk
ལས་འགན་ བསྡོམས་ (གྲངས་ ཀ་, གྲངས་ ཁ་) {
    སླར་ལོག་ ཀ་ + ཁ་;
}
བཀོད་ བསྡོམས་(༣, ༤);
```

The type keyword for a parameter is optional: `druk-hir` infers the type from how the parameter is used in the function body (an addition implies a number, an `if` condition implies a boolean, etc.).

```druk
ལས་འགན་ བསྡོམས་ (ཀ་, ཁ་) {
    སླར་ལོག་ ཀ་ + ཁ་;
}
```

If usage does not allow the type to be inferred unambiguously (for example, a parameter is never used or is used in contradictory ways), the compiler asks the developer to explicitly add the type keyword to that specific parameter, rather than failing with a generic error.

### Array

```druk
གྲངས་ ཐོ་ཡིག་ = [༡, ༢, ༣, ༤, ༥];
བཀོད་ རིང་ཚད(ཐོ་ཡིག་);
སྣོན་(ཐོ་ཡིག་, ༦);
```

### Struct

```druk
གྲངས་ མི་ = {མིང་: "རྡོ་རྗེ་", ལོ་: ༢༥};
བཀོད་ མི་.མིང་;
```

---

## 8. Future Keywords (Not Yet Implemented)

These terms are reserved for future language extensions.

| Concept | Proposed Dzongkha | Romanization | Equivalent |
|---|---|---|---|
| `import` | `ནང་འདྲེན་` | *nang 'dren* | Import a module |
| `export` | `ཕྱིར་འདྲེན་` | *phyir 'dren* | Export a symbol |
| `class` / `struct type` | `རིགས་གཞི་` | *rigs gzhi* | Define a type |
| `null` / `nil` | `ཅི་མེད` | *ci med* | Explicit null value |
| `try` | `ཚོད་ལྟ་` | *tshod lta* | Try (error handling) |
| `catch` | `འཛིན་` | *'dzin* | Catch an error |
| `throw` | `འཕེན་` | *'phen* | Throw an error |
| `async` | `རིམ་མིན་` | *rim min* | Asynchronous |
| `await` | `སྒུག་` | *sgug* | Wait |
| `match` / `switch` | `འགྲིག་པ་` | *'grig pa* | Pattern matching |
| `enum` | `གྲངས་རིམ་` | *grangs rim* | Enumeration |
| `const` | `མི་འགྱུར་` | *mi 'gyur* | Constant |
| `type` (alias) | `རིགས་མིང་` | *rigs ming* | Type alias |

---

## 9. Separators and Punctuation

| Symbol | Usage |
|---|---|
| `;` | End of statement |
| `{ }` | Code block |
| `( )` | Condition, parameters |
| `[ ]` | Array literal, indexing |
| `,` | Element separator |
| `:` | Key-value separator (struct) |
| `.` | Member access |
| `//` | Line comment (continues to the end of the line) |
| `/* */` | Multi-line comment (can contain Dzongkha text across multiple lines) |

Comments are completely ignored by the compiler, whether they span one line or multiple lines, including any Dzongkha text they contain.

```druk
གྲངས་ ཨང་ = ༡༠; // starting number
/* this is a
   multi-line comment */
བཀོད་ ཨང་;
```

---

## 10. Encoding and Files

- **File extension**: `.druk`
- **Encoding**: UTF-8 (optional BOM)
- **Script**: Tibetan Uchen (Unicode block U+0F00–U+0FFF)
- **Digits**: Tibetan digits (0–9, U+0F20–U+0F29) or ASCII digits (0–9)
