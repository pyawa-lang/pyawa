# `MS-13` ② 的 `Lib/test/` 语料：`_STRATEGIES` 正文**逐字**取自
# `Lib/test/support/_hypothesis_stubs/strategies.py`（第 25–68 行）。
# 下面是脚手架：`sorted()` 是必须的（`set` 迭代序未规定 ⇒ `MS-19`）。
_STRATEGIES = {
    "binary",
    "booleans",
    "builds",
    "characters",
    "complex_numbers",
    "composite",
    "data",
    "dates",
    "datetimes",
    "decimals",
    "deferred",
    "dictionaries",
    "emails",
    "fixed_dictionaries",
    "floats",
    "fractions",
    "from_regex",
    "from_type",
    "frozensets",
    "functions",
    "integers",
    "iterables",
    "just",
    "lists",
    "none",
    "nothing",
    "one_of",
    "permutations",
    "random_module",
    "randoms",
    "recursive",
    "register_type_strategy",
    "runner",
    "sampled_from",
    "sets",
    "shared",
    "slices",
    "timedeltas",
    "times",
    "text",
    "tuples",
    "uuids",
}


n = len(_STRATEGIES)
has = "integers" in _STRATEGIES
top = str(sorted(_STRATEGIES)[0])
