# 类体（不含 `def` 的那一档）能跑完：类创建**要读模块的 `__name__`**（类体序言），
# 所以这一条同时盯着 `pa_exec_string` 的"补 `__name__ = '__main__'`"那半。
#
# 注意：**不**在这里读 `C.v` —— 类对象上的属性读（`C.attr`）尚未接线
# （实测 `'type' object has no attribute 'v'`），按 `MS-19` 的适用范围属于"尚未实现"、
# 不进语料（见 tests/conformance/README.md 的"实测边界"）。
class C:
    v = 5
x = 'ok'
