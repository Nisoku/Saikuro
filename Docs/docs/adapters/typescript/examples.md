---
title: "TypeScript Adapter Examples"
description: "TypeScript-centered cross-language patterns"
---

## TypeScript Provider, Python Caller

TypeScript exposes user operations. Python performs periodic cleanup.

```typescript
import { Provider } from "@nisoku/saikuro";

const provider = new Provider({ namespace: "users" });
provider.register("list", async (options: { page: number; limit: number }) => {
  return db.users.paginate(options);
});
provider.register("deactivate", async (id: string) => {
  await db.users.update(id, { active: false });
  return { ok: true };
});
await provider.serve();
```

```python
from saikuro import Client

client = Client()
await client.connect()
result = await client.call("users.list", [{"page": 0, "limit": 100}])
for user in result["items"]:
    if should_deactivate(user):
        await client.call("users.deactivate", [user["id"]])
```

## Batch UI hydration

Use one `batch` for first paint data:

```typescript
const [user, prefs, notifications] = await client.batch([
  { target: "users.getById", args: ["u1"] },
  { target: "prefs.getForUser", args: ["u1"] },
  { target: "notifications.getUnread", args: ["u1"] },
]);
```

## Next Steps

- [TypeScript Adapter](./)
- [Python examples](../python/examples)
- [Invocation Primitives](../../guide/invocations)