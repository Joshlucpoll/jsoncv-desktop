use jsoncv_core::schema::validate;
use serde_json::json;

#[test]
fn valid_minimal_cv_passes() {
    assert!(validate(&json!({"basics":{"name":"A"}})).is_ok());
}
#[test]
fn unknown_top_level_key_fails_with_root_path() {
    let errs = validate(&json!({"bogus": 1})).unwrap_err();
    assert!(errs.iter().any(|e| e.path == ""));
}
#[test]
fn bad_date_fails_with_pointer() {
    let errs = validate(&json!({"work":[{"startDate":"March 2020"}]})).unwrap_err();
    assert!(errs.iter().any(|e| e.path == "/work/0/startDate"));
}
#[test]
fn meta_allows_extra_tooling_keys() {
    assert!(validate(
        &json!({"meta":{"theme":"builtin:classic","tailoredFrom":{"cv":"my-cv","commit":"abc"}}})
    )
    .is_ok());
}
