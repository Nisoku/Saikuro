use crate::shared_test;
use crate::TestSuite;
use saikuro_codegen::{to_camel_case, to_pascal_case};

// Identifier normalization used by every language backend.

pub fn register(suite: &mut TestSuite) {
    shared_test!(
        suite,
        "codegen::pascal_case_basic_snake",
        pascal_case_basic_snake
    );
    shared_test!(
        suite,
        "codegen::pascal_case_handles_all_separators",
        pascal_case_handles_all_separators,
    );
    shared_test!(
        suite,
        "codegen::pascal_case_collapses_double_separators",
        pascal_case_collapses_double_separators,
    );
    shared_test!(
        suite,
        "codegen::pascal_case_trims_leading_and_trailing_separators",
        pascal_case_trims_leading_and_trailing_separators,
    );
    shared_test!(
        suite,
        "codegen::pascal_case_empty_input",
        pascal_case_empty_input,
    );
    shared_test!(
        suite,
        "codegen::camel_case_lowercases_first_word",
        camel_case_lowercases_first_word,
    );
    shared_test!(
        suite,
        "codegen::camel_case_allcaps_first_part",
        camel_case_allcaps_first_part,
    );
}

fn pascal_case_basic_snake() -> Result<(), &'static str> {
    assert_eq!(to_pascal_case("hello_world"), "HelloWorld");
    Ok(())
}

fn pascal_case_handles_all_separators() -> Result<(), &'static str> {
    assert_eq!(
        to_pascal_case("http-server/commands\\admin"),
        "HttpServerCommandsAdmin"
    );
    assert_eq!(to_pascal_case("user_profile"), "UserProfile");
    Ok(())
}

fn pascal_case_collapses_double_separators() -> Result<(), &'static str> {
    assert_eq!(to_pascal_case("a__b"), "AB");
    assert_eq!(to_pascal_case("a_--_b"), "AB");
    Ok(())
}

fn pascal_case_trims_leading_and_trailing_separators() -> Result<(), &'static str> {
    assert_eq!(to_pascal_case("_leading_trailing_"), "LeadingTrailing");
    assert_eq!(to_pascal_case("-text-"), "Text");
    Ok(())
}

fn pascal_case_empty_input() -> Result<(), &'static str> {
    assert_eq!(to_pascal_case(""), "");
    assert_eq!(to_pascal_case("___"), "");
    Ok(())
}

fn camel_case_lowercases_first_word() -> Result<(), &'static str> {
    assert_eq!(to_camel_case("get_user"), "getUser");
    assert_eq!(to_camel_case("UserProfile"), "userProfile");
    Ok(())
}

fn camel_case_allcaps_first_part() -> Result<(), &'static str> {
    // Only the first character is case-normalized per part, so an all-caps
    // word keeps its shape apart from the initial letter.
    assert_eq!(to_camel_case("HTTP_get"), "hTTPGet");
    Ok(())
}
