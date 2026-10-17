# **`match` 语句**（第 290 轮，最小面：字面量／捕获／通配／或 ＋ 守卫）。参照 3.14 对字面量模式
# 不发 `MATCH_*`（`COPY 1; LOAD_CONST; COMPARE_OP 88; POP_JUMP_IF_FALSE`），
# 所以最小面**不需要新指令**。`asyncio/__init__.py:54` 那种 `__getattr__` 分派就是它（35 个模块）。


def classify(name):
    match name:
        case "a":
            return 1
        case "b":
            return 2
        case "c" | "d":
            return 3
        case None:
            return 4
        case True:
            return 5
        case _:
            return 9


print(str(classify("a")))
print(str(classify("b")))
print(str(classify("d")))
print(str(classify(None)))
print(str(classify(True)))
print(str(classify("zzz")))

value = 7
match value:
    case 7 if value > 5:
        print("seven")
    case 7:
        print("nope")
    case other:
        print("other " + str(other))

for item in ["x", 1, None]:
    match item:
        case "x":
            print("str x")
        case 1:
            print("int one")
        case _:
            print("fallback")

match "only":
    case "only":
        print("module scope ok")

# 捕获 + 守卫：守卫不过时**不绑**（也不进下一条 case 的重复判定）
match 3:
    case captured if captured > 10:
        print("big")
    case captured:
        print("small " + str(captured))
