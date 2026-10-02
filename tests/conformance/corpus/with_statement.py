# with（第 230 轮）：__enter__／__exit__ 调用次序与 as 目标（直行路径）
# 注：**异常出口**里 `__exit__` 的调用尚未生效（见 PLAN §9 待做），故本用例不含异常路径
class Log:
    def __init__(self):
        self.enters = 0
        self.exits = 0
class CM:
    def __init__(self, log, tag):
        self.log = log
        self.tag = tag
    def __enter__(self):
        self.log.enters = self.log.enters + 1
        return self.tag
    def __exit__(self, kind, value, tb):
        self.log.exits = self.log.exits + 1
        return False
log = Log()
entered = 0
with CM(log, 1) as first:
    entered = first
enter_total = log.enters
exit_total = log.exits
