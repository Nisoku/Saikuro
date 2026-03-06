import json
from typing import AsyncIterator, Optional

from adapters.python.saikuro import schema as s

sb = s.SchemaBuilder("parityns")


def add(a: int, b: int) -> int:
    return a + b


async def gen_numbers(count: int) -> AsyncIterator[int]:
    for i in range(count):
        yield i


def maybe(msg: Optional[str] = None) -> Optional[str]:
    return msg


sb.add_function("add", add, ["calc"], "")
sb.add_function("gen_numbers", gen_numbers, [], "")
sb.add_function("maybe", maybe, [], "Optional return example")


def sum_values(m: dict[str, int]) -> int:
    s = 0
    for k in m:
        s += m[k]
    return s


def wrap_items(items: list[int]) -> list[int]:
    return items


sb.add_function("sum_values", sum_values, [], "")
sb.add_function("wrap_items", wrap_items, [], "")


def union_echo(val: int | str) -> str:
    return str(val)


class Person:
    def __init__(self, name: str, age: int):
        self.name = name
        self.age = age


def greet(p: Person) -> str:
    return f"hello {p.name}"


def optional_arg(x: int | None = None) -> int | None:
    return x


sb.add_function("union_echo", union_echo, [], "")
sb.add_function("greet", greet, [], "")
sb.add_function("optional_arg", optional_arg, [], "")


if __name__ == "__main__":
    print(json.dumps(sb.build()))
