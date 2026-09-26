//! Custom Validators Example
//!
//! This example demonstrates the custom validator types available in cclab-shield.
//!
//! Run with:
//! ```bash
//! cargo run -p cclab-shield --example custom_validators
//! ```

use schema::{
    custom_validators::{ValidatorContext, ValidatorMode},
    Value,
};

fn main() {
    println!("Custom Validators Example");
    println!("=========================\n");

    demonstrate_validator_context();
    demonstrate_validator_modes();
    demonstrate_field_validator_pattern();
    demonstrate_model_validator_pattern();

    println!("Summary:");
    println!("  - FieldValidator: Validates individual field values");
    println!("  - ModelValidator: Validates entire objects (cross-field)");
    println!("  - ValidatorContext: Access field data and metadata");
    println!("  - ValidatorMode: Control when validation runs");
}

fn demonstrate_validator_context() {
    println!("1. Validator Context");
    println!("--------------------");

    // Create a validator context
    let mut ctx = ValidatorContext::new();

    // Add field values (for cross-field validation)
    ctx.set_field("start_date", Value::String("2024-01-01".to_string()));
    ctx.set_field("end_date", Value::String("2024-12-31".to_string()));

    // Access fields
    println!("  Fields in context:");
    if let Some(start) = ctx.get_field("start_date") {
        println!("    start_date: {:?}", start);
    }
    if let Some(end) = ctx.get_field("end_date") {
        println!("    end_date: {:?}", end);
    }

    // Use metadata
    ctx.set_metadata("source", "user_input");
    ctx.set_metadata("request_id", "req-123");
    println!("  Metadata:");
    println!("    source: {:?}", ctx.get_metadata("source"));
    println!("    request_id: {:?}", ctx.get_metadata("request_id"));

    // Create context with location
    let located_ctx = ValidatorContext::with_location("body");
    println!("  Location: '{}'", located_ctx.location());
    println!();
}

fn demonstrate_validator_modes() {
    println!("2. Validator Modes");
    println!("------------------");

    println!("  Available modes:");
    println!("    Before - Run before type coercion (raw input)");
    println!("    After  - Run after type coercion (typed value) [default]");
    println!("    Wrap   - Wrap mode with control over coercion");
    println!();

    // Default mode
    let default_mode = ValidatorMode::default();
    println!("  Default mode: {:?}", default_mode);
    println!();
}

fn demonstrate_field_validator_pattern() {
    println!("3. Field Validator Pattern");
    println!("--------------------------");

    println!("  Field validators validate single field values.");
    println!();
    println!("  Pattern (implement FieldValidator trait):");
    println!(
        r#"
    struct LowercaseValidator;

    impl FieldValidator for LowercaseValidator {{
        fn field_name(&self) -> &str {{ "username" }}

        fn mode(&self) -> ValidatorMode {{ ValidatorMode::After }}

        fn validate(&self, value: &Value, ctx: &ValidatorContext)
            -> ValidationResult<Value>
        {{
            if let Value::String(s) = value {{
                if s != s.to_lowercase() {{
                    return Err(validation_error("must be lowercase"));
                }}
            }}
            Ok(value.clone())
        }}
    }}
    "#
    );
    println!();

    // Demonstrate the concept
    let test_cases = [
        ("alice", true),
        ("Bob", false),
        ("ADMIN", false),
        ("user_123", true),
    ];

    println!("  Lowercase validation examples:");
    for (username, expected_valid) in test_cases {
        let is_valid = username == username.to_lowercase();
        let status = if is_valid { "OK" } else { "INVALID" };
        let expected = if expected_valid { "OK" } else { "INVALID" };
        assert_eq!(is_valid, expected_valid);
        println!("    '{}': {} (expected: {})", username, status, expected);
    }
    println!();
}

fn demonstrate_model_validator_pattern() {
    println!("4. Model Validator Pattern");
    println!("--------------------------");

    println!("  Model validators validate entire objects (cross-field).");
    println!();
    println!("  Pattern (implement ModelValidator trait):");
    println!(
        r#"
    struct PasswordMatchValidator;

    impl ModelValidator for PasswordMatchValidator {{
        fn name(&self) -> &str {{ "password_match" }}

        fn mode(&self) -> ValidatorMode {{ ValidatorMode::After }}

        fn validate(&self, value: &Value, ctx: &ValidatorContext)
            -> ValidationResult<Value>
        {{
            if let Value::Object(fields) = value {{
                let password = find_field(fields, "password");
                let confirm = find_field(fields, "password_confirm");

                if password != confirm {{
                    return Err(validation_error("passwords do not match"));
                }}
            }}
            Ok(value.clone())
        }}
    }}
    "#
    );
    println!();

    // Demonstrate the concept
    let examples = [
        (("secret123", "secret123"), true, "Matching"),
        (("secret123", "different"), false, "Mismatch"),
    ];

    println!("  Password match validation examples:");
    for ((pass, confirm), expected_valid, desc) in examples {
        let is_valid = pass == confirm;
        let status = if is_valid { "OK" } else { "INVALID" };
        assert_eq!(is_valid, expected_valid);
        println!("    {}: {}", desc, status);
    }
    println!();
}
