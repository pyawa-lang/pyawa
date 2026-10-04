# with（第 230／231 轮）：__enter__／__exit__ 次序、as 目标、多项、异常路径
class Log:
    def __init__(self):
        self.enters = 0
        self.exits = 0
        self.last_exit = 0
class CM:
    def __init__(self, log, tag):
        self.log = log
        self.tag = tag
    def __enter__(self):
        self.log.enters = self.log.enters + 1
        return self.tag
    def __exit__(self, kind, value, tb):
        self.log.exits = self.log.exits + 1
        self.log.last_exit = self.tag
        return False
log = Log()
entered = 0
with CM(log, 1) as first:
    entered = first
path = 0
try:
    with CM(log, 2) as second:
        entered = entered + second
        x = 1 // 0
except:
    path = 1
pair = 0
with CM(log, 3) as third, CM(log, 4) as fourth:
    pair = third + fourth
enter_total = log.enters
exit_total = log.exits
last_exit = log.last_exit
