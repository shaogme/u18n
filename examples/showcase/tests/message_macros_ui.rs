use trybuild::TestCases;

#[test]
fn rejects_invalid_message_macro_calls() {
    let cases = TestCases::new();
    cases.compile_fail("tests/ui/message_macro_invalid_calls.rs");
}
