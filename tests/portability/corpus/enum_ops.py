# enum：成员与值
import enum

class Color(enum.Enum):
    RED = 1
    GREEN = 2

print("value", Color.RED.value, Color.RED.name)
print("members", [(member.name, member.value) for member in Color])
print("lookup", Color(2).name)
print("unique", len({Color.RED, Color.GREEN}))
