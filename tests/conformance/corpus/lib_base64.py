# `MS-13` ② 的 `Lib/` 语料：逐字同步的 `Lib/base64.py`（bytes ↔ str 的编解码面）。
import base64

encoded = base64.b64encode(b"hello world").decode()
decoded = base64.b64decode("aGVsbG8gd29ybGQ=").decode()
url = base64.urlsafe_b64encode(b"\xfb\xff").decode()
