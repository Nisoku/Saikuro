using System.Text.Json;
using System.Threading.Tasks;
using Saikuro;

namespace InsightLab;

public class Program
{
    public static async Task Main(string[] args)
    {
        var channel = args.Length > 0 ? args[0] : "saikuro-insight-lab";

        var transport = new WasmHostTransport(channel);
        await transport.ConnectAsync();

        var provider = new SaikuroProvider("csharp");
        provider.Register("summary", (System.Collections.Generic.IReadOnlyList<object?> args) =>
        {
            var json = args.Count > 0 ? JsonSerializer.Serialize(args[0]) : "{}";
            return SummaryEngine.ComputeSummary(json);
        }, new RegisterOptions
        {
            Args = new[] { new ArgDescriptor { Name = "data" } },
        });

        await provider.ServeOnAsync(transport);
    }
}
