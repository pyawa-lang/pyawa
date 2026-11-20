# dataclasses：字段序与 repr/asdict
import dataclasses

@dataclasses.dataclass
class Point:
    x: int
    y: int = 0
    label: str = "p"

point = Point(1, 2)
print("repr", repr(point))
print("asdict", dataclasses.asdict(point))
print("fields", [field.name for field in dataclasses.fields(point)])
print("replace", dataclasses.replace(point, y=9))
