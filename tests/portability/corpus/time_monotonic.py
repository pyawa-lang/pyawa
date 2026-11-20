# 单调钟：值不进比对，只比性质
import time

a = time.monotonic()
b = time.monotonic()
print("monotonic", b >= a)
print("TIME=" + str(a))
