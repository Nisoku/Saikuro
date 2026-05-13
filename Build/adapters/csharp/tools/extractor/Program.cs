using System;
using System.Text.Json;
using Saikuro.Schema;

// Small CLI to run the C# SchemaExtractor on the current assembly and
// print JSON to stdout so parity tests can consume it.

class Program
{
    static int Main(string[] args)
    {
        try
        {
            var ns = args.Length > 0 ? args[0] : "parityns";
            // For parity tests we extract schema from a small in-process fixture
            // type (FixtureService) that mirrors the TypeScript/Python fixtures.
            var schema = SchemaExtractorExtensions.ExtractSchema<FixtureService>(ns);
            var opts = new JsonSerializerOptions { WriteIndented = false };
            Console.WriteLine(JsonSerializer.Serialize(schema, opts));
            return 0;
        }
        catch (Exception ex)
        {
            Console.Error.WriteLine($"Error: {ex.Message}");
            return 1;
        }
    }
}
