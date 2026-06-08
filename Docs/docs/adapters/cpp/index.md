---
title: "C++ Adapter"
description: "Saikuro adapter for C++"
---

The C++ adapter provides a header-only RAII wrapper over the C API.

## Installation

```cpp
#include "saikuro/saikuro.hpp"
// link libsaikuro_cpp.a
```

## Client

```cpp
#include "saikuro/saikuro.hpp"
#include <iostream>

int main() {
    saikuro::Client client("unix:///tmp/saikuro.sock");

    std::string result = client.call_json("math.add", "[10, 32]");
    std::cout << result << std::endl;

    return 0;
}
```

## Provider

```cpp
#include "saikuro/saikuro.hpp"

char* add_handler(void* user_data, const char* args_json) {
    return saikuro_string_dup("42");
}

int main() {
    saikuro::Provider provider("math");
    provider.register_handler("add", add_handler, nullptr);
    provider.serve("unix:///tmp/saikuro.sock");
    return 0;
}
```

## API Reference

See the [C++ API Reference](./api-reference) for the full class reference.
