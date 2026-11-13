//! **`_csv`／`csv` 的面（`task-3` ✓）**：`PLAN-milestones.md` §12.3 的双侧对拍抽例。
//!
//! 口径（照 `signal_surface_shapes.rs`）：
//!
//! * **`FACE_SCRIPT`**：同一段探针在**本机 `python3` 3.14.4** 与 pyawa 上**逐行一致** ✓ ——
//!   `_csv` 的模块面（`Error` 的 mro／`__module__`／构造、`field_size_limit`、导出名、
//!   `QUOTE_*` 常量）、方言注册表（`get`／`register`／`unregister`／`list` 与全部 `TypeError`／
//!   `Error: unknown dialect` 文案）、`Dialect` 的**属性校验文案**（`Lib/csv.py` 的 `_validate`
//!   把它们原样包成 `csv.Error` ⇒ 消息必须逐字一致）、`reader` 的解析各族（EOL／EOF／引号／
//!   转义／`skipinitialspace`／`QUOTE_*` 读侧／`line_num`）、`writer` 的写侧各族
//!   （`QUOTE_MINIMAL/ALL/NONNUMERIC/NONE/STRINGS/NOTNULL`、`doublequote`／`escapechar`／
//!   `lineterminator`、空字段规则、`writerow` 返回值＝`write()` 的返回值）、
//!   `DictReader`／`DictWriter`（含 `restkey`／`restval`／`extrasaction`／`writeheader` 返回值）、
//!   以及 `Sniffer.sniff`／`has_header`。期望值由**本机 `python3` 3.14.4 跑同一段脚本**取得 ✓。
//! * **`HONEST_SCRIPT`**：pyawa 侧**独有**的实现边界（如实钉住，避免以后漂移）：
//!   - 原生函数的 `__module__` 是 core 的统一口径 `"builtins"`（参照给 `"_csv"`）⇒
//!     `test.support.check__all__` 认不出这 7 个名字（上游 `test_csv.test__all__` 的唯一失败点）；
//!     原生函数也没有 clinic 的 `__text_signature__`（`None`）。
//!   - 三个类型是**每实例注册**的原生类型 ⇒ `__name__` 是 `_csv.reader`／`_csv.writer`／
//!     `_csv.Dialect`（参照分别是 `reader`／`writer`／`Dialect`）；`isinstance` 面照参照一致。
//!   - 实例状态放**实例字典的私有键**（`__pyawa_csv_*`；参照是 C 载荷、无 `__dict__`）；
//!     模块命名空间里多一个私有状态键 `__pyawa_csv_field_limit`（字段上限；`_dialects` 照参照）。
//!   - 只读 `Dialect` 属性用 `property` ⇒ `setattr`／`delattr` 报 `AttributeError`（类型一致；
//!     文案是 property 的，不是参照 C getset 的）。
//!
//! **为什么 `test__all__` 失败不修**（如实归因，不是 `_csv` 的语义问题）：`check__all__` 按
//! `obj.__module__ in ('csv', '_csv')` 认公共名，而本层 `builtin_function_or_method.__module__`
//! 在 `pyawa-core` 里**统一**报 `builtins`（`crates/pyawa-core/src/builtin_objects/callables.rs`
//! 的 `builtin_function_getattr`）—— 修它要动 core（本任务写域外）且影响所有内建模块。
//!
//! **另一个上游子测试组的如实归因**：`test_roundtrip_quoteed_newlines`／
//! `test_roundtrip_escaped_unquoted_newlines` 的 `lineterminator='\r'` 两格报 `newline=''` 错误，
//! 根因不在 `_csv`：本层 `_io` 的 `TextIOWrapper(newline='')` **不在孤立 `\r` 处断行**
//! （`['a,b\rc,d\r']` 而不是 `['a,b\r','c,d\r']`）⇒ reader 拿到的"一行"里含内嵌换行；`_csv`
//! 照参照语义如实报错（不偷偷容忍）。

fn run(name: &str, script: &str) -> String {
    let root = std::env::temp_dir().join(format!("pyawa-csv-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("建临时目录");
    let path = root.join("probe.py");
    std::fs::write(&path, script).expect("写脚本");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pyawa"))
        .arg(&path)
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

/// 参照侧脚本（`python3` 3.14.4 跑同一段 ⇒ `FACE_EXPECTED` ✓）。
const FACE_SCRIPT: &str = r#"import csv, io, _csv

def show(label, fn):
    try:
        print(label, "=>", repr(fn()))
    except BaseException as exc:
        print(label, "!!", type(exc).__name__, ":", exc)

print("version", csv.__version__)
print("csv-all", csv.__all__)
print("module-name", _csv.__name__)
print("module-doc", _csv.__doc__)
print("consts", _csv.QUOTE_MINIMAL, _csv.QUOTE_ALL, _csv.QUOTE_NONNUMERIC, _csv.QUOTE_NONE, _csv.QUOTE_STRINGS, _csv.QUOTE_NOTNULL)
print("error", _csv.Error.__name__, _csv.Error.__module__, _csv.Error is csv.Error, issubclass(_csv.Error, Exception), [cls.__name__ for cls in _csv.Error.__mro__])
show("error-construct", lambda: (lambda e: (str(e), e.args))( _csv.Error("boom")))
show("fsl-default", lambda: csv.field_size_limit())
show("fsl-bad", lambda: csv.field_size_limit(None))
show("fsl-many", lambda: csv.field_size_limit(1, 2))
show("fsl-kw", lambda: csv.field_size_limit(limit=3))
print("list-dialects", csv.list_dialects())
print("excel-attrs", (lambda d: (d.delimiter, d.quotechar, d.escapechar, d.doublequote, d.skipinitialspace, d.lineterminator, d.quoting, d.strict))(csv.get_dialect("excel")))
print("tab-delim", csv.get_dialect("excel-tab").delimiter)
print("unix", (csv.get_dialect("unix").lineterminator, csv.get_dialect("unix").quoting))
print("get-identity", csv.get_dialect("excel") is csv.get_dialect("excel"))
print("get-type", type(csv.get_dialect("excel")) is _csv.Dialect)
show("get-missing", lambda: csv.get_dialect("nonesuch"))
show("get-none", lambda: csv.get_dialect(None))
show("unregister-missing", lambda: csv.unregister_dialect("nonesuch"))
show("register-name", lambda: csv.register_dialect(1))
show("register-args", lambda: csv.register_dialect("x", 0, 0))
show("register-kw", lambda: csv.register_dialect("x", badargument=None))
show("register-quoting", lambda: csv.register_dialect("x", quoting=None))
show("list-arg", lambda: csv.list_dialects(None))

old_limit = csv.field_size_limit(10)
show("fsl-too-big", lambda: list(csv.reader(["x" * 11])))
print("fsl-exact", len(list(csv.reader(["x" * 10]))[0]))
print("fsl-restore", csv.field_size_limit(131072) == 10, csv.field_size_limit())
def show_type(label, fn):
    try:
        fn()
        print(label, "=> no error")
    except BaseException as exc:
        print(label, "!!", type(exc).__name__)
print("dialect-set", end=" ")
show_type("", lambda: setattr(csv.get_dialect("excel"), "delimiter", ";"))
print("dialect-del", end=" ")
show_type("", lambda: delattr(csv.get_dialect("excel"), "delimiter"))

csv.register_dialect("zz-tsv", delimiter="\t", lineterminator="\n")
print("registered", csv.list_dialects())
print("registered-read", list(csv.reader(["a\tb\n"], "zz-tsv")))
csv.unregister_dialect("zz-tsv")
print("unregistered", csv.list_dialects())

def bad_dialect(**kwargs):
    class mydialect(csv.Dialect):
        delimiter = ";"
        escapechar = "\\"
        doublequote = False
        skipinitialspace = True
        lineterminator = "\r\n"
        quoting = csv.QUOTE_NONE
    for key, value in kwargs.items():
        setattr(mydialect, key, value)
    return mydialect()

for key, value in [("delimiter", ":::"), ("delimiter", b","), ("delimiter", None),
                   ("escapechar", "**"), ("quotechar", ""), ("quotechar", 4),
                   ("lineterminator", 4), ("quoting", None), ("quoting", 42)]:
    show("dialect-%s-%r" % (key, value), lambda k=key, v=value: bad_dialect(**{k: v}))
show("dialect-quotechar-none", lambda: bad_dialect(quotechar=None, quoting=csv.QUOTE_MINIMAL).quoting)
show("dialect-quote-none", lambda: bad_dialect(quotechar=None).quoting)
show("dialect-bad-char", lambda: bad_dialect(delimiter="\n"))
show("dialect-conflict", lambda: bad_dialect(delimiter="\\"))
show("dialect-lineterm-conflict", lambda: bad_dialect(delimiter="\r"))
show("raw-dialect", lambda: (lambda d: (d.delimiter, d.quoting))(_csv.Dialect()))
show("raw-dialect-name", lambda: _csv.Dialect("excel").delimiter)
show("raw-dialect-bad", lambda: _csv.Dialect(delimiter=";;"))
show("raw-dialect-badkw", lambda: _csv.Dialect(bad=1))
show("raw-reader", lambda: _csv.Reader())
show("raw-writer", lambda: _csv.Writer())
print("types", isinstance(csv.reader([]), _csv.Reader), isinstance(csv.writer(io.StringIO()), _csv.Writer), isinstance(csv.get_dialect("excel"), _csv.Dialect))

# ---- reader ----
def read(lines, *args, **kwargs):
    return list(csv.reader(lines, *args, **kwargs))

print("read-basic", read(["a,b\n", "1,2\n"]))
print("read-eol", read(["a,b", "c,d"]))
print("read-crlf", read(["a,b\r\n", "c,d\r\n"]))
print("read-empty", read([""]))
print("read-none", read([]))
print("read-nul", read(["a,\0b,c"]))
print("read-escape", read(["a,b\\,c", 'x,"p\\"q"'], escapechar="\\"))
print("read-quote-none", read(['1,",3,",5'], quoting=csv.QUOTE_NONE, escapechar="\\"))
print("read-nonnum", read([',3,"5",7.3, 9'], quoting=csv.QUOTE_NONNUMERIC))
print("read-strings", read([',3,"5",7.3, 9'], quoting=csv.QUOTE_STRINGS))
print("read-notnull", read([',3,"5",7.3, 9'], quoting=csv.QUOTE_NOTNULL))
print("read-multiline", read(['a,"b\nc",d\r\n', "e,f,g\r\n"]))
print("read-skipinit", read(["no space, space,  spaces"], skipinitialspace=True))
print("read-space-delim", read(["a   b"], delimiter=" ", skipinitialspace=False))
print("read-space-delim2", read(["a   b"], delimiter=" ", skipinitialspace=True))
print("read-doublequote", read(['"ab"c'], doublequote=0))
show("read-strict", lambda: read(['a,b"c'], strict=True))
show("read-strict-eof", lambda: read(['"a'], strict=True))
show("read-bytes", lambda: read([b"abc"]))
show("read-newline", lambda: read(["a,b\rc,d"]))
show("read-bad-dialect", lambda: read([], "nonesuch"))
print("reader-iter-self", (lambda r: iter(r) is r)(csv.reader([])))
print("reader-dialect", (lambda r: (r.dialect.delimiter, r.dialect.lineterminator))(csv.reader([], delimiter=";", lineterminator="\n")))
r = csv.reader(io.StringIO("a,b\nc,d\ne,f\n"))
print("line-nums", r.line_num)
print("line-1", next(r), r.line_num)
print("line-2", next(r), r.line_num)
print("line-3", next(r), r.line_num)
show("line-stop", lambda: next(r))
print("line-after", r.line_num)

# ---- writer ----
def write(fields, **kwargs):
    sink = io.StringIO()
    writer = csv.writer(sink, **kwargs)
    result = writer.writerow(fields)
    return (sink.getvalue(), result)

print("write-minimal", write(["a", 1, "p,q"]))
print("write-all", write(["a", 1, "p,q"], quoting=csv.QUOTE_ALL))
print("write-nonnum", write(["a", 1, "p,q"], quoting=csv.QUOTE_NONNUMERIC))
print("write-strings", write(["a", "", None, 1], quoting=csv.QUOTE_STRINGS))
print("write-notnull", write(["a", "", None, 1], quoting=csv.QUOTE_NOTNULL))
print("write-escape", write(['C\\', "6", 'X"'], escapechar="\\", quoting=csv.QUOTE_MINIMAL))
print("write-escape2", write(['p,"q"'], escapechar="\\", doublequote=False))
print("write-empty", write(["", ""]))
print("write-single-empty", write([None]))
print("write-empty-record", write([]))
print("write-lt", write(["a", "b"], lineterminator="!@#"))
show("write-none-quote", lambda: write(["a", "p,q"], quoting=csv.QUOTE_NONE))
show("write-single-nq", lambda: write([""], quoting=csv.QUOTE_NONE))
show("write-single-notnull", lambda: write([None], quoting=csv.QUOTE_NOTNULL))
show("write-space", lambda: write(["", ""], delimiter=" ", skipinitialspace=True))
show("write-space-nq", lambda: write(["", ""], delimiter=" ", skipinitialspace=True, quoting=csv.QUOTE_NONE))
show("write-int", lambda: csv.writer(io.StringIO()).writerow(1))
show("write-none-row", lambda: csv.writer(io.StringIO()).writerow(None))
show("writer-no-write", lambda: csv.writer(object()))
print("writer-dialect", (lambda w: (w.dialect.delimiter, w.dialect.quoting))(csv.writer(io.StringIO())))
print("writerows-return", (lambda s: (csv.writer(s).writerows([["a", "b"], ["c", "d"]]), s.getvalue()))(io.StringIO()))

class Sink:
    def __init__(self):
        self.data = ""
    def write(self, text):
        self.data += text
        return "written:%d" % len(text)

sink = Sink()
print("write-return-custom", csv.writer(sink).writerow(["a"]), repr(sink.data))

# ---- DictReader / DictWriter ----
rows = ["f1,f2,f3\r\n", "1,2,abc\r\n", "4,5\r\n", "6,7,8,9\r\n"]
dict_reader = csv.DictReader(rows)
print("dict-fields", dict_reader.fieldnames)
print("dict-rows", list(dict_reader))
print("dict-restkey", list(csv.DictReader(["h1,h2\r\n", "a,b,c\r\n"], restkey="_rest")))
print("dict-restval", list(csv.DictReader(["h1,h2,h3\r\n", "a,b\r\n"], restval="DEFAULT")))
sink = io.StringIO()
dict_writer = csv.DictWriter(sink, fieldnames=["f1", "f2"])
print("dict-writeheader", dict_writer.writeheader(), repr(sink.getvalue()))
print("dict-writerow", dict_writer.writerow({"f1": 10, "f2": "x"}), repr(sink.getvalue()))
show("dict-extra-raise", lambda: dict_writer.writerow({"f3": 1}))
ignore = csv.DictWriter(io.StringIO(), fieldnames=["f1"], extrasaction="ignore")
print("dict-extra-ignore", ignore.writerow({"f1": "a", "f9": "b"}))
show("dict-bad-extras", lambda: csv.DictWriter(io.StringIO(), ["f1"], extrasaction="raised"))

# ---- Sniffer ----
sniffer = csv.Sniffer()
dialect = sniffer.sniff("a;b;c\n1;2;3\n")
print("sniff", dialect.delimiter, dialect.quotechar, dialect.doublequote, repr(dialect.lineterminator), dialect.quoting)
print("has-header", sniffer.has_header("name,age\nAlice,30\nBob,25\n"), sniffer.has_header("1,2\n3,4\n"))
"#;

/// 参照（本机 `python3` 3.14.4 跑 `FACE_SCRIPT` 实测 ✓，逐行）。
const FACE_EXPECTED: &str = r#"version 1.0
csv-all ['QUOTE_MINIMAL', 'QUOTE_ALL', 'QUOTE_NONNUMERIC', 'QUOTE_NONE', 'QUOTE_STRINGS', 'QUOTE_NOTNULL', 'Error', 'Dialect', 'excel', 'excel_tab', 'field_size_limit', 'reader', 'writer', 'register_dialect', 'get_dialect', 'list_dialects', 'Sniffer', 'unregister_dialect', 'DictReader', 'DictWriter', 'unix_dialect']
module-name _csv
module-doc CSV parsing and writing.

consts 0 1 2 3 4 5
error Error _csv True True ['Error', 'Exception', 'BaseException', 'object']
error-construct => ('boom', ('boom',))
fsl-default => 131072
fsl-bad !! TypeError : limit must be an integer
fsl-many !! TypeError : field_size_limit() takes at most 1 argument (2 given)
fsl-kw !! TypeError : field_size_limit() got an unexpected keyword argument 'limit'
list-dialects ['excel', 'excel-tab', 'unix']
excel-attrs (',', '"', None, True, False, '\r\n', 0, False)
tab-delim 	
unix ('\n', 1)
get-identity True
get-type True
get-missing !! Error : unknown dialect
get-none !! Error : unknown dialect
unregister-missing !! Error : unknown dialect
register-name !! TypeError : dialect name must be a string
register-args !! TypeError : register_dialect expected at most 2 arguments, got 3
register-kw !! TypeError : this function got an unexpected keyword argument 'badargument'
register-quoting !! TypeError : "quoting" must be an integer, not NoneType
list-arg !! TypeError : _csv.list_dialects() takes no arguments (1 given)
fsl-too-big !! Error : field larger than field limit (10)
fsl-exact 1
fsl-restore True 131072
dialect-set  !! AttributeError
dialect-del  !! AttributeError
registered ['excel', 'excel-tab', 'unix', 'zz-tsv']
registered-read [['a', 'b']]
unregistered ['excel', 'excel-tab', 'unix']
dialect-delimiter-':::' !! Error : "delimiter" must be a unicode character, not a string of length 3
dialect-delimiter-b',' !! Error : "delimiter" must be a unicode character, not bytes
dialect-delimiter-None !! Error : "delimiter" must be a unicode character, not NoneType
dialect-escapechar-'**' !! Error : "escapechar" must be a unicode character or None, not a string of length 2
dialect-quotechar-'' !! Error : "quotechar" must be a unicode character or None, not a string of length 0
dialect-quotechar-4 !! Error : "quotechar" must be a unicode character or None, not int
dialect-lineterminator-4 !! Error : "lineterminator" must be a string, not int
dialect-quoting-None !! Error : "quoting" must be an integer, not NoneType
dialect-quoting-42 !! Error : bad "quoting" value
dialect-quotechar-none !! Error : quotechar must be set if quoting enabled
dialect-quote-none => 3
dialect-bad-char !! ValueError : bad delimiter value
dialect-conflict !! ValueError : bad delimiter or escapechar value
dialect-lineterm-conflict !! ValueError : bad delimiter value
raw-dialect => (',', 0)
raw-dialect-name => ','
raw-dialect-bad !! TypeError : "delimiter" must be a unicode character, not a string of length 2
raw-dialect-badkw !! TypeError : this function got an unexpected keyword argument 'bad'
raw-reader !! TypeError : cannot create '_csv.reader' instances
raw-writer !! TypeError : cannot create '_csv.writer' instances
types True True True
read-basic [['a', 'b'], ['1', '2']]
read-eol [['a', 'b'], ['c', 'd']]
read-crlf [['a', 'b'], ['c', 'd']]
read-empty [[]]
read-none []
read-nul [['a', '\x00b', 'c']]
read-escape [['a', 'b,c'], ['x', 'p"q']]
read-quote-none [['1', '"', '3', '"', '5']]
read-nonnum [['', 3.0, '5', 7.3, 9.0]]
read-strings [[None, 3.0, '5', 7.3, 9.0]]
read-notnull [[None, '3', '5', '7.3', ' 9']]
read-multiline [['a', 'b\nc', 'd'], ['e', 'f', 'g']]
read-skipinit [['no space', 'space', 'spaces']]
read-space-delim [['a', '', '', 'b']]
read-space-delim2 [['a', 'b']]
read-doublequote [['abc']]
read-strict => [['a', 'b"c']]
read-strict-eof !! Error : unexpected end of data
read-bytes !! Error : iterator should return strings, not bytes (the file should be opened in text mode)
read-newline !! Error : new-line character seen in unquoted field - do you need to open the file with newline=''?
read-bad-dialect !! Error : unknown dialect
reader-iter-self True
reader-dialect (';', '\n')
line-nums 0
line-1 ['a', 'b'] 1
line-2 ['c', 'd'] 2
line-3 ['e', 'f'] 3
line-stop !! StopIteration : 
line-after 3
write-minimal ('a,1,"p,q"\r\n', 11)
write-all ('"a","1","p,q"\r\n', 15)
write-nonnum ('"a",1,"p,q"\r\n', 13)
write-strings ('"a","",,1\r\n', 11)
write-notnull ('"a","",,"1"\r\n', 13)
write-escape ('C\\\\,6,"X"""\r\n', 13)
write-escape2 ('"p,\\"q\\""\r\n', 11)
write-empty (',\r\n', 3)
write-single-empty ('""\r\n', 4)
write-empty-record ('\r\n', 2)
write-lt ('a,b!@#', 6)
write-none-quote !! Error : need to escape, but no escapechar set
write-single-nq !! Error : single empty field record must be quoted
write-single-notnull !! Error : single empty field record must be quoted
write-space => ('"" ""\r\n', 7)
write-space-nq !! Error : empty field must be quoted if delimiter is a space and skipinitialspace is true
write-int !! Error : iterable expected, not int
write-none-row !! Error : iterable expected, not NoneType
writer-no-write !! TypeError : argument 1 must have a "write" method
writer-dialect (',', 0)
writerows-return (None, 'a,b\r\nc,d\r\n')
write-return-custom written:3 'a\r\n'
dict-fields ['f1', 'f2', 'f3']
dict-rows [{'f1': '1', 'f2': '2', 'f3': 'abc'}, {'f1': '4', 'f2': '5', 'f3': None}, {'f1': '6', 'f2': '7', 'f3': '8', None: ['9']}]
dict-restkey [{'h1': 'a', 'h2': 'b', '_rest': ['c']}]
dict-restval [{'h1': 'a', 'h2': 'b', 'h3': 'DEFAULT'}]
dict-writeheader 7 'f1,f2\r\n'
dict-writerow 6 'f1,f2\r\n10,x\r\n'
dict-extra-raise !! ValueError : dict contains fields not in fieldnames: 'f3'
dict-extra-ignore 3
dict-bad-extras !! ValueError : extrasaction (raised) must be 'raise' or 'ignore'
sniff ; " False '\r\n' 0
has-header True False
"#;

/// pyawa 侧独有边界（见文件头 ✓；参照侧无对应物）。
const HONEST_SCRIPT: &str = r#"import csv, io, _csv

# 1) 原生函数的 `__module__`：本层 core 统一给 "builtins"（参照给 "_csv"）⇒
#    `test.support.check__all__` 认不出这 7 个名字（上游 test_csv.test__all__ 的唯一失败点）。
print("native-module", _csv.reader.__module__, _csv.writer.__module__, _csv.field_size_limit.__module__)
print("native-text-signature", _csv.reader.__text_signature__)

# 2) 类型名走本层"每实例类型注册"口径（参照 `__name__` 是 reader／writer／Dialect）。
print("type-names", _csv.Reader.__name__, _csv.Writer.__name__, _csv.Dialect.__name__)

# 3) 实例状态放实例字典的私有键（参照是 C 载荷、没有 __dict__）。
r = csv.reader(io.StringIO("a,b\n"))
w = csv.writer(io.StringIO())
d = csv.get_dialect("excel")
print("reader-dict", sorted(r.__dict__))
print("writer-dict", sorted(w.__dict__))
print("dialect-dict", hasattr(d, "__dict__"))

# 4) 模块命名空间里多一个私有状态键（字段大小上限）；`_dialects` 照参照暴露。
print("module-private-key", "__pyawa_csv_field_limit" in dir(_csv))
print("registry-dict", type(_csv._dialects).__name__, sorted(_csv._dialects))

# 5) `writerow` 的返回值就是文件对象 `write()` 的返回值（与参照一致）。
class Sink:
    def write(self, text):
        return ("W", len(text))
print("writerow-return", csv.writer(Sink()).writerow(["ab"]))
"#;

/// pyawa 侧期望（实跑取得 ✓，逐行）。
const HONEST_EXPECTED: &str = r#"native-module builtins builtins builtins
native-text-signature None
type-names _csv.reader _csv.writer _csv.Dialect
reader-dict ['__pyawa_csv_dialect', '__pyawa_csv_input', '__pyawa_csv_line_num']
writer-dict ['__pyawa_csv_dialect', '__pyawa_csv_write']
dialect-dict True
module-private-key True
registry-dict dict ['excel', 'excel-tab', 'unix']
writerow-return ('W', 4)
"#;

#[test]
fn csv_face_matches_reference() {
    assert_eq!(run("face", FACE_SCRIPT), FACE_EXPECTED);
}

#[test]
fn csv_native_surface_is_honestly_pinned() {
    assert_eq!(run("honest", HONEST_SCRIPT), HONEST_EXPECTED);
}
