#include <saikuro/schema_extractor.hpp>

#include <algorithm>
#include <cctype>
#include <fstream>
#include <iostream>
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

struct NestingState {
    int paren_depth = 0;
    int bracket_depth = 0;
    int angle_depth = 0;
    bool in_single_quote = false;
    bool in_double_quote = false;
    bool escaped = false;

    bool is_top_level() const {
        return paren_depth == 0 && bracket_depth == 0 && angle_depth == 0;
    }

    // Process one character. Returns true if `c` is `delimiter` at top level
    // and outside any quotes. When `is_angle_operator` is true, `<`/`>` are
    // treated as C++ operators (comparison, shift) rather than template delimiters.
    bool process(char c, char delimiter, bool is_angle_operator = false) {
        if (in_double_quote) {
            if (escaped) {
                escaped = false;
            } else if (c == '\\') {
                escaped = true;
            } else if (c == '"') {
                in_double_quote = false;
            }
            return false;
        }

        if (in_single_quote) {
            if (escaped) {
                escaped = false;
            } else if (c == '\\') {
                escaped = true;
            } else if (c == '\'') {
                in_single_quote = false;
            }
            return false;
        }

        if (c == '"') {
            in_double_quote = true;
            return false;
        }
        if (c == '\'') {
            in_single_quote = true;
            return false;
        }

        if (c == '(') {
            ++paren_depth;
        } else if (c == ')' && paren_depth > 0) {
            --paren_depth;
        } else if (c == '[') {
            ++bracket_depth;
        } else if (c == ']' && bracket_depth > 0) {
            --bracket_depth;
        } else if (c == '<' && !is_angle_operator) {
            ++angle_depth;
        } else if (c == '>' && !is_angle_operator && angle_depth > 0) {
            --angle_depth;
        }

        return c == delimiter && is_top_level();
    }
};

std::vector<std::string> split_args_aware_of_nesting(const std::string& text) {
    std::vector<std::string> out;
    if (trim(text).empty()) {
        return out;
    }

    NestingState state;
    std::string current;

    for (size_t i = 0; i < text.size(); ++i) {
        char c = text[i];
        bool is_angle_operator = false;
        if (c == '<' && i + 1 < text.size() && (text[i + 1] == '=' || text[i + 1] == '<'))
            is_angle_operator = true;
        else if (c == '>' && i + 1 < text.size() && (text[i + 1] == '=' || text[i + 1] == '>'))
            is_angle_operator = true;
        if (state.process(c, ',', is_angle_operator)) {
            out.push_back(current);
            current.clear();
        } else {
            current += c;
        }
    }

    out.push_back(current);
    return out;
}

std::string strip_top_level_initializer(const std::string& text) {
    NestingState state;

    for (size_t i = 0; i < text.size(); ++i) {
        if (state.process(text[i], '=')) {
            return trim(text.substr(0, i));
        }
    }

    return trim(text);
}

bool is_ident_char(char c) {
    return std::isalnum(static_cast<unsigned char>(c)) != 0 || c == '_';
}

char next_non_space_char(const std::string& source, size_t start) {
    for (size_t i = start; i < source.size(); ++i) {
        const unsigned char ch = static_cast<unsigned char>(source[i]);
        if (std::isspace(ch) == 0) {
            return source[i];
        }
    }
    return '\0';
}

std::string remove_comments(const std::string& source) {
    std::string out;
    out.reserve(source.size());

    bool in_line_comment = false;
    bool in_block_comment = false;
    bool in_double_quote = false;
    bool in_single_quote = false;
    bool in_raw_string = false;
    bool escaped = false;
    std::string raw_delim;

    for (size_t i = 0; i < source.size(); ++i) {
        const char c = source[i];
        const char next = (i + 1 < source.size()) ? source[i + 1] : '\0';

        if (in_line_comment) {
            if (c == '\n' || c == '\r') {
                in_line_comment = false;
                out += c;
            }
            continue;
        }

        if (in_block_comment) {
            if (c == '*' && next == '/') {
                const char prev = out.empty() ? '\0' : out.back();
                const char upcoming = next_non_space_char(source, i + 2);
                if (is_ident_char(prev) && is_ident_char(upcoming)) {
                    out += ' ';
                }
                in_block_comment = false;
                ++i;
                continue;
            }
            if (c == '\n' || c == '\r') {
                out += c;
            }
            continue;
        }

        if (in_double_quote) {
            out += c;
            if (escaped) {
                escaped = false;
            } else if (c == '\\') {
                escaped = true;
            } else if (c == '"') {
                in_double_quote = false;
            }
            continue;
        }

        if (in_single_quote) {
            out += c;
            if (escaped) {
                escaped = false;
            } else if (c == '\\') {
                escaped = true;
            } else if (c == '\'') {
                in_single_quote = false;
            }
            continue;
        }

        if (in_raw_string) {
            out += c;
            const std::string terminator = ")" + raw_delim + "\"";
            if (c == ')' && i + terminator.size() <= source.size()) {
                if (source.substr(i, terminator.size()) == terminator) {
                    for (size_t j = 1; j < terminator.size(); ++j) {
                        out += source[i + j];
                    }
                    i += terminator.size() - 1;
                    in_raw_string = false;
                    raw_delim.clear();
                }
            }
            continue;
        }

        if (c == '/' && next == '/') {
            in_line_comment = true;
            ++i;
            continue;
        }
        if (c == '/' && next == '*') {
            in_block_comment = true;
            ++i;
            continue;
        }
        if (c == '"') {
            in_double_quote = true;
            out += c;
            continue;
        }
        if (c == 'R' && next == '"') {
            size_t j = i + 2;
            std::string delim;
            while (j < source.size() && source[j] != '(') {
                delim += source[j];
                ++j;
            }
            if (j < source.size() && source[j] == '(') {
                in_raw_string = true;
                raw_delim = delim;
                out.append(source, i, j - i + 1);
                i = j;
                continue;
            }
        }
        if (c == '\'') {
            in_single_quote = true;
            out += c;
            continue;
        }

        out += c;
    }

    return out;
}

std::string json_escape(const std::string& value) {
    std::string out;
    out.reserve(value.size());
    constexpr const char hex[] = "0123456789abcdef";
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

    const auto erase_word = [&normalized](const std::string& word) {
        size_t pos = 0;
        while ((pos = normalized.find(word, pos)) != std::string::npos) {
            const bool left_ok = (pos == 0) || !is_ident_char(normalized[pos - 1]);
            const size_t end = pos + word.size();
            const bool right_ok = (end >= normalized.size()) || !is_ident_char(normalized[end]);
            if (left_ok && right_ok) {
                normalized.erase(pos, word.size());
            } else {
                pos = end;
            }
        }
    };

    erase_word("const");
    erase_word("volatile");
    normalized.erase(std::remove(normalized.begin(), normalized.end(), ' '), normalized.end());
    normalized = trim(normalized);

    std::string compact = raw;
    compact.erase(std::remove_if(compact.begin(), compact.end(), ::isspace), compact.end());
    const bool is_container = compact.find('<') != std::string::npos;

    static const std::regex plain_char_ptr_re(R"(^char(\*|\*const|const\*)+$)");
    static const std::regex plain_string_re(R"(^(std::string|string)([\*&]+)?$)");
    static const std::regex plain_bool_re(R"(^bool([\*&]+)?$)");
    static const std::regex plain_float_re(R"(^(float|double)([\*&]+)?$)");
    static const std::regex plain_int_re(R"(^(int|long|short|size_t)([\*&]+)?$)");

    if (!is_container &&
        (std::regex_match(compact, plain_char_ptr_re) ||
         std::regex_match(compact, plain_string_re))) {
        return "string";
    }
    if (!is_container && std::regex_match(compact, plain_bool_re)) {
        return "bool";
    }
    if (!is_container && std::regex_match(compact, plain_float_re)) {
        return "f64";
    }
    if (normalized == "void") {
        return "unit";
    }
    if (!is_container && std::regex_match(compact, plain_int_re)) {
        return "i64";
    }
    return "any";
}

bool parse_arg(const std::string& raw, size_t index, Arg* out) {
    const std::string arg = strip_top_level_initializer(raw);
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

    out->name = name.empty() ? ("arg" + std::to_string(index)) : name;
    out->type = map_cpp_type(type);
    out->optional = false;
    return true;
}

std::vector<Function> parse_functions(const std::string& source) {
    // This parser intentionally supports a pragmatic subset of C++ declarations:
    // simple, semicolon-terminated function prototypes. Argument tokenization is
    // nesting-aware, but complex forms (e.g., function-pointer declarations) may
    // still be skipped by the prototype regex.
    const std::regex proto(
        R"(([A-Za-z_][A-Za-z0-9_:<>\s\*&]+?)\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(([^)]*)\)\s*;)");

    std::vector<Function> functions;
    const std::string clean_source = remove_comments(source);

    if (clean_source.find("(*") != std::string::npos ||
        std::regex_search(clean_source, std::regex(R"(<[^>]*,[^>]*>)"))) {
        // Parser skips declarations with function-pointer params or complex template
        // arguments.  The regex-based parser intentionally targets simple prototypes.
    }

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

        const std::vector<std::string> args = split_args_aware_of_nesting(args_raw);
        for (size_t i = 0; i < args.size(); ++i) {
            Arg parsed;
            if (parse_arg(args[i], i, &parsed)) {
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
    std::ifstream in(path);
    if (!in.is_open()) {
        throw std::runtime_error("failed to open file: " + path);
    }
    std::stringstream buffer;
    buffer << in.rdbuf();
    return extract_schema_from_header(buffer.str(), namespace_name, pretty);
}

}  // namespace saikuro
