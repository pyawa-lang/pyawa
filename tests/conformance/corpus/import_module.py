# 加载器（第 98／99／100 轮）：`import`／`from … import`／`from … import *` 三种形态两侧语义一致 ✓
# 搜索路径：harness 给被测侧设语料目录 ✓、给参照侧设同目录的 `PYTHONPATH` ✓
import corpus_helper
print(corpus_helper.value)
from corpus_helper import value
print(value)
from corpus_helper import *
print(value)
