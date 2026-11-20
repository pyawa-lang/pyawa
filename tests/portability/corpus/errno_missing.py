# OSError 的可移植面：errno 语义名（WinError 只在 Windows）
import errno
import os

try:
    open("definitely-missing-file.txt", "r", encoding="utf-8")
except OSError as exc:
    print("ERRNO=" + str(errno.errorcode.get(exc.errno, "UNKNOWN")))
    print("WINERROR=" + str(getattr(exc, "winerror", "none")))
    print("name", type(exc).__name__)
else:
    print("unexpected")
