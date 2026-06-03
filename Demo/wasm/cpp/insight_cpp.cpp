#include <algorithm>
#include <cctype>
#include <cstring>
#include <emscripten/emscripten.h>
#include <sstream>
#include <string>
#include <unordered_map>
#include <vector>

static std::vector<std::string> tokenize(const std::string &text) {
    std::string lowered = text;
    std::transform(lowered.begin(), lowered.end(), lowered.begin(), [](unsigned char c) {
        if (std::isalnum(c)) return (char)std::tolower(c);
        return ' ';
    });
    std::stringstream ss(lowered);
    std::string token;
    std::vector<std::string> tokens;
    while (ss >> token) {
        tokens.push_back(token);
    }
    return tokens;
}

static std::string make_json(const std::vector<std::pair<std::string, int>> &bigrams,
                             const std::vector<std::pair<std::string, int>> &trigrams) {
    std::stringstream ss;
    ss << "{\"bigrams\":[";
    for (size_t i = 0; i < bigrams.size(); i++) {
        ss << "[\"" << bigrams[i].first << "\"," << bigrams[i].second << "]";
        if (i + 1 < bigrams.size()) ss << ",";
    }
    ss << "],\"trigrams\":[";
    for (size_t i = 0; i < trigrams.size(); i++) {
        ss << "[\"" << trigrams[i].first << "\"," << trigrams[i].second << "]";
        if (i + 1 < trigrams.size()) ss << ",";
    }
    ss << "]}";
    return ss.str();
}

static std::vector<std::pair<std::string, int>> top_ngrams(
    const std::vector<std::string> &tokens, int n, int topN) {
    std::unordered_map<std::string, int> counts;
    for (size_t i = 0; i + (size_t)n <= tokens.size(); i++) {
        std::string key = tokens[i];
        for (int j = 1; j < n; j++) {
            key += " " + tokens[i + j];
        }
        counts[key]++;
    }
    std::vector<std::pair<std::string, int>> vec(counts.begin(), counts.end());
    std::sort(vec.begin(), vec.end(), [](const auto &a, const auto &b) {
        return a.second > b.second || (a.second == b.second && a.first < b.first);
    });
    if (topN < 0) topN = 0;
    if ((int)vec.size() > topN) {
        vec.resize((size_t)topN);
    }
    return vec;
}

extern "C" EMSCRIPTEN_KEEPALIVE
char *insight_cpp_ngrams(const char *input, int topN) {
    if (!input) return nullptr;
    std::vector<std::string> tokens = tokenize(input);
    auto bigrams = top_ngrams(tokens, 2, topN);
    auto trigrams = top_ngrams(tokens, 3, topN);

    std::string json = make_json(bigrams, trigrams);
    char *out = (char *)malloc(json.size() + 1);
    if (!out) return nullptr;
    memcpy(out, json.c_str(), json.size() + 1);
    return out;
}

extern "C" EMSCRIPTEN_KEEPALIVE
void insight_cpp_free(char *ptr) {
    if (ptr) free(ptr);
}
