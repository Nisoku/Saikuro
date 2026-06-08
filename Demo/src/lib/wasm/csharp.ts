import { getLogger } from "@nisoku/saikuro";

const log = getLogger("demo.wasm.csharp");

let bootPromise: Promise<void> | null = null;

export async function startCSharpProvider(channel: string): Promise<void> {
  if (!bootPromise) {
    bootPromise = (async () => {
      log.info("loading C# WASM (dotnet.js)", { channel });

      // Load the Saikuro BroadcastChannel JS module.
      // This registers globalThis functions that the C# [JSImport] bindings
      // call for channel creation, handshake, send, and message dequeue.
      await import("../../../public/wasm/csharp/Saikuro.BroadcastChannel.js");

      // Load dotnet.js from the public path (not through Vite's bundler) so
      // that import.meta.url inside it resolves to /wasm/csharp/ instead of
      // assets/
      const dotnetUrl = new URL(
        "wasm/csharp/dotnet.js",
        document.baseURI,
      ).href;
      const { dotnet } = await import(dotnetUrl);
      log.info("C# dotnet.js loaded, creating runtime");

      // Use the builder API to configure the entry point and create the
      // runtime.
      const dotnetApi: any = await (dotnet as any)
        .withMainAssembly("InsightLab.dll")
        .withApplicationArguments(channel)
        .create();

      log.info("C# runtime created, starting provider", { channel });

      // Verify the JS interop functions that the C# code relies on are
      // registered on globalThis.
      const requiredFns = [
        "Saikuro_ConnectToRuntime",
        "Saikuro_DequeueRuntimeMessage",
        "Saikuro_SendRuntime",
        "Saikuro_CloseRuntime",
      ];
      for (const fn of requiredFns) {
        if (typeof (globalThis as any)[fn] !== "function") {
          log.warn("missing globalThis function", { fn });
        }
      }

      // Start Program.Main in the background.  This connects the C# provider
      // to the Rust runtime via BroadcastChannel, announces the "csharp"
      // namespace, and enters the serve loop.  We use runMain (not
      // runMainAndExit) so Main runs without mono_exit.
      dotnetApi.runMain("InsightLab.dll", [channel]).catch((e: unknown) => {
        const message = e instanceof Error ? e.message : String(e);
        const details =
          e instanceof Error
            ? { stack: e.stack, name: e.name, message: e.message }
            : { raw: e };
        log.error("C# dotnet runtime error", { error: message, details });
      });
    })();
  }
  return bootPromise;
}
