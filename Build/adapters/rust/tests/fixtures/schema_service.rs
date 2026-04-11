pub fn add(a: i64, b: i64) -> i64 {
    a + b
}

pub fn maybe(msg: Option<String>) -> Option<String> {
    msg
}

pub fn sum_values(m: std::collections::HashMap<String, i64>) -> i64 {
    m.values().copied().sum()
}

pub fn wrap_items(items: Vec<i64>) -> Vec<i64> {
    items
}
