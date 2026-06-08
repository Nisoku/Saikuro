// Wire-format protocol key constants.
//
// Single source of truth for all string keys used in the MessagePack
// protocol maps.  Every dictionary lookup or construction in this
// adapter must reference these constants, never raw string literals.

namespace Saikuro;

internal static class WireKey
{
    // Envelope
    public const string Version = "version";
    public const string Type = "type";
    public const string Id = "id";
    public const string Target = "target";
    public const string Args = "args";
    public const string Meta = "meta";
    public const string Capability = "capability";
    public const string BatchItems = "batch_items";
    public const string StreamControl = "stream_control";
    public const string Seq = "seq";

    // ResponseEnvelope
    public const string Ok = "ok";
    public const string Result = "result";
    public const string Error = "error";

    // ErrorPayload
    public const string Code = "code";
    public const string Message = "message";
    public const string Details = "details";

    // TypeDescriptor (schema)
    public const string Kind = "kind";
    public const string Item = "item";
    public const string Key = "key";
    public const string Value = "value";
    public const string Inner = "inner";
    public const string Name = "name";
    public const string Send = "send";
    public const string Recv = "recv";

    // Schema announcement root
    public const string Namespaces = "namespaces";
    public const string Functions = "functions";
    public const string Types = "types";

    // Log record
    public const string Ts = "ts";
    public const string Level = "level";
    public const string Msg = "msg";
    public const string Fields = "fields";

    // ResourceHandle
    public const string MimeType = "mime_type";

    // Internal ID conventions
    public const string LogIdPrefix = "log-";
    public const string LogTarget = "$log";
    public const string Size = "size";
    public const string Uri = "uri";

    // Schema arg metadata
    public const string Optional = "optional";
    public const string Doc = "doc";
    public const string Default = "default";

    // Schema function metadata
    public const string Returns = "returns";
    public const string Visibility = "visibility";
    public const string Capabilities = "capabilities";
    public const string Idempotent = "idempotent";

    /// <summary>Build the outer schema announcement dictionary.</summary>
    internal static Dictionary<string, object?> BuildSchemaDict(
        string ns, Dictionary<string, object?> functions) => new()
        {
            [Version] = 1,
            [Namespaces] = new Dictionary<string, object?>
            {
                [ns] = new Dictionary<string, object?> { [Functions] = functions },
            },
            [Types] = new Dictionary<string, object?>(),
        };
}
