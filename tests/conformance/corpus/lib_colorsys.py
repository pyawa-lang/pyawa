# `MS-13` ② 的 `Lib/` 语料：被测代码即逐字同步的 `Lib/colorsys.py`（纯函数、无 import）。
import colorsys

hls = str(colorsys.rgb_to_hls(0.2, 0.4, 0.6))
rgb = str(colorsys.hls_to_rgb(0.5833333333333334, 0.4, 0.49999999999999994))
hsv = str(colorsys.rgb_to_hsv(0.1, 0.2, 0.3))
yiq = str(colorsys.rgb_to_yiq(0.1, 0.2, 0.3))
back = str(colorsys.yiq_to_rgb(0.2, 0.1, -0.1))
