# `functools.singledispatch.register` 的**实测原形**（本会话 46b5f07）：`argname, cls = next(...)`
# 的 `cls` 是形参 cell，随后 `p = cls` 又作右值 ⇒ 两处都必须走 DEREF，不能按 varnames 快路。
def register(cls):
    argname, cls = next(iter([("arg", cls)]))

    def wrapper(fn):
        return cls

    return argname, wrapper


name, wrap = register("C")
value = wrap(None)
