#include <saikuro/schema_extractor.hpp>

#include <algorithm>
#include <cctype>
#include <fstream>
#include <nlohmann/json.hpp>
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

std::string trim(const std::string &text) {
  size_t start = 0;
  while (start < text.size() &&
         std::isspace(static_cast<unsigned char>(text[start])) != 0) {
    start += 1;
  }
  size_t end = text.size();
  while (end > start &&
         std::isspace(static_cast<unsigned char>(text[end - 1])) != 0) {
    end -= 1;
  }
  return text.substr(start, end - start);
}

std::vector<std::string> split(const std::string &text, char delimiter) {
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
  // treated as C++ operators (comparison, shift) rather than template
  // delimiters.
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

std::vector<std::string> split_args_aware_of_nesting(const std::string &text) {
  std::vector<std::string> out;
  if (trim(text).empty()) {
    return out;
  }

  NestingState state;
  std::string current;

  for (size_t i = 0; i < text.size(); ++i) {
    char c = text[i];
    bool is_angle_operator = false;
    if (c == '<' && i + 1 < text.size() &&
        (text[i + 1] == '=' || text[i + 1] == '<'))
      is_angle_operator = true;
    else if (c == '>' && i + 1 < text.size() &&
             (text[i + 1] == '=' || text[i + 1] == '>'))
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

std::string strip_top_level_initializer(const std::string &text) {
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

char next_non_space_char(const std::string &source, size_t start) {
  for (size_t i = start; i < source.size(); ++i) {
    const unsigned char ch = static_cast<unsigned char>(source[i]);
    if (std::isspace(ch) == 0) {
      return source[i];
    }
  }
  return '\0';
}

std::string remove_comments(const std::string &source) {
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

std::string map_cpp_type(const std::string &raw) {
  std::string normalized = raw;

  const auto erase_word = [&normalized](const std::string &word) {
    size_t pos = 0;
    while ((pos = normalized.find(word, pos)) != std::string::npos) {
      const bool left_ok = (pos == 0) || !is_ident_char(normalized[pos - 1]);
      const size_t end = pos + word.size();
      const bool right_ok =
          (end >= normalized.size()) || !is_ident_char(normalized[end]);
      if (left_ok && right_ok) {
        normalized.erase(pos, word.size());
      } else {
        pos = end;
      }
    }
  };

  erase_word("const");
  erase_word("volatile");
  normalized.erase(std::remove(normalized.begin(), normalized.end(), ' '),
                   normalized.end());
  normalized = trim(normalized);

  std::string compact = raw;
  compact.erase(std::remove_if(compact.begin(), compact.end(), ::isspace),
                compact.end());
  const bool is_container = compact.find('<') != std::string::npos;

  static const std::regex plain_char_ptr_re(R"(^char(\*|\*const|const\*)+$)");
  static const std::regex plain_string_re(R"(^(std::string|string)([\*&]+)?$)");
  static const std::regex plain_bool_re(R"(^bool([\*&]+)?$)");
  static const std::regex plain_float_re(R"(^(float|double)([\*&]+)?$)");
  static const std::regex plain_int_re(R"(^(int|long|short|size_t)([\*&]+)?$)");

  if (!is_container && (std::regex_match(compact, plain_char_ptr_re) ||
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

bool parse_arg(const std::string &raw, size_t index, Arg *out) {
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

std::vector<Function> parse_functions(const std::string &source) {
  // Nesting-aware scanner for semicolon-terminated C++ function declarations.
  // Scans for `name(` patterns (with nesting-aware paren matching) then
  // backtracks to extract the return type.  Unlike a regex, this correctly
  // handles nested parentheses in argument lists.

  std::vector<Function> functions;
  const std::string clean_source = remove_comments(source);
  const size_t len = clean_source.size();
  size_t pos = 0;

  while (pos < len) {
    // Skip whitespace and semicolons.
    while (pos < len &&
           (std::isspace(static_cast<unsigned char>(clean_source[pos])) ||
            clean_source[pos] == ';')) {
      ++pos;
    }
    if (pos >= len)
      break;

    // Find the next identifier followed by '('.
    size_t name_start = pos;
    size_t decl_start = pos;
    while (
        name_start < len &&
        !std::isalpha(static_cast<unsigned char>(clean_source[name_start])) &&
        clean_source[name_start] != '_') {
      ++name_start;
    }
    if (name_start >= len)
      break;

    size_t name_end = name_start;
    while (name_end < len &&
           (std::isalnum(static_cast<unsigned char>(clean_source[name_end])) ||
            clean_source[name_end] == '_')) {
      ++name_end;
    }
    const std::string name =
        clean_source.substr(name_start, name_end - name_start);
    if (name.empty()) {
      pos = name_start + 1;
      continue;
    }

    // Skip whitespace before '('.
    size_t paren_pos = name_end;
    while (paren_pos < len &&
           std::isspace(static_cast<unsigned char>(clean_source[paren_pos]))) {
      ++paren_pos;
    }
    if (paren_pos >= len || clean_source[paren_pos] != '(') {
      pos = name_start + 1;
      continue;
    }

    // Nesting-aware scan for matching ')'.
    int depth = 0;
    size_t args_start = paren_pos + 1;
    size_t args_end = args_start;
    bool found_paren = false;
    for (; args_end < len; ++args_end) {
      char c = clean_source[args_end];
      if (c == '(') {
        ++depth;
      } else if (c == ')') {
        if (depth == 0) {
          found_paren = true;
          break;
        }
        --depth;
      } else if (c == '"' || c == '\'') {
        char quote = c;
        ++args_end;
        while (args_end < len && clean_source[args_end] != quote) {
          if (clean_source[args_end] == '\\')
            ++args_end;
          ++args_end;
        }
      }
    }
    if (!found_paren)
      break;

    const std::string args_raw =
        clean_source.substr(args_start, args_end - args_start);

    // Expect ';' after ')', ignoring whitespace.
    size_t semi_pos = args_end + 1;
    while (semi_pos < len &&
           std::isspace(static_cast<unsigned char>(clean_source[semi_pos]))) {
      ++semi_pos;
    }
    if (semi_pos >= len || clean_source[semi_pos] != ';') {
      pos = name_start + 1;
      continue;
    }

    // Return type is everything from the start of the declaration to the name.
    // Capture before advancing pos so the length (name_start - pos) is valid.
    const std::string returns =
        trim(clean_source.substr(decl_start, name_start - decl_start));

    pos = semi_pos + 1; // advance past ';' for next iteration
    decl_start = pos;

    if (name.find("saikuro_") == 0)
      continue;

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

} // namespace

std::string extract_schema_from_header(const std::string &source,
                                       const std::string &namespace_name,
                                       bool pretty) {
  const std::vector<Function> functions = parse_functions(source);

  nlohmann::json functions_obj;
  for (const Function &fn : functions) {
    nlohmann::json args_arr = nlohmann::json::array();
    for (const Arg &arg : fn.args) {
      args_arr.push_back({
          {"name", arg.name},
          {"type", {{"kind", "primitive"}, {"type", arg.type}}},
          {"optional", arg.optional},
      });
    }

    functions_obj[fn.name] = {
        {"args", std::move(args_arr)},
        {"returns", {{"kind", "primitive"}, {"type", fn.returns}}},
        {"visibility", "public"},
        {"capabilities", nlohmann::json::array()},
        {"idempotent", false},
    };
  }

  nlohmann::json doc = {
      {"version", 1},
      {"namespaces",
       {{namespace_name, {{"functions", std::move(functions_obj)}}}}},
      {"types", nlohmann::json::object()},
  };

  return doc.dump(pretty ? 2 : -1);
}

std::string extract_schema_from_file(const std::string &path,
                                     const std::string &namespace_name,
                                     bool pretty) {
  std::ifstream in(path);
  if (!in.is_open()) {
    throw std::runtime_error("failed to open file: " + path);
  }
  std::stringstream buffer;
  buffer << in.rdbuf();
  return extract_schema_from_header(buffer.str(), namespace_name, pretty);
}

} // namespace saikuro
