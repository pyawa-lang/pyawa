# **`math`**（第 336 轮）：纯 Rust `f64` ⇒ 不碰平台、不碰能力域（`CX-4`）。
# 上限榜上 `ModuleNotFoundError: No module named 'math'` 那一族卡它；`Lib/` 里大量模块导入期就取它的名字。
# **口径照参照实测**：`floor`／`ceil`／`trunc` 返回 `int`；定义域错的消息逐条量过（3.14 的措辞比
# "math domain error" 细）；`int` 与 `float` 都算"实数"。
# **如实登记的未落地面**：`frexp`／`modf`／`ldexp`／`nextafter`／`ulp` 等（真撞上再补，按 `CM-6`
# 如实报缺失，不给错值）。

import math

print(str(math.pi))
print(str(math.e))
print(str(math.tau))
print(str(math.sqrt(2)))
print(str(math.sqrt(16)))
print(str(math.floor(2.7)))
print(str(math.ceil(2.1)))
print(str(math.trunc(-2.7)))
print(str(type(math.floor(2.7)).__name__))
print(str(math.pow(2, 10)))
print(str(math.log(math.e)))
print(str(math.log(8, 2)))
print(str(math.fmod(7, 3)))
print(str(math.gcd(12, 18)))
print(str(math.gcd(12, 18, 24)))
print(str(math.lcm(4, 6)))
print(str(math.isqrt(17)))
print(str(math.factorial(10)))
print(str(math.isnan(math.nan)))
print(str(math.isinf(math.inf)))
print(str(math.isfinite(1.0)))
print(str(math.hypot(3, 4)))
print(str(math.degrees(math.pi)))
print(str(math.fsum([0.1, 0.2])))
print(str(math.copysign(3, -1)))
print(str(math.atan2(1, 1)))
print(str(math.sinh(0)))
print(str(math.cbrt(-8)))
print(str(math.radians(180)))
print(str(math.exp(0)))
