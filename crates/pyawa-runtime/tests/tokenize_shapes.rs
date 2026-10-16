//! **`_tokenize.TokenizerIter`——逐 token 与参照对齐**（task-19 ✓）。
//!
//! 参照值由本机 `python3`（3.14）跑**同一段探针**取得 ✓（脚本见各 `const` ✓）。
//!
//! **一处真相** ✓：词法扫描就是编译器的词法器（`pyawa_core::python_tokens` ✓），
//! `_tokenize` 只做参照投影 ✓。被这组抽例钉住的真 bug：
//! ① `str.splitlines(True)` **没读 `keepends`** ✗（还给 `['a','b']` ✗ ⇒ readline 拼不出源码 ✓）；
//! ② 续行的 `NL` 底层不发 ✓（要按"隐式续行"补 ✓）；
//! ③ 多行 token 的 `line` 字段＝**这个 token 自己吃掉的物理行** ✓；
//! ④ `DEDENT` 的**位置＝触发行的缩进宽度** ✓、`line` ＝那一行 ✓（不是恒 `(row,0)` ＋空串 ✗）；
//! ⑤ 行尾 `COMMENT` 的文本**不含 `\n`** ✓；
//! ⑥ **f-string 整段按 `STRING` 出** ✗ ⇒ 已按参照拆 `FSTRING_START`／`MIDDLE`／`END` ＋ 字段 token ✓
//!    （`{{`／`}}` 只算一个花括号且**断开 `MIDDLE` 段** ✓；格式说明收尾**必发**（可为空）`MIDDLE` ✓）；
//! ⑦ `repr` **不转义不可打印字符** ✗（`\x00` 等原样吐 ✓）⇒ 按参照的 `\xNN`／`\uNNNN`／`\U…` 收口 ✓；
//! ⑧ `len(str)` 数的是 **UTF-8 字节** ✗（`len('中')` 给 3 ✓、参照 1 ✓）⇒ 改**字符数** ✓。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-tokenize-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
        .current_dir(&root)
        .output()
        .expect("跑 CLI");
    let _ = std::fs::remove_dir_all(&root);
    assert!(
        output.status.success(),
        "不许中止（退出状态 {:?}）\n--- stdout ---\n{}\n--- stderr ---\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// 隐式续行的 `NL` ＋ 五元组／`ENDMARKER` ✓
const CONTINUATION_SCRIPT: &str = r#"import tokenize
print("tokenize ok", tokenize.__name__)

def lines():
    yield "x = 1\n"
    yield "y = x + 2\n"
    yield "z = (1,\n"
    yield "     2)\n"

toks = list(tokenize.generate_tokens(lines().__next__))
for tok in toks:
    print(tokenize.tok_name[tok.type], repr(tok.string), tok.start, tok.end, repr(tok.line))
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）：见 `CONTINUATION_SCRIPT` ✓。
const CONTINUATION_EXPECTED: &str = "tokenize ok tokenize\nNAME 'x' (1, 0) (1, 1) 'x = 1\\n'\nOP '=' (1, 2) (1, 3) 'x = 1\\n'\nNUMBER '1' (1, 4) (1, 5) 'x = 1\\n'\nNEWLINE '\\n' (1, 5) (1, 6) 'x = 1\\n'\nNAME 'y' (2, 0) (2, 1) 'y = x + 2\\n'\nOP '=' (2, 2) (2, 3) 'y = x + 2\\n'\nNAME 'x' (2, 4) (2, 5) 'y = x + 2\\n'\nOP '+' (2, 6) (2, 7) 'y = x + 2\\n'\nNUMBER '2' (2, 8) (2, 9) 'y = x + 2\\n'\nNEWLINE '\\n' (2, 9) (2, 10) 'y = x + 2\\n'\nNAME 'z' (3, 0) (3, 1) 'z = (1,\\n'\nOP '=' (3, 2) (3, 3) 'z = (1,\\n'\nOP '(' (3, 4) (3, 5) 'z = (1,\\n'\nNUMBER '1' (3, 5) (3, 6) 'z = (1,\\n'\nOP ',' (3, 6) (3, 7) 'z = (1,\\n'\nNL '\\n' (3, 7) (3, 8) 'z = (1,\\n'\nNUMBER '2' (4, 5) (4, 6) '     2)\\n'\nOP ')' (4, 6) (4, 7) '     2)\\n'\nNEWLINE '\\n' (4, 7) (4, 8) '     2)\\n'\nENDMARKER '' (5, 0) (5, 0) ''\n";

/// 运算符／关键字／行尾注释／`DEDENT` 位置与 `line`／全角字面量 ✓
const OPERATORS_SCRIPT: &str = r#"import tokenize

SRC = "class C:\n    def m(self, x=1):  # trailing\n        if x >= 2 and x != 3:\n            return x ** 2 // 3\n        return -x\n\nobj = C()\n全角 = obj.m(4)\nlast = 'it\\'s ok'\n"

def lines():
    for piece in SRC.split("\n"):
        yield piece + "\n"

for tok in tokenize.generate_tokens(lines().__next__):
    print(tokenize.tok_name[tok.type], repr(tok.string), tok.start, tok.end, repr(tok.line))
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）：见 `OPERATORS_SCRIPT` ✓。
const OPERATORS_EXPECTED: &str = "NAME 'class' (1, 0) (1, 5) 'class C:\\n'\nNAME 'C' (1, 6) (1, 7) 'class C:\\n'\nOP ':' (1, 7) (1, 8) 'class C:\\n'\nNEWLINE '\\n' (1, 8) (1, 9) 'class C:\\n'\nINDENT '    ' (2, 0) (2, 4) '    def m(self, x=1):  # trailing\\n'\nNAME 'def' (2, 4) (2, 7) '    def m(self, x=1):  # trailing\\n'\nNAME 'm' (2, 8) (2, 9) '    def m(self, x=1):  # trailing\\n'\nOP '(' (2, 9) (2, 10) '    def m(self, x=1):  # trailing\\n'\nNAME 'self' (2, 10) (2, 14) '    def m(self, x=1):  # trailing\\n'\nOP ',' (2, 14) (2, 15) '    def m(self, x=1):  # trailing\\n'\nNAME 'x' (2, 16) (2, 17) '    def m(self, x=1):  # trailing\\n'\nOP '=' (2, 17) (2, 18) '    def m(self, x=1):  # trailing\\n'\nNUMBER '1' (2, 18) (2, 19) '    def m(self, x=1):  # trailing\\n'\nOP ')' (2, 19) (2, 20) '    def m(self, x=1):  # trailing\\n'\nOP ':' (2, 20) (2, 21) '    def m(self, x=1):  # trailing\\n'\nCOMMENT '# trailing' (2, 23) (2, 33) '    def m(self, x=1):  # trailing\\n'\nNEWLINE '\\n' (2, 33) (2, 34) '    def m(self, x=1):  # trailing\\n'\nINDENT '        ' (3, 0) (3, 8) '        if x >= 2 and x != 3:\\n'\nNAME 'if' (3, 8) (3, 10) '        if x >= 2 and x != 3:\\n'\nNAME 'x' (3, 11) (3, 12) '        if x >= 2 and x != 3:\\n'\nOP '>=' (3, 13) (3, 15) '        if x >= 2 and x != 3:\\n'\nNUMBER '2' (3, 16) (3, 17) '        if x >= 2 and x != 3:\\n'\nNAME 'and' (3, 18) (3, 21) '        if x >= 2 and x != 3:\\n'\nNAME 'x' (3, 22) (3, 23) '        if x >= 2 and x != 3:\\n'\nOP '!=' (3, 24) (3, 26) '        if x >= 2 and x != 3:\\n'\nNUMBER '3' (3, 27) (3, 28) '        if x >= 2 and x != 3:\\n'\nOP ':' (3, 28) (3, 29) '        if x >= 2 and x != 3:\\n'\nNEWLINE '\\n' (3, 29) (3, 30) '        if x >= 2 and x != 3:\\n'\nINDENT '            ' (4, 0) (4, 12) '            return x ** 2 // 3\\n'\nNAME 'return' (4, 12) (4, 18) '            return x ** 2 // 3\\n'\nNAME 'x' (4, 19) (4, 20) '            return x ** 2 // 3\\n'\nOP '**' (4, 21) (4, 23) '            return x ** 2 // 3\\n'\nNUMBER '2' (4, 24) (4, 25) '            return x ** 2 // 3\\n'\nOP '//' (4, 26) (4, 28) '            return x ** 2 // 3\\n'\nNUMBER '3' (4, 29) (4, 30) '            return x ** 2 // 3\\n'\nNEWLINE '\\n' (4, 30) (4, 31) '            return x ** 2 // 3\\n'\nDEDENT '' (5, 8) (5, 8) '        return -x\\n'\nNAME 'return' (5, 8) (5, 14) '        return -x\\n'\nOP '-' (5, 15) (5, 16) '        return -x\\n'\nNAME 'x' (5, 16) (5, 17) '        return -x\\n'\nNEWLINE '\\n' (5, 17) (5, 18) '        return -x\\n'\nNL '\\n' (6, 0) (6, 1) '\\n'\nDEDENT '' (7, 0) (7, 0) 'obj = C()\\n'\nDEDENT '' (7, 0) (7, 0) 'obj = C()\\n'\nNAME 'obj' (7, 0) (7, 3) 'obj = C()\\n'\nOP '=' (7, 4) (7, 5) 'obj = C()\\n'\nNAME 'C' (7, 6) (7, 7) 'obj = C()\\n'\nOP '(' (7, 7) (7, 8) 'obj = C()\\n'\nOP ')' (7, 8) (7, 9) 'obj = C()\\n'\nNEWLINE '\\n' (7, 9) (7, 10) 'obj = C()\\n'\nNAME '全角' (8, 0) (8, 2) '全角 = obj.m(4)\\n'\nOP '=' (8, 3) (8, 4) '全角 = obj.m(4)\\n'\nNAME 'obj' (8, 5) (8, 8) '全角 = obj.m(4)\\n'\nOP '.' (8, 8) (8, 9) '全角 = obj.m(4)\\n'\nNAME 'm' (8, 9) (8, 10) '全角 = obj.m(4)\\n'\nOP '(' (8, 10) (8, 11) '全角 = obj.m(4)\\n'\nNUMBER '4' (8, 11) (8, 12) '全角 = obj.m(4)\\n'\nOP ')' (8, 12) (8, 13) '全角 = obj.m(4)\\n'\nNEWLINE '\\n' (8, 13) (8, 14) '全角 = obj.m(4)\\n'\nNAME 'last' (9, 0) (9, 4) \"last = 'it\\\\'s ok'\\n\"\nOP '=' (9, 5) (9, 6) \"last = 'it\\\\'s ok'\\n\"\nSTRING \"'it\\\\'s ok'\" (9, 7) (9, 17) \"last = 'it\\\\'s ok'\\n\"\nNEWLINE '\\n' (9, 17) (9, 18) \"last = 'it\\\\'s ok'\\n\"\nNL '\\n' (10, 0) (10, 1) '\\n'\nENDMARKER '' (11, 0) (11, 0) ''\n";

/// 注释／空行／缩进块／多行字符串（`line` 字段）／全角字面量 ✓
const COMMENTED_SCRIPT: &str = r#"import tokenize

SRC = 'def add(a, b):\n    # comment\n    total = a + b\n\n    text = """multi\nline"""\n    full = "全角：αβ"\n    return total, text, name, full\n'

def lines():
    for piece in SRC.split("\n"):
        yield piece + "\n"

for tok in tokenize.generate_tokens(lines().__next__):
    print(tokenize.tok_name[tok.type], repr(tok.string), tok.start, tok.end, repr(tok.line))
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）：见 `COMMENTED_SCRIPT` ✓。
const COMMENTED_EXPECTED: &str = "NAME 'def' (1, 0) (1, 3) 'def add(a, b):\\n'\nNAME 'add' (1, 4) (1, 7) 'def add(a, b):\\n'\nOP '(' (1, 7) (1, 8) 'def add(a, b):\\n'\nNAME 'a' (1, 8) (1, 9) 'def add(a, b):\\n'\nOP ',' (1, 9) (1, 10) 'def add(a, b):\\n'\nNAME 'b' (1, 11) (1, 12) 'def add(a, b):\\n'\nOP ')' (1, 12) (1, 13) 'def add(a, b):\\n'\nOP ':' (1, 13) (1, 14) 'def add(a, b):\\n'\nNEWLINE '\\n' (1, 14) (1, 15) 'def add(a, b):\\n'\nCOMMENT '# comment' (2, 4) (2, 13) '    # comment\\n'\nNL '\\n' (2, 13) (2, 14) '    # comment\\n'\nINDENT '    ' (3, 0) (3, 4) '    total = a + b\\n'\nNAME 'total' (3, 4) (3, 9) '    total = a + b\\n'\nOP '=' (3, 10) (3, 11) '    total = a + b\\n'\nNAME 'a' (3, 12) (3, 13) '    total = a + b\\n'\nOP '+' (3, 14) (3, 15) '    total = a + b\\n'\nNAME 'b' (3, 16) (3, 17) '    total = a + b\\n'\nNEWLINE '\\n' (3, 17) (3, 18) '    total = a + b\\n'\nNL '\\n' (4, 0) (4, 1) '\\n'\nNAME 'text' (5, 4) (5, 8) '    text = \"\"\"multi\\n'\nOP '=' (5, 9) (5, 10) '    text = \"\"\"multi\\n'\nSTRING '\"\"\"multi\\nline\"\"\"' (5, 11) (6, 7) '    text = \"\"\"multi\\nline\"\"\"\\n'\nNEWLINE '\\n' (6, 7) (6, 8) 'line\"\"\"\\n'\nNAME 'full' (7, 4) (7, 8) '    full = \"全角：αβ\"\\n'\nOP '=' (7, 9) (7, 10) '    full = \"全角：αβ\"\\n'\nSTRING '\"全角：αβ\"' (7, 11) (7, 18) '    full = \"全角：αβ\"\\n'\nNEWLINE '\\n' (7, 18) (7, 19) '    full = \"全角：αβ\"\\n'\nNAME 'return' (8, 4) (8, 10) '    return total, text, name, full\\n'\nNAME 'total' (8, 11) (8, 16) '    return total, text, name, full\\n'\nOP ',' (8, 16) (8, 17) '    return total, text, name, full\\n'\nNAME 'text' (8, 18) (8, 22) '    return total, text, name, full\\n'\nOP ',' (8, 22) (8, 23) '    return total, text, name, full\\n'\nNAME 'name' (8, 24) (8, 28) '    return total, text, name, full\\n'\nOP ',' (8, 28) (8, 29) '    return total, text, name, full\\n'\nNAME 'full' (8, 30) (8, 34) '    return total, text, name, full\\n'\nNEWLINE '\\n' (8, 34) (8, 35) '    return total, text, name, full\\n'\nNL '\\n' (9, 0) (9, 1) '\\n'\nDEDENT '' (10, 0) (10, 0) ''\nENDMARKER '' (10, 0) (10, 0) ''\n";

/// f-string 三段拆分：`!r`／`:` 格式说明／嵌套 `{}`／`{{` 转义 ✓
const FSTRINGS_SCRIPT: &str = r#"import tokenize

CASES = [
    'a = f"x={total}"\n',
    'b = f"{x!r}"\n',
    'c = f"{x:>5}"\n',
    'd = f"{x:{w}}"\n',
    'e = f"{{literal}}"\n',
    'g = f"{x=}"\n',
    'h = f"{x + 1}"\n',
    'i = rf"\\d{x}"\n',
    'j = f"a{x}b{y}c"\n',
    'k = f"{{x}}"\n',
    'l = f"a{{b}}c"\n',
    'm = f"{{{x}}}"\n',
    'n = f"}}"\n',
    'o = f"{x:>{w}}"\n',
    'p = f"{x!r:>{w}}"\n',
    'q = f"{ {1:2} }"\n',
]

def lines(src):
    def reader():
        for piece in src.split("\n"):
            yield piece + "\n"
    return reader

for src in CASES:
    toks = list(tokenize.generate_tokens(lines(src)().__next__))
    for tok in toks:
        if tok.type not in (tokenize.ENDMARKER, tokenize.NEWLINE):
            print(src.strip(), "|", tokenize.tok_name[tok.type], repr(tok.string), tok.start, tok.end)
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）：见 `FSTRINGS_SCRIPT` ✓。
const FSTRINGS_EXPECTED: &str = "a = f\"x={total}\" | NAME 'a' (1, 0) (1, 1)\na = f\"x={total}\" | OP '=' (1, 2) (1, 3)\na = f\"x={total}\" | FSTRING_START 'f\"' (1, 4) (1, 6)\na = f\"x={total}\" | FSTRING_MIDDLE 'x=' (1, 6) (1, 8)\na = f\"x={total}\" | OP '{' (1, 8) (1, 9)\na = f\"x={total}\" | NAME 'total' (1, 9) (1, 14)\na = f\"x={total}\" | OP '}' (1, 14) (1, 15)\na = f\"x={total}\" | FSTRING_END '\"' (1, 15) (1, 16)\na = f\"x={total}\" | NL '\\n' (2, 0) (2, 1)\nb = f\"{x!r}\" | NAME 'b' (1, 0) (1, 1)\nb = f\"{x!r}\" | OP '=' (1, 2) (1, 3)\nb = f\"{x!r}\" | FSTRING_START 'f\"' (1, 4) (1, 6)\nb = f\"{x!r}\" | OP '{' (1, 6) (1, 7)\nb = f\"{x!r}\" | NAME 'x' (1, 7) (1, 8)\nb = f\"{x!r}\" | OP '!' (1, 8) (1, 9)\nb = f\"{x!r}\" | NAME 'r' (1, 9) (1, 10)\nb = f\"{x!r}\" | OP '}' (1, 10) (1, 11)\nb = f\"{x!r}\" | FSTRING_END '\"' (1, 11) (1, 12)\nb = f\"{x!r}\" | NL '\\n' (2, 0) (2, 1)\nc = f\"{x:>5}\" | NAME 'c' (1, 0) (1, 1)\nc = f\"{x:>5}\" | OP '=' (1, 2) (1, 3)\nc = f\"{x:>5}\" | FSTRING_START 'f\"' (1, 4) (1, 6)\nc = f\"{x:>5}\" | OP '{' (1, 6) (1, 7)\nc = f\"{x:>5}\" | NAME 'x' (1, 7) (1, 8)\nc = f\"{x:>5}\" | OP ':' (1, 8) (1, 9)\nc = f\"{x:>5}\" | FSTRING_MIDDLE '>5' (1, 9) (1, 11)\nc = f\"{x:>5}\" | OP '}' (1, 11) (1, 12)\nc = f\"{x:>5}\" | FSTRING_END '\"' (1, 12) (1, 13)\nc = f\"{x:>5}\" | NL '\\n' (2, 0) (2, 1)\nd = f\"{x:{w}}\" | NAME 'd' (1, 0) (1, 1)\nd = f\"{x:{w}}\" | OP '=' (1, 2) (1, 3)\nd = f\"{x:{w}}\" | FSTRING_START 'f\"' (1, 4) (1, 6)\nd = f\"{x:{w}}\" | OP '{' (1, 6) (1, 7)\nd = f\"{x:{w}}\" | NAME 'x' (1, 7) (1, 8)\nd = f\"{x:{w}}\" | OP ':' (1, 8) (1, 9)\nd = f\"{x:{w}}\" | OP '{' (1, 9) (1, 10)\nd = f\"{x:{w}}\" | NAME 'w' (1, 10) (1, 11)\nd = f\"{x:{w}}\" | OP '}' (1, 11) (1, 12)\nd = f\"{x:{w}}\" | FSTRING_MIDDLE '' (1, 12) (1, 12)\nd = f\"{x:{w}}\" | OP '}' (1, 12) (1, 13)\nd = f\"{x:{w}}\" | FSTRING_END '\"' (1, 13) (1, 14)\nd = f\"{x:{w}}\" | NL '\\n' (2, 0) (2, 1)\ne = f\"{{literal}}\" | NAME 'e' (1, 0) (1, 1)\ne = f\"{{literal}}\" | OP '=' (1, 2) (1, 3)\ne = f\"{{literal}}\" | FSTRING_START 'f\"' (1, 4) (1, 6)\ne = f\"{{literal}}\" | FSTRING_MIDDLE '{' (1, 6) (1, 7)\ne = f\"{{literal}}\" | FSTRING_MIDDLE 'literal}' (1, 8) (1, 16)\ne = f\"{{literal}}\" | FSTRING_END '\"' (1, 17) (1, 18)\ne = f\"{{literal}}\" | NL '\\n' (2, 0) (2, 1)\ng = f\"{x=}\" | NAME 'g' (1, 0) (1, 1)\ng = f\"{x=}\" | OP '=' (1, 2) (1, 3)\ng = f\"{x=}\" | FSTRING_START 'f\"' (1, 4) (1, 6)\ng = f\"{x=}\" | OP '{' (1, 6) (1, 7)\ng = f\"{x=}\" | NAME 'x' (1, 7) (1, 8)\ng = f\"{x=}\" | OP '=' (1, 8) (1, 9)\ng = f\"{x=}\" | OP '}' (1, 9) (1, 10)\ng = f\"{x=}\" | FSTRING_END '\"' (1, 10) (1, 11)\ng = f\"{x=}\" | NL '\\n' (2, 0) (2, 1)\nh = f\"{x + 1}\" | NAME 'h' (1, 0) (1, 1)\nh = f\"{x + 1}\" | OP '=' (1, 2) (1, 3)\nh = f\"{x + 1}\" | FSTRING_START 'f\"' (1, 4) (1, 6)\nh = f\"{x + 1}\" | OP '{' (1, 6) (1, 7)\nh = f\"{x + 1}\" | NAME 'x' (1, 7) (1, 8)\nh = f\"{x + 1}\" | OP '+' (1, 9) (1, 10)\nh = f\"{x + 1}\" | NUMBER '1' (1, 11) (1, 12)\nh = f\"{x + 1}\" | OP '}' (1, 12) (1, 13)\nh = f\"{x + 1}\" | FSTRING_END '\"' (1, 13) (1, 14)\nh = f\"{x + 1}\" | NL '\\n' (2, 0) (2, 1)\ni = rf\"\\d{x}\" | NAME 'i' (1, 0) (1, 1)\ni = rf\"\\d{x}\" | OP '=' (1, 2) (1, 3)\ni = rf\"\\d{x}\" | FSTRING_START 'rf\"' (1, 4) (1, 7)\ni = rf\"\\d{x}\" | FSTRING_MIDDLE '\\\\d' (1, 7) (1, 9)\ni = rf\"\\d{x}\" | OP '{' (1, 9) (1, 10)\ni = rf\"\\d{x}\" | NAME 'x' (1, 10) (1, 11)\ni = rf\"\\d{x}\" | OP '}' (1, 11) (1, 12)\ni = rf\"\\d{x}\" | FSTRING_END '\"' (1, 12) (1, 13)\ni = rf\"\\d{x}\" | NL '\\n' (2, 0) (2, 1)\nj = f\"a{x}b{y}c\" | NAME 'j' (1, 0) (1, 1)\nj = f\"a{x}b{y}c\" | OP '=' (1, 2) (1, 3)\nj = f\"a{x}b{y}c\" | FSTRING_START 'f\"' (1, 4) (1, 6)\nj = f\"a{x}b{y}c\" | FSTRING_MIDDLE 'a' (1, 6) (1, 7)\nj = f\"a{x}b{y}c\" | OP '{' (1, 7) (1, 8)\nj = f\"a{x}b{y}c\" | NAME 'x' (1, 8) (1, 9)\nj = f\"a{x}b{y}c\" | OP '}' (1, 9) (1, 10)\nj = f\"a{x}b{y}c\" | FSTRING_MIDDLE 'b' (1, 10) (1, 11)\nj = f\"a{x}b{y}c\" | OP '{' (1, 11) (1, 12)\nj = f\"a{x}b{y}c\" | NAME 'y' (1, 12) (1, 13)\nj = f\"a{x}b{y}c\" | OP '}' (1, 13) (1, 14)\nj = f\"a{x}b{y}c\" | FSTRING_MIDDLE 'c' (1, 14) (1, 15)\nj = f\"a{x}b{y}c\" | FSTRING_END '\"' (1, 15) (1, 16)\nj = f\"a{x}b{y}c\" | NL '\\n' (2, 0) (2, 1)\nk = f\"{{x}}\" | NAME 'k' (1, 0) (1, 1)\nk = f\"{{x}}\" | OP '=' (1, 2) (1, 3)\nk = f\"{{x}}\" | FSTRING_START 'f\"' (1, 4) (1, 6)\nk = f\"{{x}}\" | FSTRING_MIDDLE '{' (1, 6) (1, 7)\nk = f\"{{x}}\" | FSTRING_MIDDLE 'x}' (1, 8) (1, 10)\nk = f\"{{x}}\" | FSTRING_END '\"' (1, 11) (1, 12)\nk = f\"{{x}}\" | NL '\\n' (2, 0) (2, 1)\nl = f\"a{{b}}c\" | NAME 'l' (1, 0) (1, 1)\nl = f\"a{{b}}c\" | OP '=' (1, 2) (1, 3)\nl = f\"a{{b}}c\" | FSTRING_START 'f\"' (1, 4) (1, 6)\nl = f\"a{{b}}c\" | FSTRING_MIDDLE 'a{' (1, 6) (1, 8)\nl = f\"a{{b}}c\" | FSTRING_MIDDLE 'b}' (1, 9) (1, 11)\nl = f\"a{{b}}c\" | FSTRING_MIDDLE 'c' (1, 12) (1, 13)\nl = f\"a{{b}}c\" | FSTRING_END '\"' (1, 13) (1, 14)\nl = f\"a{{b}}c\" | NL '\\n' (2, 0) (2, 1)\nm = f\"{{{x}}}\" | NAME 'm' (1, 0) (1, 1)\nm = f\"{{{x}}}\" | OP '=' (1, 2) (1, 3)\nm = f\"{{{x}}}\" | FSTRING_START 'f\"' (1, 4) (1, 6)\nm = f\"{{{x}}}\" | FSTRING_MIDDLE '{' (1, 6) (1, 7)\nm = f\"{{{x}}}\" | OP '{' (1, 8) (1, 9)\nm = f\"{{{x}}}\" | NAME 'x' (1, 9) (1, 10)\nm = f\"{{{x}}}\" | OP '}' (1, 10) (1, 11)\nm = f\"{{{x}}}\" | FSTRING_MIDDLE '}' (1, 11) (1, 12)\nm = f\"{{{x}}}\" | FSTRING_END '\"' (1, 13) (1, 14)\nm = f\"{{{x}}}\" | NL '\\n' (2, 0) (2, 1)\nn = f\"}}\" | NAME 'n' (1, 0) (1, 1)\nn = f\"}}\" | OP '=' (1, 2) (1, 3)\nn = f\"}}\" | FSTRING_START 'f\"' (1, 4) (1, 6)\nn = f\"}}\" | FSTRING_MIDDLE '}' (1, 6) (1, 7)\nn = f\"}}\" | FSTRING_END '\"' (1, 8) (1, 9)\nn = f\"}}\" | NL '\\n' (2, 0) (2, 1)\no = f\"{x:>{w}}\" | NAME 'o' (1, 0) (1, 1)\no = f\"{x:>{w}}\" | OP '=' (1, 2) (1, 3)\no = f\"{x:>{w}}\" | FSTRING_START 'f\"' (1, 4) (1, 6)\no = f\"{x:>{w}}\" | OP '{' (1, 6) (1, 7)\no = f\"{x:>{w}}\" | NAME 'x' (1, 7) (1, 8)\no = f\"{x:>{w}}\" | OP ':' (1, 8) (1, 9)\no = f\"{x:>{w}}\" | FSTRING_MIDDLE '>' (1, 9) (1, 10)\no = f\"{x:>{w}}\" | OP '{' (1, 10) (1, 11)\no = f\"{x:>{w}}\" | NAME 'w' (1, 11) (1, 12)\no = f\"{x:>{w}}\" | OP '}' (1, 12) (1, 13)\no = f\"{x:>{w}}\" | FSTRING_MIDDLE '' (1, 13) (1, 13)\no = f\"{x:>{w}}\" | OP '}' (1, 13) (1, 14)\no = f\"{x:>{w}}\" | FSTRING_END '\"' (1, 14) (1, 15)\no = f\"{x:>{w}}\" | NL '\\n' (2, 0) (2, 1)\np = f\"{x!r:>{w}}\" | NAME 'p' (1, 0) (1, 1)\np = f\"{x!r:>{w}}\" | OP '=' (1, 2) (1, 3)\np = f\"{x!r:>{w}}\" | FSTRING_START 'f\"' (1, 4) (1, 6)\np = f\"{x!r:>{w}}\" | OP '{' (1, 6) (1, 7)\np = f\"{x!r:>{w}}\" | NAME 'x' (1, 7) (1, 8)\np = f\"{x!r:>{w}}\" | OP '!' (1, 8) (1, 9)\np = f\"{x!r:>{w}}\" | NAME 'r' (1, 9) (1, 10)\np = f\"{x!r:>{w}}\" | OP ':' (1, 10) (1, 11)\np = f\"{x!r:>{w}}\" | FSTRING_MIDDLE '>' (1, 11) (1, 12)\np = f\"{x!r:>{w}}\" | OP '{' (1, 12) (1, 13)\np = f\"{x!r:>{w}}\" | NAME 'w' (1, 13) (1, 14)\np = f\"{x!r:>{w}}\" | OP '}' (1, 14) (1, 15)\np = f\"{x!r:>{w}}\" | FSTRING_MIDDLE '' (1, 15) (1, 15)\np = f\"{x!r:>{w}}\" | OP '}' (1, 15) (1, 16)\np = f\"{x!r:>{w}}\" | FSTRING_END '\"' (1, 16) (1, 17)\np = f\"{x!r:>{w}}\" | NL '\\n' (2, 0) (2, 1)\nq = f\"{ {1:2} }\" | NAME 'q' (1, 0) (1, 1)\nq = f\"{ {1:2} }\" | OP '=' (1, 2) (1, 3)\nq = f\"{ {1:2} }\" | FSTRING_START 'f\"' (1, 4) (1, 6)\nq = f\"{ {1:2} }\" | OP '{' (1, 6) (1, 7)\nq = f\"{ {1:2} }\" | OP '{' (1, 8) (1, 9)\nq = f\"{ {1:2} }\" | NUMBER '1' (1, 9) (1, 10)\nq = f\"{ {1:2} }\" | OP ':' (1, 10) (1, 11)\nq = f\"{ {1:2} }\" | NUMBER '2' (1, 11) (1, 12)\nq = f\"{ {1:2} }\" | OP '}' (1, 12) (1, 13)\nq = f\"{ {1:2} }\" | OP '}' (1, 14) (1, 15)\nq = f\"{ {1:2} }\" | FSTRING_END '\"' (1, 15) (1, 16)\nq = f\"{ {1:2} }\" | NL '\\n' (2, 0) (2, 1)\n";

/// `repr` 不可打印字符转义（`\xNN`／`\uNNNN`／`\U…` ✓）
const REPR_SCRIPT: &str = r#"CASES = ['\x00', '\x07', '\x0b', '\x0c', '\x1b', '\x7f', '\x80', '\x85', '\xa0', '\u200b',
         '\u2028', '\u2029', '\u3000', '\uffff', '\U0001f600', '\u00e9', '\u4e2d', ' ', '\u200e',
         "it's", 'a\'b"c', 'a\nb\tc\\d', 'plain']
for ch in CASES:
    print(hex(ord(ch)) if len(ch) == 1 else ch, "|", repr(ch))
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）：见 `REPR_SCRIPT` ✓。
const REPR_EXPECTED: &str = "0x0 | '\\x00'\n0x7 | '\\x07'\n0xb | '\\x0b'\n0xc | '\\x0c'\n0x1b | '\\x1b'\n0x7f | '\\x7f'\n0x80 | '\\x80'\n0x85 | '\\x85'\n0xa0 | '\\xa0'\n0x200b | '\\u200b'\n0x2028 | '\\u2028'\n0x2029 | '\\u2029'\n0x3000 | '\\u3000'\n0xffff | '\\uffff'\n0x1f600 | '😀'\n0xe9 | 'é'\n0x4e2d | '中'\n0x20 | ' '\n0x200e | '\\u200e'\nit's | \"it's\"\na'b\"c | 'a\\'b\"c'\na\nb	c\\d | 'a\\nb\\tc\\\\d'\nplain | 'plain'\n";

/// `len(str)` 按**字符**数（不是 UTF-8 字节 ✓）
const LENGTHS_SCRIPT: &str = r#"print(len("\u4e2d"), len("\u00e9"), len("\x80"), len("abc"))
print(len("\U0001f600"), len(""))
"#;

/// 参照（`python3` 3.14 实测 ✓，逐行）：见 `LENGTHS_SCRIPT` ✓。
const LENGTHS_EXPECTED: &str = "1 1 1 3\n1 0\n";

#[test]
fn continuation_nl_agree_with_the_reference() {
    let stdout = run("continuation_nl", CONTINUATION_SCRIPT);
    assert_eq!(stdout, CONTINUATION_EXPECTED, "{stdout}");
}

#[test]
fn operators_comments_and_dedents_agree_with_the_reference() {
    let stdout = run("operators_comments_and_dedents", OPERATORS_SCRIPT);
    assert_eq!(stdout, OPERATORS_EXPECTED, "{stdout}");
}

#[test]
fn comments_indents_and_multiline_strings_agree_with_the_reference() {
    let stdout = run("comments_indents_and_multiline_strings", COMMENTED_SCRIPT);
    assert_eq!(stdout, COMMENTED_EXPECTED, "{stdout}");
}

#[test]
fn fstring_three_part_split_agree_with_the_reference() {
    let stdout = run("fstring_three_part_split", FSTRINGS_SCRIPT);
    assert_eq!(stdout, FSTRINGS_EXPECTED, "{stdout}");
}

#[test]
fn repr_escapes_non_printables_agree_with_the_reference() {
    let stdout = run("repr_escapes_non_printables", REPR_SCRIPT);
    assert_eq!(stdout, REPR_EXPECTED, "{stdout}");
}

#[test]
fn str_len_counts_characters_agree_with_the_reference() {
    let stdout = run("str_len_counts_characters", LENGTHS_SCRIPT);
    assert_eq!(stdout, LENGTHS_EXPECTED, "{stdout}");
}

/// `str.splitlines(keepends)` 的四档（task-19 路上钉死的真 bug ✗）。
const SPLITLINES_SCRIPT: &str = r#"S = 'a\nb\n'
print(S.splitlines(), S.splitlines(True))
T = 'x\r\ny\r\nz'
print(T.splitlines(True))
print(''.splitlines(), 'a'.splitlines(True))
P = 'p\n\nq'
print(P.splitlines(True))
Q = 'r\r'
print(Q.splitlines(True), Q.splitlines())
"#;

/// 参照（`python3` 3.14 实测 ✓）。
const SPLITLINES_EXPECTED: &str = "['a', 'b'] ['a\\n', 'b\\n']\n['x\\r\\n', 'y\\r\\n', 'z']\n[] ['a']\n['p\\n', '\\n', 'q']\n['r\\r'] ['r']\n";

#[test]
fn splitlines_keepends_agrees_with_the_reference() {
    let stdout = run("splitlines", SPLITLINES_SCRIPT);
    assert_eq!(stdout, SPLITLINES_EXPECTED, "{stdout}");
}
