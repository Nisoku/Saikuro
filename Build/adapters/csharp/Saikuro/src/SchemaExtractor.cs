// Saikuro Schema Introspection for C#
//
// Uses .NET reflection to extract function metadata from C# assemblies during
// development mode. This enables automatic schema discovery without requiring
// manual registration of every function.
//
// The extractor:
// 1. Loads assemblies and scans for public methods
// 2. Extracts parameter types, return types, XML documentation
// 3. Handles complex types: generics, nullable, arrays, collections
// 4. Processes attributes for capabilities, visibility, etc.
// 5. Builds a schema announcement compatible with the Saikuro runtime
//
// Usage:
//
//   var extractor = new SchemaExtractor();
//   extractor.AddAssembly(typeof(MyService).Assembly);
//   var schema = extractor.BuildSchema("my-namespace");
//   // schema is ready for announcement to Saikuro runtime
//
// For XML documentation extraction, include the XML documentation file
// alongside your assembly.
//
//   extractor.AddXmlDocumentation("MyService.xml");

using System.Reflection;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Text.RegularExpressions;
using Saikuro;

namespace Saikuro.Schema;


/// <summary>
/// Convert a .NET Type to a Saikuro TypeDescriptor.
/// Handles primitives, arrays, collections, generics, nullable, and special types.
/// </summary>
public static class TypeDescriptorFactory
{
    public static TypeDescriptor FromSystemType(Type type)
    {
        if (type is null)
            return new TypeDescriptor.Primitive("any");

        // Handle nullable
        if (Nullable.GetUnderlyingType(type) != null)
        {
            return new TypeDescriptor.Optional(
                FromSystemType(Nullable.GetUnderlyingType(type)!));
        }

        // Handle arrays
        if (type.IsArray)
        {
            return new TypeDescriptor.List(FromSystemType(type.GetElementType()!));
        }

        // Handle generic types
        if (type.IsGenericType)
        {
            var genericDef = type.GetGenericTypeDefinition();

            // Task<T> / ValueTask<T> - unwrap
            if (genericDef == typeof(Task<>) || genericDef == typeof(ValueTask<>))
                return FromSystemType(type.GetGenericArguments()[0]);

            // IAsyncEnumerable<T> - stream
            if (genericDef == typeof(IAsyncEnumerable<>))
            {
                var itemType = type.GetGenericArguments().FirstOrDefault() ?? typeof(object);
                return new TypeDescriptor.Stream(FromSystemType(itemType));
            }

            // IEnumerable<T> - list
            if (genericDef == typeof(IEnumerable<>))
            {
                var itemType = type.GetGenericArguments().FirstOrDefault() ?? typeof(object);
                return new TypeDescriptor.List(FromSystemType(itemType));
            }

            // IDictionary / Dictionary
            if (genericDef == typeof(IDictionary<,>) || genericDef == typeof(Dictionary<,>))
            {
                var args = type.GetGenericArguments();
                return new TypeDescriptor.Map(
                    args.Length >= 1 ? FromSystemType(args[0]) : new TypeDescriptor.Primitive("string"),
                    args.Length >= 2 ? FromSystemType(args[1]) : new TypeDescriptor.Primitive("any"));
            }

            // ICollection<T>, IList<T>, List<T>
            if (genericDef == typeof(ICollection<>)
                || genericDef == typeof(IList<>)
                || genericDef == typeof(List<>))
            {
                var itemType = type.GetGenericArguments().FirstOrDefault() ?? typeof(object);
                return new TypeDescriptor.List(FromSystemType(itemType));
            }

            // Channel<T>
            if (type.Name.Contains("Channel"))
            {
                var args = type.GetGenericArguments();
                return new TypeDescriptor.Channel(
                    args.Length >= 1 ? FromSystemType(args[0]) : new TypeDescriptor.Primitive("any"),
                    args.Length >= 2 ? FromSystemType(args[1]) : new TypeDescriptor.Primitive("any"));
            }
        }

        // Handle special interfaces
        if (type.IsInterface)
        {
            if (type.Name.StartsWith("IAsyncEnumerable"))
            {
                var itemType = type.GetGenericArguments().FirstOrDefault() ?? typeof(object);
                return new TypeDescriptor.Stream(FromSystemType(itemType));
            }
            if (type.Name.StartsWith("IDictionary"))
            {
                var args = type.GetGenericArguments();
                return new TypeDescriptor.Map(
                    args.Length >= 1 ? FromSystemType(args[0]) : new TypeDescriptor.Primitive("string"),
                    args.Length >= 2 ? FromSystemType(args[1]) : new TypeDescriptor.Primitive("any"));
            }
            if (type.Name.StartsWith("IEnumerable"))
            {
                return new TypeDescriptor.List(new TypeDescriptor.Primitive("any"));
            }
        }

        // Handle primitives
        return PrimitiveFromType(type);
    }

    private static TypeDescriptor PrimitiveFromType(Type type)
    {
        var underlying = Nullable.GetUnderlyingType(type) ?? type;

        if (underlying == typeof(bool))
            return new TypeDescriptor.Primitive("bool");
        if (underlying == typeof(sbyte) || underlying == typeof(byte)
            || underlying == typeof(short) || underlying == typeof(ushort)
            || underlying == typeof(int) || underlying == typeof(uint)
            || underlying == typeof(long) || underlying == typeof(ulong))
            return new TypeDescriptor.Primitive("i64");
        if (underlying == typeof(float) || underlying == typeof(double) || underlying == typeof(decimal))
            return new TypeDescriptor.Primitive("f64");
        if (underlying == typeof(string))
            return new TypeDescriptor.Primitive("string");
        if (underlying == typeof(byte[]) || underlying == typeof(Memory<byte>) || underlying == typeof(ReadOnlyMemory<byte>))
            return new TypeDescriptor.Primitive("bytes");
        if (underlying == typeof(void))
            return new TypeDescriptor.Primitive("unit");

        return new TypeDescriptor.Named(type.Name);
    }
}

// Extracted Function Metadata 

/// <summary>A single extracted argument/parameter.</summary>
public class ExtractedArg
{
    [JsonPropertyName("name")]
    public string Name { get; init; } = "";

    [JsonPropertyName("type")]
    public TypeDescriptor Type { get; init; } = new TypeDescriptor.Primitive("any");

    [JsonPropertyName("optional")]
    public bool Optional { get; init; }

    [JsonPropertyName("default")]
    public string? DefaultValue { get; init; }

    [JsonPropertyName("doc")]
    public string? Doc { get; init; }
}

/// <summary>An extracted function with its metadata.</summary>
public class ExtractedFunction
{
    [JsonPropertyName("name")]
    public string Name { get; init; } = "";

    [JsonPropertyName("args")]
    public List<ExtractedArg> Args { get; init; } = new();

    [JsonPropertyName("returns")]
    public TypeDescriptor Returns { get; init; } = new TypeDescriptor.Primitive("any");

    [JsonPropertyName("capabilities")]
    public List<string> Capabilities { get; init; } = new();

    [JsonPropertyName("visibility")]
    public string Visibility { get; init; } = "public";

    [JsonPropertyName("doc")]
    public string? Doc { get; init; }

    [JsonPropertyName("isAsync")]
    public bool IsAsync { get; init; }

    [JsonPropertyName("isGenerator")]
    public bool IsGenerator { get; init; }
}

// XML Documentation Parser

/// <summary>
/// Parses XML documentation files to extract method and parameter comments.
/// </summary>
public class XmlDocumentationParser
{
    private readonly Dictionary<string, string> _memberDocs = new();
    private readonly Dictionary<string, Dictionary<string, string>> _paramDocs = new();

    public XmlDocumentationParser(string xmlPath)
    {
        if (!File.Exists(xmlPath))
            return;

        var content = File.ReadAllText(xmlPath);
        ParseXml(content);
    }

    private void ParseXml(string content)
    {
        // Simple XML parsing - look for <member> elements with name and <summary>/<param>
        var memberRegex = new System.Text.RegularExpressions.Regex(
            @"<member\s+name=""([^""]+)""[^>]*>[\s\S]*?</member>",
            System.Text.RegularExpressions.RegexOptions.Compiled
        );

        foreach (System.Text.RegularExpressions.Match match in memberRegex.Matches(content))
        {
            var name = match.Groups[1].Value;
            var body = match.Groups[0].Value;

            // Extract summary
            var summaryMatch = System.Text.RegularExpressions.Regex.Match(
                body,
                @"<summary[^>]*>([\s\S]*?)</summary>"
            );
            if (summaryMatch.Success)
            {
                var summary = CleanXmlText(summaryMatch.Groups[1].Value);
                if (!string.IsNullOrWhiteSpace(summary))
                {
                    _memberDocs[name] = summary;
                }
            }

            // Extract param docs
            var paramMatches = System.Text.RegularExpressions.Regex.Matches(
                body,
                @"<param\s+name=""([^""]+)""[^>]*>([\s\S]*?)</param>"
            );
            if (paramMatches.Count > 0)
            {
                var paramDocs = new Dictionary<string, string>();
                foreach (System.Text.RegularExpressions.Match pm in paramMatches)
                {
                    var paramName = pm.Groups[1].Value;
                    var paramDoc = CleanXmlText(pm.Groups[2].Value);
                    if (!string.IsNullOrWhiteSpace(paramDoc))
                    {
                        paramDocs[paramName] = paramDoc;
                    }
                }
                if (paramDocs.Count > 0)
                {
                    _paramDocs[name] = paramDocs;
                }
            }
        }
    }

    private static string CleanXmlText(string text)
    {
        return Regex.Replace(text, @"\s+", " ").Trim();
    }

    public string? GetMemberDoc(string fullMemberName)
    {
        return _memberDocs.TryGetValue(fullMemberName, out var doc) ? doc : null;
    }

    public Dictionary<string, string>? GetParamDocs(string fullMemberName)
    {
        return _paramDocs.TryGetValue(fullMemberName, out var docs) ? docs : null;
    }
}

// Schema Extractor

/// <summary>
/// Extracts function metadata from C# assemblies using reflection.
/// Supports XML documentation for rich metadata.
/// </summary>
public class SchemaExtractor
{
    private readonly List<Assembly> _assemblies = new();
    private readonly Dictionary<string, XmlDocumentationParser> _xmlDocs = new();

    /// <summary>
    /// Add an assembly to be scanned for exported functions.
    /// </summary>
    public SchemaExtractor AddAssembly(Assembly assembly)
    {
        _assemblies.Add(assembly);
        return this;
    }

    /// <summary>
    /// Add an assembly by type - convenient shortcut.
    /// </summary>
    public SchemaExtractor AddAssemblyContaining<T>()
    {
        return AddAssembly(typeof(T).Assembly);
    }

    /// <summary>
    /// Add XML documentation file for the previously added assemblies.
    /// Call after AddAssembly() for the matching assembly.
    /// </summary>
    public SchemaExtractor AddXmlDocumentation(string xmlPath)
    {
        if (File.Exists(xmlPath))
        {
            var parser = new XmlDocumentationParser(xmlPath);
            foreach (var assembly in _assemblies)
            {
                _xmlDocs[assembly.FullName ?? assembly.GetName().Name!] = parser;
            }
        }
        return this;
    }

    /// <summary>
    /// Add XML documentation with explicit assembly name key.
    /// </summary>
    public SchemaExtractor AddXmlDocumentation(string assemblyName, string xmlPath)
    {
        if (File.Exists(xmlPath))
        {
            _xmlDocs[assemblyName] = new XmlDocumentationParser(xmlPath);
        }
        return this;
    }

    /// <summary>
    /// Extract all public methods from added assemblies.
    /// </summary>
    public List<ExtractedFunction> Extract()
    {
        var functions = new List<ExtractedFunction>();

        foreach (var assembly in _assemblies)
        {
            var types = assembly.GetExportedTypes();
            var docParser = _xmlDocs.GetValueOrDefault(
                assembly.FullName ?? assembly.GetName().Name!
            );

            foreach (var type in types)
            {
                // Skip static classes (could be revisited for static method exports)
                if (type.IsAbstract && type.IsSealed)
                    continue; // Static class

                // Get public instance methods
                var methods = type.GetMethods(
                    BindingFlags.Public | BindingFlags.Instance | BindingFlags.DeclaredOnly
                );

                foreach (var method in methods)
                {
                    var fn = ExtractMethod(method, type, docParser);
                    if (fn != null)
                    {
                        functions.Add(fn);
                    }
                }
            }
        }

        return functions;
    }

    private ExtractedFunction? ExtractMethod(
        MethodInfo method,
        Type declaringType,
        XmlDocumentationParser? docParser
    )
    {
        // Skip property accessors, operators, special methods
        if (method.IsSpecialName)
            return null;
        if (method.Name.StartsWith("get_") || method.Name.StartsWith("set_"))
            return null;
        if (method.Name.StartsWith("add_") || method.Name.StartsWith("remove_"))
            return null;

        var fullMemberName = $"M:{declaringType.FullName}.{method.Name}";

        // Build parameter list
        var args = new List<ExtractedArg>();
        var parameters = method.GetParameters();

        var paramDocs = docParser?.GetParamDocs(fullMemberName);

        foreach (var param in parameters)
        {
            args.Add(
                new ExtractedArg
                {
                    Name = param.Name ?? "",
                    Type = TypeDescriptorFactory.FromSystemType(param.ParameterType),
                    Optional = param.IsOptional || param.ParameterType.IsByRef,
                    DefaultValue = param.HasDefaultValue
                        ? (param.DefaultValue?.ToString() ?? "null")
                        : null,
                    Doc = paramDocs?.GetValueOrDefault(param.Name ?? ""),
                }
            );
        }

        // Determine return type
        var returnType = method.ReturnType;
        var isAsync =
            returnType.IsGenericType && returnType.GetGenericTypeDefinition() == typeof(Task<>);
        var isGenerator =
            typeof(IAsyncEnumerable<>).IsAssignableFrom(returnType)
            || typeof(IEnumerable<>).IsAssignableFrom(returnType);

        TypeDescriptor returns;
        if (returnType == typeof(void))
        {
            returns = new TypeDescriptor.Primitive("unit");
        }
        else if (isAsync)
        {
            var innerType = returnType.GetGenericArguments()[0];
            returns = TypeDescriptorFactory.FromSystemType(innerType);
        }
        else if (isGenerator)
        {
            var innerType = returnType.GetGenericArguments().FirstOrDefault() ?? typeof(object);
            returns = new TypeDescriptor.Stream(TypeDescriptorFactory.FromSystemType(innerType));
        }
        else
        {
            returns = TypeDescriptorFactory.FromSystemType(returnType);
        }

        // Get capabilities from attributes and function-level metadata
        var capabilities = new List<string>();
        var capAttr = method.GetCustomAttribute<SaikuroCapabilityAttribute>();
        if (capAttr != null)
            capabilities.Add(capAttr.Capability);

        // Check for SaikuroFunctionAttribute for explicit metadata
        var fnAttr = method.GetCustomAttribute<SaikuroFunctionAttribute>();
        var visibility = fnAttr?.Visibility ?? "public";

        // Get method documentation: prefer XML docs, fall back to attribute Doc
        var doc = docParser?.GetMemberDoc(fullMemberName) ?? fnAttr?.Doc;

        return new ExtractedFunction
        {
            Name = method.Name,
            Args = args,
            Returns = returns,
            Capabilities = capabilities,
            Visibility = visibility,
            Doc = doc,
            IsAsync = isAsync,
            IsGenerator = isGenerator,
        };
    }

    /// <summary>
    /// Build a Saikuro schema announcement from extracted functions.
    /// </summary>
    public Dictionary<string, object?> BuildSchema(string namespaceName)
    {
        var functions = Extract();

        var schemaFunctions = new Dictionary<string, object?>();
        foreach (var fn in functions)
        {
            var argList = fn
            .Args.Select(arg => new Dictionary<string, object?>
            {
                [WireKey.Name] = arg.Name,
                [WireKey.Type] = arg.Type.ToWire(),
                [WireKey.Optional] = arg.Optional,
            })
                .ToList();

            var returns = fn.IsGenerator
                ? new Dictionary<string, object?>
                {
                    [WireKey.Kind] = "stream",
                    [WireKey.Item] = fn.Returns.ToWire(),
                }
                : fn.Returns.ToWire();

            var schemaFn = new Dictionary<string, object?>
            {
                [WireKey.Args] = argList,
                [WireKey.Returns] = returns,
                [WireKey.Visibility] = fn.Visibility,
                [WireKey.Capabilities] = fn.Capabilities,
            };

            if (!string.IsNullOrEmpty(fn.Doc))
            {
                schemaFn[WireKey.Doc] = fn.Doc;
            }

            schemaFunctions[fn.Name] = schemaFn;
        }

        return WireKey.BuildSchemaDict(namespaceName, schemaFunctions);
    }

    private static object SerializeType(TypeDescriptor type) => type.ToWire();
}

// Attributes

/// <summary>
/// Declare a capability required to invoke a function.
/// </summary>
[AttributeUsage(AttributeTargets.Method, AllowMultiple = true)]
public class SaikuroCapabilityAttribute : Attribute
{
    public string Capability { get; }

    public SaikuroCapabilityAttribute(string capability) => Capability = capability;
}

/// <summary>
/// Additional metadata for a Saikuro function.
/// </summary>
[AttributeUsage(AttributeTargets.Method)]
public class SaikuroFunctionAttribute : Attribute
{
    public string? Doc { get; init; }
    public string Visibility { get; init; } = "public";
    public bool Idempotent { get; init; }
}

// Convenience Extensions

public static class SchemaExtractorExtensions
{
    /// <summary>
    /// Convenience method to quickly extract schema from a type's assembly.
    /// </summary>
    public static Dictionary<string, object> ExtractSchema<T>(string namespaceName)
    {
        return new SchemaExtractor().AddAssemblyContaining<T>().BuildSchema(namespaceName);
    }

    /// <summary>
    /// Extract schema with XML documentation.
    /// </summary>
    public static Dictionary<string, object> ExtractSchemaWithDocs<T>(
        string namespaceName,
        string xmlPath
    )
    {
        return new SchemaExtractor()
            .AddAssemblyContaining<T>()
            .AddXmlDocumentation(typeof(T).Assembly.GetName().Name!, xmlPath)
            .BuildSchema(namespaceName);
    }
}
