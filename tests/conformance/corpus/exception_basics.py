# 异常基础（第 163 轮修的真 bug）：异常类型此前**不在内建名字空间** ⇒ 最基础的 try/except 都走不通
try:
    raise ValueError("boom")
except ValueError:
    caught = "same"

assert caught == "same"

try:
    raise KeyError("k")
except ValueError:
    caught2 = "wrong"
except KeyError:
    caught2 = "keyerror"

assert caught2 == "keyerror"

# 子类匹配：ModuleNotFoundError 要被 ImportError 接住
try:
    import no_such_module_xyz
except ImportError:
    caught3 = "import-error"

assert caught3 == "import-error"

# 注：`finally` 里访问**模块级**变量（`order.append(...)`）在本层会报 `NameError` ✗
# ⇒ 那是**另一条**独立缺陷（异常块的作用域 ✓，已登记 ✗）⇒ 本条语料只守**已通**的部分 ✓。
try:
    raise ValueError("x")
except ValueError:
    caught4 = "except"
finally:
    caught4 = caught4 + "-finally"

assert caught4 == "except-finally"
assert ValueError("m") is not None
print("ok")
