# 语言：异常与消息（不打印 traceback/路径）
class Boom(Exception):
    pass

steps = []
try:
    try:
        raise Boom("bang")
    except Boom as exc:
        steps.append("caught:" + str(exc))
        raise ValueError("wrapped") from exc
    finally:
        steps.append("finally")
except ValueError as exc:
    steps.append("outer:" + str(exc))
    steps.append("cause:" + type(exc.__cause__).__name__)
    steps.append("args:" + repr(exc.args))
for step in steps:
    print(step)
