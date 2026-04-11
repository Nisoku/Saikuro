#include <saikuro/schema_extractor.hpp>

#include <cctype>
#include <fstream>
#include <regex>
#include <sstream>
#include <stdexcept>
#include <string>
#include <vector>

namespace saikuro {
namespace {

struct Arg {
    std::string name;
    std::string type;
    bool optional;
};

struct Function {
    std::string name;
    std::string returns;
    std::vector<Arg> args;
};

std::string trim(const std::string& text) {
    size_t start = 0;
    while (start < text.size() && std::isspace(static_cast<unsigned char>(text[start])) != 0) {
        start += 1;
    }
    size_t end = text.size();
    while (end > start && std::isspace(static_cast<unsigned char>(text[end - 1])) != 0) {
        end -= 1;
    }
    return text.substr(start, end - start);
}

std::vector<std::string> split(const std::string& text, char delimiter) {
    std::vector<std::string> out;
    std::stringstream ss(text);
    std::string item;
    while (std::getline(ss, item, delimiter)) {
        out.push_back(item);
    }
    return out;
}

std::string remove_comments(const std::string& source) {
    const std::regex block_comment(R"(/\*[\s\S]*?\*/)");
    const std::regex line_comment(R"(//[^\n\r]*)");
    const std::string no_block = std::regex_replace(source, block_comment, " ");
    return std::regex_replace(no_block, line_comment, " ");
}

std::string json_escape(const std::string& value) {
    std::string out;
    out.reserve(value.size());
    static const char* hex = "0123456789abcdef";
    for (size_t i = 0; i < value.size(); ++i) {
        const unsigned char ch = static_cast<unsigned char>(value[i]);
        const char c = static_cast<char>(ch);
        switch (c) {
            case '\\':
                out += "\\\\";
                break;
            case '"':
                out += "\\\"";
                break;
            case '\n':
                out += "\\n";
                break;
            case '\r':
                out += "\\r";
                break;
            case '\t':
                out += "\\t";
                break;
            default:
                if (ch < 0x20) {
                    out += "\\u00";
                    out += hex[(ch >> 4) & 0x0f];
                    out += hex[ch & 0x0f];
                } else {
                    out += c;
                }
                break;
        }
    }
    return out;
}

std::string map_cpp_type(const std::string& raw) {
    std::string normalized = raw;

    const std::regex spaces("\\s+");
    normalized = std::regex_replace(normalized, spaces, " ");

    const auto erase_word = [&normalized](const std::string& word) {
        size_t pos = std::string::npos;
        while ((pos = normalized.find(word)) != std::string::npos) {
            normalized.erase(pos, word.size());
        }
    };

    erase_word("const");
    erase_word("volatile");
    normalized = trim(normalized);

    if (normalized.find("char*") != std::string::npos ||
        normalized.find("char *") != std::string::npos ||
        normalized.find("std::string") != std::string::npos ||
        normalized.find("string") != std::string::npos) {
        return "string";
    }
    if (normalized.find("bool") != std::string::npos) {
        return "bool";
    }
    if (normalized.find("float") != std::string::npos || normalized.find("double") != std::string::npos) {
        return "f64";
    }
    if (normalized == "void") {
        return "unit";
    }
    if (normalized.find("int") != std::string::npos ||
        normalized.find("long") != std::string::npos ||
        normalized.find("short") != std::string::npos ||
        normalized.find("size_t") != std::string::npos) {
        return "i64";
    }
    return "any";
}

bool parse_arg(const std::string& raw, Arg* out) {
    const std::string arg = trim(raw);
    if (arg.empty() || arg == "void") {
        return false;
    }

    std::vector<std::string> parts = split(arg, ' ');
    while (!parts.empty() && trim(parts.back()).empty()) {
        parts.pop_back();
    }
    if (parts.empty()) {
        return false;
    }

    std::string name;
    std::string candidate = trim(parts.back());
    std::string sigils;
    while (!candidate.empty() && (candidate[0] == '*' || candidate[0] == '&')) {
        sigils += candidate[0];
        candidate.erase(0, 1);
    }

    static const std::regex ident_re("^[A-Za-z_][A-Za-z0-9_]*$");
    if (!candidate.empty() && std::regex_match(candidate, ident_re)) {
        name = candidate;
        parts.pop_back();
    } else {
        sigils.clear();
    }

    std::string type;
    for (size_t i = 0; i < parts.size(); ++i) {
        const std::string part = trim(parts[i]);
        if (part.empty()) {
            continue;
        }
        if (!type.empty()) {
            type += " ";
        }
        type += part;
    }

    if (!sigils.empty()) {
        type += sigils;
    }

    if (type.empty()) {
        type = arg;
    }

    out->name = name.empty() ? "arg" : name;
    out->type = map_cpp_type(type);
    out->optional = false;
    return true;
}

std::vector<Function> parse_functions(const std::string& source) {
    const std::regex proto(
        R"(([A-Za-z_][A-Za-z0-9_:<>\s\*&]+?)\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(([^)]*)\)\s*;)");

    std::vector<Function> functions;
    const std::string clean_source = remove_comments(source);
    std::sregex_iterator it(clean_source.begin(), clean_source.end(), proto);
    std::sregex_iterator end;
    for (; it != end; ++it) {
        const std::smatch match = *it;

        const std::string returns = trim(match[1].str());
        const std::string name = match[2].str();
        const std::string args_raw = match[3].str();

        if (name.find("saikuro_") == 0) {
            continue;
        }

        Function f;
        f.name = name;
        f.returns = map_cpp_type(returns);

        const std::vector<std::string> args = split(args_raw, ',');
        for (size_t i = 0; i < args.size(); ++i) {
            Arg parsed;
            if (parse_arg(args[i], &parsed)) {
                f.args.push_back(parsed);
            }
        }

        functions.push_back(f);
    }
    return functions;
}

void write_indent(std::ostringstream& out, bool pretty, int depth) {
    if (!pretty) {
        return;
    }
    out << '\n';
    for (int i = 0; i < depth; ++i) {
        out << "  ";
    }
}

void write_type_obj(std::ostringstream& out, const std::string& primitive_type, bool pretty, int depth) {
    out << '{';
    if (pretty) {
        write_indent(out, pretty, depth + 1);
    }
    out << "\"kind\":\"primitive\"";
    out << ',';
    if (pretty) {
        write_indent(out, pretty, depth + 1);
    }
    out << "\"type\":\"" << json_escape(primitive_type) << "\"";
    if (pretty) {
        write_indent(out, pretty, depth);
    }
    out << '}';
}

}  // namespace

std::string extract_schema_from_header(
    const std::string& source,
    const std::string& namespace_name,
    bool pretty
) {
    const std::vector<Function> functions = parse_functions(source);

    std::ostringstream out;
    out << '{';
    write_indent(out, pretty, 1);
    out << "\"version\":1,";
    write_indent(out, pretty, 1);
    out << "\"namespaces\":{";
    write_indent(out, pretty, 2);
    out << "\"" << json_escape(namespace_name) << "\":{";
    write_indent(out, pretty, 3);
    out << "\"functions\":{";

    for (size_t i = 0; i < functions.size(); ++i) {
        const Function& fn = functions[i];
        write_indent(out, pretty, 4);
        out << "\"" << json_escape(fn.name) << "\":{";

        write_indent(out, pretty, 5);
        out << "\"args\":[";
        for (size_t ai = 0; ai < fn.args.size(); ++ai) {
            const Arg& arg = fn.args[ai];
            if (pretty) {
                write_indent(out, pretty, 6);
            }
            out << '{';
            if (pretty) {
                write_indent(out, pretty, 7);
            }
            out << "\"name\":\"" << json_escape(arg.name) << "\",";
            if (pretty) {
                write_indent(out, pretty, 7);
            }
            out << "\"type\":";
            write_type_obj(out, arg.type, pretty, 7);
            out << ',';
            if (pretty) {
                write_indent(out, pretty, 7);
            }
            out << "\"optional\":" << (arg.optional ? "true" : "false");
            if (pretty) {
                write_indent(out, pretty, 6);
            }
            out << '}';
            if (ai + 1 < fn.args.size()) {
                out << ',';
            }
        }
        if (pretty && !fn.args.empty()) {
            write_indent(out, pretty, 5);
        }
        out << "],";

        write_indent(out, pretty, 5);
        out << "\"returns\":";
        write_type_obj(out, fn.returns, pretty, 5);
        out << ',';

        write_indent(out, pretty, 5);
        out << "\"visibility\":\"public\",";
        write_indent(out, pretty, 5);
        out << "\"capabilities\":[],";
        write_indent(out, pretty, 5);
        out << "\"idempotent\":false";
        write_indent(out, pretty, 4);
        out << '}';
        if (i + 1 < functions.size()) {
            out << ',';
        }
    }

    if (pretty && !functions.empty()) {
        write_indent(out, pretty, 3);
    }
    out << '}';
    write_indent(out, pretty, 2);
    out << '}';
    write_indent(out, pretty, 1);
    out << "},";

    write_indent(out, pretty, 1);
    out << "\"types\":{}";
    write_indent(out, pretty, 0);
    out << '}';

    return out.str();
}

std::string extract_schema_from_file(
    const std::string& path,
    const std::string& namespace_name,
    bool pretty
) {
    std::ifstream in(path.c_str());
    if (!in.is_open()) {
        throw std::runtime_error("failed to open file: " + path);
    }
    std::stringstream buffer;
    buffer << in.rdbuf();
    return extract_schema_from_header(buffer.str(), namespace_name, pretty);
}

}  // namespace saikuro
