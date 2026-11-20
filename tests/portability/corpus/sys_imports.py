# import 面：常用模块可导入且属性稳定
import collections
import dataclasses
import enum
import itertools
import json
import math
import os
import re
import socket
import sys
import time

names = ["collections", "dataclasses", "enum", "itertools", "json",
         "math", "os", "re", "socket", "sys", "time"]
print("imports", sorted(names))
print("modules", all(name in sys.modules for name in names))
print("json_version", json.__name__)
print("PATHMODULE=" + os.path.__name__)
