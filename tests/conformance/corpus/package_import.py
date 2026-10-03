# 包与子模块导入（第 135 轮）：加载器此前只认 <dir>/<名字>.py ✗
import pkgsample
import pkgsample.helper

assert pkgsample.VALUE == 41
assert pkgsample.helper.add(1, 2) == 3
print("ok")
