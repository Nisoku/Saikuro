---
type: concept
title: "C++ Examples"
description: "C++ adapter usage patterns"
source: "https://nisoku.org/Saikuro/adapters/cpp/examples/"
path: /adapters/cpp/examples/
updated: 2026-06-27
okf:
  generated_by: "@docmd/plugin-okf"
  generated_at: "2026-06-27T13:11:26.468Z"
---
---
title: "C++ Examples"
description: "C++ adapter usage patterns"
---

## Math Provider

```cpp
#include "saikuro/saikuro.hpp"
#include <iostream>

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

## Client

```cpp
#include "saikuro/saikuro.hpp"
#include <iostream>

int main() {
    saikuro::Client client("unix:///tmp/saikuro.sock");

    std::string result = client.call_json("math.add", "[10, 32]");
    std::cout << "10 + 32 = " << result << std::endl;

    return 0;
}
```

## Stream

```cpp
#include "saikuro/saikuro.hpp"
#include <iostream>

int main() {
    saikuro::Client client("unix:///tmp/saikuro.sock");

    auto stream = client.stream_json("events.tick", "[5]");
    std::string item;
    while (stream.next_json(item)) {
        std::cout << "tick: " << item << std::endl;
    }

    return 0;
}
```

## Channel

```cpp
#include "saikuro/saikuro.hpp"
#include <iostream>

int main() {
    saikuro::Client client("unix:///tmp/saikuro.sock");

    auto channel = client.channel_json("chat.room", "[\"lobby\"]");
    channel.send_json("\"hello\"");

    std::string reply;
    channel.next_json(reply);
    std::cout << "reply: " << reply << std::endl;

    channel.close();
    return 0;
}
```

## Batch

```cpp
#include "saikuro/saikuro.hpp"
#include <iostream>

int main() {
    saikuro::Client client("unix:///tmp/saikuro.sock");

    std::string results = client.batch_json(
        "[[\"math.add\", [1, 2]], [\"math.multiply\", [3, 4]]]");
    std::cout << "batch: " << results << std::endl;

    return 0;
}
```
