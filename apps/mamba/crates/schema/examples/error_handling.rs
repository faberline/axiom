//! Error Handling Example
//!
//! This example demonstrates working with ValidationErrors in cclab-shield.
//!
//! Run with:
//! ```bash
//! cargo run -p cclab-shield --example error_handling
//! ```

use schema::{
    constraints::{FieldDescriptor, NumericConstraints, StringConstraints},
    errors::{ErrorType, ValidationError, ValidationErrors},
    validate, TypeDescriptor, Value,
};

fn main() {
    println!("Error Handling Example");
    println!("======================\n");

    demonstrate_error_types();
    demonstrate_error_accumulation();
    demonstrate_error_formatting();
    demonstrate_error_location();

    println!("Summary:");
    println!("  - ValidationErrors collects multiple errors");
    println!("  - ErrorType indicates the kind of error");
    println!("  - Location helps identify where errors occurred");
    println!("  - Display trait provides user-friendly messages");
}

fn demonstrate_error_types() {
    println!("1. Error Types");
    println!("--------------");

    // Type error - wrong type provided
    let int_type = TypeDescriptor::Int64(NumericConstraints::default());
    let string_value = Value::String("not a number".to_string());
    match validate(&string_value, &int_type) {
        Ok(()) => println!("  Unexpected success"),
        Err(errors) => {
            println!("  Type error:");
            for err in errors.as_slice() {
                println!("    Field: {}", err.field);
                println!("    Type: {:?}", err.error_type);
                println!("    Message: {}", err.message);
            }
        }
    }
    println!();

    // Constraint error - value doesn't meet constraints
    let constrained_string = TypeDescriptor::String(StringConstraints {
        min_length: Some(5),
        max_length: Some(10),
        pattern: None,
        format: None,
    });
    let short_string = Value::String("ab".to_string());
    match validate(&short_string, &constrained_string) {
        Ok(()) => println!("  Unexpected success"),
        Err(errors) => {
            println!("  Constraint error:");
            for err in errors.as_slice() {
                println!("    Field: {}", err.field);
                println!("    Type: {:?}", err.error_type);
                println!("    Message: {}", err.message);
            }
        }
    }
    println!();

    // Missing field error
    let user_type = TypeDescriptor::Object {
        fields: vec![
            FieldDescriptor::new("name", TypeDescriptor::String(Default::default())),
            FieldDescriptor::new("email", TypeDescriptor::Email),
        ],
        additional: None,
    };
    let missing_email = Value::Object(vec![(
        "name".to_string(),
        Value::String("Alice".to_string()),
    )]);
    match validate(&missing_email, &user_type) {
        Ok(()) => println!("  Unexpected success"),
        Err(errors) => {
            println!("  Missing field error:");
            for err in errors.as_slice() {
                println!("    Field: {}", err.field);
                println!("    Type: {:?}", err.error_type);
                println!("    Message: {}", err.message);
            }
        }
    }
    println!();
}

fn demonstrate_error_accumulation() {
    println!("2. Error Accumulation");
    println!("---------------------");

    // Multiple errors in one validation
    let user_type = TypeDescriptor::Object {
        fields: vec![
            FieldDescriptor::new(
                "name",
                TypeDescriptor::String(StringConstraints {
                    min_length: Some(2),
                    max_length: None,
                    pattern: None,
                    format: None,
                }),
            ),
            FieldDescriptor::new("email", TypeDescriptor::Email),
            FieldDescriptor::new(
                "age",
                TypeDescriptor::Int64(NumericConstraints {
                    minimum: Some(0),
                    maximum: Some(150),
                    ..Default::default()
                }),
            ),
        ],
        additional: None,
    };

    let invalid_user = Value::Object(vec![
        ("name".to_string(), Value::String("A".to_string())), // Too short
        ("email".to_string(), Value::String("invalid".to_string())), // Invalid email
        ("age".to_string(), Value::Int(-5)),                  // Negative age
    ]);

    match validate(&invalid_user, &user_type) {
        Ok(()) => println!("  Unexpected success"),
        Err(errors) => {
            println!("  All errors collected:");
            println!("  Total errors: {}", errors.len());
            for (i, err) in errors.as_slice().iter().enumerate() {
                println!("    {}. {}: {}", i + 1, err.field, err.message);
            }
        }
    }
    println!();
}

fn demonstrate_error_formatting() {
    println!("3. Error Formatting");
    println!("-------------------");

    // Create errors programmatically
    let mut errors = ValidationErrors::new();
    errors.add(ValidationError::new(
        "body".to_string(),
        "email".to_string(),
        "must be a valid email address".to_string(),
        ErrorType::ValueError,
    ));
    errors.add(ValidationError::new(
        "body".to_string(),
        "password".to_string(),
        "must be at least 8 characters".to_string(),
        ErrorType::ValueError,
    ));

    // Display formatting
    println!("  Using Display trait:");
    println!("  {}", errors);
    println!();

    // Debug formatting
    println!("  Using Debug trait:");
    println!("  {:?}", errors);
    println!();

    // Manual formatting
    println!("  Custom formatting:");
    for err in errors.as_slice() {
        println!("    - [{}] {}: {}", err.error_type, err.field, err.message);
    }
    println!();
}

fn demonstrate_error_location() {
    println!("4. Error Location");
    println!("-----------------");

    // Nested object validation
    let address_type = TypeDescriptor::Object {
        fields: vec![
            FieldDescriptor::new("street", TypeDescriptor::String(Default::default())),
            FieldDescriptor::new("city", TypeDescriptor::String(Default::default())),
            FieldDescriptor::new(
                "zip",
                TypeDescriptor::String(StringConstraints {
                    min_length: Some(5),
                    max_length: Some(5),
                    pattern: None,
                    format: None,
                }),
            ),
        ],
        additional: None,
    };

    let user_type = TypeDescriptor::Object {
        fields: vec![
            FieldDescriptor::new("name", TypeDescriptor::String(Default::default())),
            FieldDescriptor::new("address", address_type),
        ],
        additional: None,
    };

    let user_with_bad_address = Value::Object(vec![
        ("name".to_string(), Value::String("Alice".to_string())),
        (
            "address".to_string(),
            Value::Object(vec![
                (
                    "street".to_string(),
                    Value::String("123 Main St".to_string()),
                ),
                ("city".to_string(), Value::String("Springfield".to_string())),
                ("zip".to_string(), Value::String("123".to_string())), // Too short
            ]),
        ),
    ]);

    match validate(&user_with_bad_address, &user_type) {
        Ok(()) => println!("  Unexpected success"),
        Err(errors) => {
            println!("  Nested error location:");
            for err in errors.as_slice() {
                println!("    Location: {}", err.location);
                println!("    Field: {}", err.field);
                println!("    Message: {}", err.message);
            }
        }
    }
    println!();
}
