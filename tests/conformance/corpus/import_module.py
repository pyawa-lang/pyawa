# 加载器（第 98／99 轮）：`import <模块>` 与 `from <模块> import <名字>` 两侧语义都要一致 ✓
# 搜索路径：harness 给被测侧设语料目录 ✓、给参照侧设同目录的 `PYTHONPATH` ✓
import corpus_helper
print(corpus_helper.value)
from corpus_helper import value
print(value)
