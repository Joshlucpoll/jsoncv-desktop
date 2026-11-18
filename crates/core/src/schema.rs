use std::sync::LazyLock;

use serde::Serialize;

pub const SCHEMA: &str = include_str!("../schema/jsoncv.schema.json");

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SchemaError {
    /// JSON pointer to the failing value; "" is the root.
    pub path: String,
    pub message: String,
}

static VALIDATOR: LazyLock<jsonschema::Validator> = LazyLock::new(|| {
    let schema: serde_json::Value =
        serde_json::from_str(SCHEMA).expect("bundled schema is valid JSON");
    jsonschema::validator_for(&schema).expect("bundled schema compiles")
});

pub fn validate(cv: &serde_json::Value) -> Result<(), Vec<SchemaError>> {
    let errs: Vec<SchemaError> = VALIDATOR
        .iter_errors(cv)
        .map(|e| SchemaError {
            path: e.instance_path().to_string(),
            message: e.to_string(),
        })
        .collect();
    if errs.is_empty() {
        Ok(())
    } else {
        Err(errs)
    }
}
