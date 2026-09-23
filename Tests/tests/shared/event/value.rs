use crate::shared_test;
use crate::TestSuite;
use saikuro_event::log::{LogLevel, LogRecord};
use saikuro_event::{Value, ValueMap};

// Value accessor narrowing/widening and LogRecord <> Value conversion.

pub fn register(suite: &mut TestSuite) {
    shared_test!(
        suite,
        "event::value_i64_narrows_uint_within_range",
        value_i64_narrows_uint_within_range,
    );
    shared_test!(
        suite,
        "event::value_i64_rejects_overflowing_uint",
        value_i64_rejects_overflowing_uint,
    );
    shared_test!(
        suite,
        "event::value_u64_widens_nonnegative_int",
        value_u64_widens_nonnegative_int,
    );
    shared_test!(
        suite,
        "event::value_u64_rejects_negative_int",
        value_u64_rejects_negative_int,
    );
    shared_test!(
        suite,
        "event::value_f64_accepts_numeric_variants",
        value_f64_accepts_numeric_variants,
    );
    shared_test!(
        suite,
        "event::value_accessors_return_none_for_wrong_variant",
        value_accessors_return_none_for_wrong_variant,
    );
    shared_test!(suite, "event::value_null_is_default", value_null_is_default,);
    shared_test!(
        suite,
        "event::log_record_from_value_map",
        log_record_from_value_map,
    );
    shared_test!(
        suite,
        "event::log_record_missing_fields_default",
        log_record_missing_fields_default,
    );
    shared_test!(
        suite,
        "event::log_record_from_non_map_errors",
        log_record_from_non_map_errors,
    );
    shared_test!(
        suite,
        "event::log_record_too_many_fields_errors",
        log_record_too_many_fields_errors,
    );
}

fn value_i64_narrows_uint_within_range() -> Result<(), &'static str> {
    assert_eq!(Value::UInt(42).as_i64(), Some(42));
    assert_eq!(Value::UInt(0).as_i64(), Some(0));
    Ok(())
}

fn value_i64_rejects_overflowing_uint() -> Result<(), &'static str> {
    crate::check_test!(
        Value::UInt(u64::MAX).as_i64().is_none(),
        "uint above i64::MAX must not narrow"
    );
    Ok(())
}

fn value_u64_widens_nonnegative_int() -> Result<(), &'static str> {
    assert_eq!(Value::Int(7).as_u64(), Some(7));
    assert_eq!(Value::Int(0).as_u64(), Some(0));
    Ok(())
}

fn value_u64_rejects_negative_int() -> Result<(), &'static str> {
    crate::check_test!(
        Value::Int(-1).as_u64().is_none(),
        "negative int must not widen to u64"
    );
    crate::check_test!(
        Value::Int(i64::MIN).as_u64().is_none(),
        "min int must not widen to u64"
    );
    Ok(())
}

fn value_f64_accepts_numeric_variants() -> Result<(), &'static str> {
    assert_eq!(Value::Float(1.5).as_f64(), Some(1.5));
    assert_eq!(Value::Int(-3).as_f64(), Some(-3.0));
    assert_eq!(Value::UInt(4).as_f64(), Some(4.0));
    Ok(())
}

fn value_accessors_return_none_for_wrong_variant() -> Result<(), &'static str> {
    crate::check_test!(
        Value::Bool(true).as_i64().is_none(),
        "bool must not be an i64"
    );
    crate::check_test!(
        Value::Bool(true).as_u64().is_none(),
        "bool must not be a u64"
    );
    crate::check_test!(
        Value::String("x".into()).as_f64().is_none(),
        "string must not be an f64"
    );
    crate::check_test!(Value::Int(5).as_str().is_none(), "int must not be a string");
    Ok(())
}

fn value_null_is_default() -> Result<(), &'static str> {
    let value = Value::default();
    crate::check_test!(value.is_null(), "default value must be Null");
    crate::check_test!(Value::Null.is_null(), "explicit Null must be null");
    Ok(())
}

fn log_record_from_value_map() -> Result<(), &'static str> {
    let mut map = ValueMap::new();
    map.insert(
        "ts".into(),
        Value::String("2026-09-18T00:00:00.000Z".into()),
    );
    map.insert("level".into(), Value::String("warn".into()));
    map.insert("name".into(), Value::String("svc.a".into()));
    map.insert("msg".into(), Value::String("boom".into()));
    map.insert("attempts".into(), Value::Int(3));

    let record = LogRecord::try_from(Value::Map(map)).map_err(|_| "try_from must succeed")?;
    crate::check_test!(record.level == LogLevel::Warn, "level must parse");
    crate::check_test!(record.name == "svc.a", "name must roundtrip");
    crate::check_test!(record.msg == "boom", "msg must roundtrip");
    let attempts = record
        .fields()
        .and_then(|fields| fields.get("attempts"))
        .ok_or("context field must survive conversion")?;
    crate::check_test!(
        *attempts == Value::Int(3),
        "extra fields must land in the context bag"
    );
    Ok(())
}

fn log_record_missing_fields_default() -> Result<(), &'static str> {
    let record =
        LogRecord::try_from(Value::Map(ValueMap::new())).map_err(|_| "empty map must convert")?;
    crate::check_test!(record.level == LogLevel::Info, "level must default to Info");
    crate::check_test!(record.name.is_empty(), "name must default to empty");
    crate::check_test!(record.msg.is_empty(), "msg must default to empty");
    crate::check_test!(
        record.fields().is_none(),
        "no context fields must stay absent"
    );
    Ok(())
}

fn log_record_from_non_map_errors() -> Result<(), &'static str> {
    crate::check_test!(
        LogRecord::try_from(Value::Int(1)).is_err(),
        "non-map value must be rejected"
    );
    crate::check_test!(
        LogRecord::try_from(Value::Null).is_err(),
        "null value must be rejected"
    );
    Ok(())
}

fn log_record_too_many_fields_errors() -> Result<(), &'static str> {
    let mut map = ValueMap::new();
    for i in 0..32 {
        map.insert(crate::format!("key{i}"), Value::Int(i as i64));
    }
    crate::check_test!(
        LogRecord::try_from(Value::Map(map)).is_err(),
        "a context bag over capacity must error, not silently truncate"
    );
    Ok(())
}
