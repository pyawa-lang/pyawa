# `MS-13` ② 的 `Lib/` 语料（第三批补充）：被测代码即逐字同步的 `Lib/colorsys.py`。
import colorsys

back = str(colorsys.hls_to_rgb(*colorsys.rgb_to_hls(0.2, 0.4, 0.6)))
yiq_back = str(colorsys.yiq_to_rgb(*colorsys.rgb_to_yiq(0.1, 0.2, 0.3)))
