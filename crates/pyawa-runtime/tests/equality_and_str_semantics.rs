//! **`==`／`!=` 的特殊方法派发 ＋ 一批 `str` 方法的参照口径**（`P3-42` round 44）。
//!
//! 落地：
//! * `==`／`!=` 现在**先问两侧的 `__eq__`／`__ne__`**（只认**用户类**，`object` 的默认实现跳过，
//!   否则无限递归 ✓），容器 `list`／`tuple`／`dict` **逐元素**递归（元素上的 `__eq__` 同样生效），
//!   最后退回结构相等；`NotImplemented` 交给下一跳 ✓。此前 `==` 是"纯结构" ⇒
//!   `UserString('ab') == 'ab'` 为 False ✗ ⇒ `test_userstring` 38 failures／4 errors。
//! * `str` 的参照细节：`center`／`ljust`／`rjust` 接**填充字符**；`replace` 的 `count` 生效
//!   （含关键字）；`zfill` 把零补在**符号之后**；`expandtabs` 在 `\n`／`\r` 处**归零列**；
//!   `%` 的 `*`／`.*` 从实参取宽、超大宽度报 `ValueError`、超 `ssize_t` 报 `OverflowError`、
//!   `%(名字)` 括号配平、`str.__mod__` 可按名取用、无转换却给实参报 `TypeError`；
//!   `rpartition('')`／`partition('')` 报 `ValueError`；`rfind('' , 0)` 夹 `end`；
//!   搜索族多给实参报 `<name> expected at most 3 arguments, got N`；下标是字符串时按容器口径报
//!   `TypeError`，超出 `i64` 的大整数下标按方向饱和；切片 `step`／边界饱和，`sys.maxsize` 量级不再溢出。
//!
//! 期望值由本机 `python3`（3.14.4）跑**同一段脚本**取得（`MS-17`），逐字节一致。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-{}-{}", std::process::id(), name));
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

const SCRIPT: &str = r#"class Eq:
    def __init__(self, v):
        self.v = v
    def __eq__(self, other):
        if isinstance(other, Eq):
            return self.v == other.v
        return NotImplemented
    def __repr__(self):
        return "Eq(%r)" % (self.v,)

class NI:
    def __eq__(self, other):
        return NotImplemented

print(Eq(1) == Eq(1), Eq(1) == Eq(2), Eq(1) != Eq(2))
print([Eq(1)] == [Eq(1)], (Eq(1),) == (Eq(1),), [Eq(1)] == [Eq(2)])
print({"a": Eq(1)} == {"a": Eq(1)}, {"a": Eq(1)} == {"a": Eq(2)})
print(NI() == NI(), NI() == 1)
print(repr("abc".center(10, "*")), repr("abc".ljust(7, "-")), repr("abc".rjust(7, "-")))
print(repr("+123".zfill(5)), repr("-42".zfill(5)), repr("abc".center(9, "*")))
print(repr("abab".replace("a", "x", 1)), repr("abab".replace("a", "x", count=0)))
print(repr("aa".replace("", "*-", 1)), repr("".replace("", "A")))
print(repr("abc\rab      def\ng       hi".expandtabs()))
print(repr("%*s" % (3, "x")), repr("%.*f" % (2, 3.14159)))
print("%((foo))s" % {"(foo)": "bar"})
try:
    "abc" % object()
except TypeError as error:
    print(type(error).__name__, error)
try:
    "ab".rpartition("")
except ValueError as error:
    print(type(error).__name__, error)
print("abc".rfind("", 0), "abc".rfind("", 3), "abc".rfind("", 4))
try:
    "hello".find("x", None, None, None)
except TypeError as error:
    print(type(error).__name__, str(error)[:6])"#;

const EXPECTED: &str = "True False True\nTrue True False\nTrue False\nFalse False\n'***abc****' 'abc----' '----abc'\n'+0123' '-0042' '***abc***'\n'xbab' 'abab'\n'*-aa' 'A'\n'abc\\rab      def\\ng       hi'\n'  x' '3.14'\nbar\nTypeError not all arguments converted during string formatting\nValueError empty separator\n3 3 -1\nTypeError find e\n";

#[test]
fn equality_dispatch_and_str_semantics_match_the_reference() {
    assert_eq!(run("r44", SCRIPT), EXPECTED);
}
