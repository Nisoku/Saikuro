using System.Buffers;
using System.Collections;
using MessagePack;

namespace Saikuro;

internal static class MsgpackHelper
{
    internal static byte[] Encode(Dictionary<string, object?> obj)
    {
        var buffer = new ArrayBufferWriter<byte>();
        var writer = new MessagePackWriter(buffer);
        WriteObject(ref writer, obj);
        writer.Flush();
        return buffer.WrittenSpan.ToArray();
    }

    internal static Dictionary<string, object?> Decode(ReadOnlyMemory<byte> bytes)
    {
        var reader = new MessagePackReader(bytes);
        var result = ReadObject(ref reader);
        return result as Dictionary<string, object?>
            ?? throw new InvalidDataException("MessagePack root is not a map");
    }

    private static void WriteObject(ref MessagePackWriter writer, object? value)
    {
        if (value is null)
        {
            writer.WriteNil();
            return;
        }

        switch (value)
        {
            case string s:
                writer.Write(s);
                return;
            case bool b:
                writer.Write(b);
                return;
            case int n:
                writer.Write(n);
                return;
            case long n:
                writer.Write(n);
                return;
            case short n:
                writer.Write(n);
                return;
            case byte n:
                writer.Write(n);
                return;
            case sbyte n:
                writer.Write(n);
                return;
            case ushort n:
                writer.Write(n);
                return;
            case uint n:
                writer.Write(n);
                return;
            case ulong n:
                writer.Write(n);
                return;
            case float f:
                writer.Write(f);
                return;
            case double d:
                writer.Write(d);
                return;
            case byte[] bin:
                writer.Write(bin);
                return;
        }

        if (value is IDictionary dict)
        {
            writer.WriteMapHeader(dict.Count);
            foreach (DictionaryEntry entry in dict)
            {
                WriteObject(ref writer, entry.Key);
                WriteObject(ref writer, entry.Value);
            }
            return;
        }

        if (value is IList list)
        {
            writer.WriteArrayHeader(list.Count);
            foreach (var item in list)
            {
                WriteObject(ref writer, item);
            }
            return;
        }

        throw new MessagePackSerializationException(
            $"Type {value.GetType()} is not supported by MsgpackHelper");
    }

    private static object? ReadObject(ref MessagePackReader reader)
    {
        switch (reader.NextMessagePackType)
        {
            case MessagePackType.Nil:
                reader.ReadNil();
                return null;

            case MessagePackType.Boolean:
                return reader.ReadBoolean();

            case MessagePackType.Integer:
                return reader.ReadInt64();

            case MessagePackType.Float:
                return reader.NextCode == MessagePackCode.Float32
                    ? reader.ReadSingle()
                    : reader.ReadDouble();

            case MessagePackType.String:
                return reader.ReadString();

            case MessagePackType.Binary:
                return reader.ReadBytes()?.ToArray();

            case MessagePackType.Array:
                {
                    var count = reader.ReadArrayHeader();
                    var arr = new object?[count];
                    for (var i = 0; i < count; i++)
                    {
                        arr[i] = ReadObject(ref reader);
                    }
                    return arr;
                }

            case MessagePackType.Map:
                {
                    var count = reader.ReadMapHeader();
                    var dict = new Dictionary<string, object?>((int)count);
                    for (var i = 0; i < count; i++)
                    {
                        var key = ReadObject(ref reader);
                        var value = ReadObject(ref reader);
                        dict.Add(key as string ?? throw new MessagePackSerializationException("Map key is not a string"), value);
                    }
                    return dict;
                }

            default:
                throw new MessagePackSerializationException(
                    $"Unsupported MessagePack type: {reader.NextMessagePackType}");
        }
    }
}
