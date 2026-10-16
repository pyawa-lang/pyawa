//! **`_tokenize.TokenizerIter`——逐 token 与参照对齐**（task-19 第 1 轮 ✓）。
//!
//! 参照值由本机 `python3`（3.14）跑**同一段探针**取得 ✓（脚本见各 `const` ✓）。
//!
//! **一处真相** ✓：词法扫描就是编译器的词法器（`pyawa_core::python_tokens` ✓），
//! `_tokenize` 只做参照投影 ✓。被这组抽例钉住的真 bug：
//! ① `str.splitlines(True)` **没读 `keepends`** ✗（还给 `['a','b']` ✗ ⇒ readline 拼不出源码 ✓）；
//! ② 续行的 `NL` 底层不发 ✓（要按"隐式续行"补 ✓）；
//! ③ 多行 token 的 `line` 字段＝**这个 token 自己吃掉的物理行** ✓；
//! ④ `DEDENT` 的**位置＝触发行的缩进宽度** ✓、`line` ＝那一行 ✓（不是恒 `(row,0)` ＋空串 ✗）；
//! ⑤ 行尾 `COMMENT` 的文本**不含 `\n`** ✓。
//!
//! **如实范围 ✗**（第 2 轮 ✓）：**f-string 整段按 `STRING` 出** ✗ —— 参照给
//! `FSTRING_START`／`FSTRING_MIDDLE`／`FSTRING_END` ✓（故本组抽例**不含 f-string** ✓）。

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
