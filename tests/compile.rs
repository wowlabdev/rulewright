#[test]
fn invalid_rule_parameter_types_have_a_focused_error() {
    let fixtures = trybuild::TestCases::new();

    fixtures.compile_fail("tests/ui/unsupported_param_type.rs");
}
