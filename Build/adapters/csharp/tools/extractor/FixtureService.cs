using System.Collections.Generic;
using System.Threading.Tasks;
using Saikuro.Schema;

/// <summary>
/// Fixture service used by the extractor tool for parity tests.
/// Mirrors the TypeScript/Python parity fixture.
/// </summary>
public class FixtureService
{
    /// <summary>Add two integers.</summary>
    [SaikuroCapability("calc")]
    public Task<long> Add(long a, long b) => Task.FromResult(a + b);

    /// <summary>Generator of numbers.</summary>
    public async IAsyncEnumerable<long> Gen_Numbers(long count)
    {
        for (long i = 0; i < count; i++)
        {
            yield return i;
            await Task.Yield();
        }
    }

    /// <summary>Optional return example</summary>
    [SaikuroFunction(Doc = "Optional return example")]
    public string? Maybe(string? msg = null) => msg;

    /// <summary>Sum values in a dictionary.</summary>
    public long Sum_Values(System.Collections.Generic.IDictionary<string, long> m)
    {
        long s = 0;
        foreach (var kv in m)
        {
            s += kv.Value;
        }
        return s;
    }

    /// <summary>Echo list of numbers.</summary>
    public System.Collections.Generic.List<long> Wrap_Items(
        System.Collections.Generic.List<long> items
    )
    {
        return items;
    }

    public string Union_Echo(object val)
    {
        return val?.ToString() ?? string.Empty;
    }

    public string Greet(Person p)
    {
        return $"hello {p.Name}";
    }

    public long? Optional_Arg(long? x = null)
    {
        return x;
    }
}

public class Person
{
    public string Name { get; set; } = "";
    public long Age { get; set; }
}
