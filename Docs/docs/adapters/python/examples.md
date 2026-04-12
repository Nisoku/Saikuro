---
title: "Python Adapter Examples"
description: "Python-centered cross-language patterns"
---

## Python Provider, TypeScript Caller

Python hosts inference; TypeScript API consumes it.

```python
from saikuro import Provider

provider = Provider(namespace="model")

@provider.register("predict")
async def predict(features):
    score = model.predict(features)
    return {"score": float(score), "label": classify(score)}

await provider.serve()
```

```typescript
import { Client } from "@nisoku/saikuro";

const client = new Client();
await client.connect();
const result = await client.call("model.predict", [features]);
return res.json(result);
```

## Chat room channels

Python handles room state while browser clients join via WebSocket channels.

```python
@provider.register_channel("join")
async def join_room(args, chan):
    room = args[0]["roomId"]
    async for msg in chan.incoming():
        await broadcast(room, msg)
```

## Next Steps

- [Python Adapter](./)
- [TypeScript examples](../typescript/examples)
- [Transports](../../guide/transports)