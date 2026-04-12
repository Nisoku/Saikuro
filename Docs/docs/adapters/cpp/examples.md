---
title: "C++ Adapter Examples"
description: "C++ RAII usage patterns over the C ABI"
---

## RAII client call

```cpp
#include <saikuro/saikuro.hpp>
#include <iostream>

int main() {
    saikuro::Client client("tcp://127.0.0.1:7700");
    const auto result = client.call_json("math.add", "[10, 32]");
    std::cout << result << "\n";
}
```

## Batch and stream usage

```cpp
const auto results = client.batch_json(R"([
  {"target":"math.add","args":[1,2]},
  {"target":"math.add","args":[3,4]}
])");

auto stream = client.stream_json("events.subscribe", R"(["error"])" );
while (auto item = stream.next_json()) {
    std::cout << *item << "\n";
}
```

## Ownership boundary

Use the C++ wrapper whenever possible; it centralizes C string allocation/free behavior and prevents manual lifetime bugs.

## Next Steps

- [C++ Adapter](./)
- [C Adapter](../c/)
- [Schema](../../guide/schema)