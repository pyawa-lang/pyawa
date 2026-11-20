# 显式平台查询：如实输出（比对时按归一化标签处理）
import os
import sys

print("PLATFORM=" + sys.platform)
print("OSNAME=" + os.name)
print("maxsize", sys.maxsize)
print("byteorder", sys.byteorder)
print("version_info", sys.version_info[:3])
print("CWD=" + os.getcwd())
