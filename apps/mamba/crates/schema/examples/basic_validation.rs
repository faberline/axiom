//! Basic Validation Example
//!
//! This example demonstrates simple type validation using cclab-shield.
//!
//! Run with:
//! ```bash
//! cargo run -p cclab-shield --example basic_validation
//! ```

use schema::{
    constraints::{FieldDescriptor, NumericConstraints, StringConstraints},
    validate, TypeDescriptor, Value,
};

// ============================================================================
// Primitive Type Validation
// ============================================================================

fn validate_primitives() {
    println!("1. Primitive Type Validation");
    println!("----------------------------");

    // String validation
    let string_type = TypeDescriptor::String(StringConstraints::default());
    let string_value = Value::String("Hello, World!".to_string());
    let result = validate(&string_value, &string_type);
    println!("  String 'Hello, World!': {}", result_str(&result));

    // Integer validation
    let int_type = TypeDescriptor::Int64(NumericConstraints::default());
    let int_value = Value::Int(42);
    let result = validate(&int_value, &int_type);
    println!("  Int 42: {}", result_str(&result));

    // Boolean validation
    let bool_type = TypeDescriptor::Bool;
    let bool_value = Value::Bool(true);
    let result = validate(&bool_value, &bool_type);
    println!("  Bool true: {}", result_str(&result));

    // Float validation
    let float_type = TypeDescriptor::Float64(NumericConstraints::default());
    let float_value = Value::Float(3.14159);
    let result = validate(&float_value, &float_type);
    println!("  Float 3.14159: {}", result_str(&result));

    // Type mismatch
    let result = validate(&Value::String("not a number".to_string()), &int_type);
    println!("  String 'not a number' as Int: {}", result_str(&result));
    println!();
}

// ============================================================================
// String Constraints
// ============================================================================

fn validate_string_constraints() {
    println!("2. String Constraints");
    println!("--------------------");

    // Min/Max length
    let constrained = TypeDescriptor::String(StringConstraints {
        min_length: Some(3),
        max_length: Some(10),
        pattern: None,
        format: None,
    });

    let test_strings = ["ab", "abc", "abcdefghij", "abcdefghijk"];
    for s in test_strings {
        let value = Value::String(s.to_string());
        let result = validate(&value, &constrained);
        println!("  '{}' (len {}): {}", s, s.len(), result_str(&result));
    }
    println!();
}

// ============================================================================
// Numeric Constraints
// ============================================================================

fn validate_numeric_constraints() {
    println!("3. Numeric Constraints");
    println!("---------------------");

    // Integer range
    let age_type = TypeDescriptor::Int64(NumericConstraints {
        minimum: Some(0),
        maximum: Some(150),
        exclusive_minimum: None,
        exclusive_maximum: None,
        multiple_of: None,
    });

    let test_ages = [-1, 0, 25, 150, 151];
    for age in test_ages {
        let value = Value::Int(age);
        let result = validate(&value, &age_type);
        println!("  Age {}: {}", age, result_str(&result));
    }
    println!();

    // Multiple of constraint
    let even_type = TypeDescriptor::Int64(NumericConstraints {
        minimum: None,
        maximum: None,
        exclusive_minimum: None,
        exclusive_maximum: None,
        multiple_of: Some(2),
    });

    let test_numbers = [2, 3, 4, 5];
    println!("  Multiple of 2:");
    for n in test_numbers {
        let value = Value::Int(n);
        let result = validate(&value, &even_type);
        println!("    {}: {}", n, result_str(&result));
    }
    println!();
}

// ============================================================================
// Format Types
// ============================================================================

fn validate_format_types() {
    println!("4. Format Types");
    println!("---------------");

    // Email validation
    let email_type = TypeDescriptor::Email;
    let test_emails = [
        "user@example.com",
        "invalid-email",
        "user@",
        "test.user@domain.co.uk",
    ];

    println!("  Email validation:");
    for email in test_emails {
        let value = Value::String(email.to_string());
        let result = validate(&value, &email_type);
        println!("    '{}': {}", email, result_str(&result));
    }
    println!();

    // UUID validation
    let uuid_type = TypeDescriptor::Uuid;
    let test_uuids = [
        "550e8400-e29b-41d4-a716-446655440000",
        "not-a-uuid",
        "550e8400-e29b-41d4-a716",
    ];

    println!("  UUID validation:");
    for uuid in test_uuids {
        let value = Value::String(uuid.to_string());
        let result = validate(&value, &uuid_type);
        println!("    '{}': {}", uuid, result_str(&result));
    }
    println!();
}

// ============================================================================
// Object Validation
// ============================================================================

fn validate_objects() {
    println!("5. Object Validation");
    println!("--------------------");

    // Define a User type
    let user_type = TypeDescriptor::Object {
        fields: vec![
            FieldDescriptor::new(
                "name",
                TypeDescriptor::String(StringConstraints {
                    min_length: Some(1),
                    max_length: Some(100),
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
            )
            .optional(),
        ],
        additional: None,
    };

    // Valid user
    let valid_user = Value::Object(vec![
        ("name".to_string(), Value::String("Alice".to_string())),
        (
            "email".to_string(),
            Value::String("alice@example.com".to_string()),
        ),
        ("age".to_string(), Value::Int(30)),
    ]);
    let result = validate(&valid_user, &user_type);
    println!("  Valid user: {}", result_str(&result));

    // Invalid email
    let invalid_email = Value::Object(vec![
        ("name".to_string(), Value::String("Bob".to_string())),
        (
            "email".to_string(),
            Value::String("invalid-email".to_string()),
        ),
    ]);
    let result = validate(&invalid_email, &user_type);
    println!("  Invalid email: {}", result_str(&result));

    // Missing required field
    let missing_field = Value::Object(vec![(
        "email".to_string(),
        Value::String("test@example.com".to_string()),
    )]);
    let result = validate(&missing_field, &user_type);
    println!("  Missing 'name': {}", result_str(&result));

    // Without optional field
    let without_optional = Value::Object(vec![
        ("name".to_string(), Value::String("Charlie".to_string())),
        (
            "email".to_string(),
            Value::String("charlie@example.com".to_string()),
        ),
    ]);
    let result = validate(&without_optional, &user_type);
    println!("  Without optional 'age': {}", result_str(&result));
    println!();
}

// ============================================================================
// List Validation
// ============================================================================

fn validate_lists() {
    println!("6. List Validation");
    println!("------------------");

    use schema::constraints::ListConstraints;

    // List of integers
    let int_list_type = TypeDescriptor::List {
        items: Box::new(TypeDescriptor::Int64(NumericConstraints::default())),
        constraints: ListConstraints::default(),
    };

    let valid_list = Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
    let result = validate(&valid_list, &int_list_type);
    println!("  [1, 2, 3]: {}", result_str(&result));

    // List with wrong type
    let mixed_list = Value::List(vec![
        Value::Int(1),
        Value::String("two".to_string()),
        Value::Int(3),
    ]);
    let result = validate(&mixed_list, &int_list_type);
    println!("  [1, 'two', 3]: {}", result_str(&result));

    // List with min/max items
    let sized_list_type = TypeDescriptor::List {
        items: Box::new(TypeDescriptor::Int64(NumericConstraints::default())),
        constraints: ListConstraints {
            min_items: Some(2),
            max_items: Some(4),
            unique_items: false,
        },
    };

    let small_list = Value::List(vec![Value::Int(1)]);
    let result = validate(&small_list, &sized_list_type);
    println!("  [1] (min 2): {}", result_str(&result));

    let large_list = Value::List(vec![
        Value::Int(1),
        Value::Int(2),
        Value::Int(3),
        Value::Int(4),
        Value::Int(5),
    ]);
    let result = validate(&large_list, &sized_list_type);
    println!("  [1,2,3,4,5] (max 4): {}", result_str(&result));
    println!();
}

// ============================================================================
// Helper Functions
// ============================================================================

fn result_str<T>(result: &Result<T, schema::ValidationErrors>) -> &'static str {
    if result.is_ok() {
        "OK"
    } else {
        "INVALID"
    }
}

// ============================================================================
// Main
// ============================================================================

fn main() {
    println!("Basic Validation Example");
    println!("========================\n");

    validate_primitives();
    validate_string_constraints();
    validate_numeric_constraints();
    validate_format_types();
    validate_objects();
    validate_lists();

    println!("Summary:");
    println!("  - Use TypeDescriptor to define expected types");
    println!("  - Use Value to represent runtime data");
    println!("  - validate() returns Ok(()) or Err(ValidationErrors)");
    println!("  - Constraints provide fine-grained validation rules");
}
