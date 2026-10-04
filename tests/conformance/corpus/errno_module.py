# errno（第 134 轮）：模块本体与静态映射不依赖宿主注入 ✓
# （宿主的 E* 常量按 CM-20 由 runtime 注入 ✓ —— CLI 那条路已注入 ✓；
#  harness 属 pyawa-abi，分层上拿不到 runtime 的表 ✗ ⇒ 语料只守这里 ✓）
import errno

assert errno.errorcode[2] == "ENOENT"
assert errno.errorcode[17] == "EEXIST"
assert errno.errorcode[2] != errno.errorcode[17]
print("ok")
