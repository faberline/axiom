//! Discriminated Unions Example
//!
//! This example demonstrates union type handling in cclab-shield.
//!
//! Run with:
//! ```bash
//! cargo run -p cclab-shield --example discriminated_unions
//! ```

use schema::{
    constraints::{FieldDescriptor, NumericConstraints, StringConstraints},
    validate, DiscriminatedUnion, DiscriminatedUnionBuilder, TypeDescriptor, Value,
};

fn main() {
    println!("Discriminated Unions Example");
    println!("============================\n");

    demonstrate_simple_union();
    demonstrate_discriminated_union();
    demonstrate_nullable_union();

    println!("Summary:");
    println!("  - Union types allow values to be one of several types");
    println!("  - Discriminated unions use a field to identify the variant");
    println!("  - Nullable unions include null as a valid option");
}

fn demonstrate_simple_union() {
    println!("1. Simple Union Types");
    println!("---------------------");

    // A value that can be either a string or an integer
    let string_or_int = TypeDescriptor::Union {
        variants: vec![
            TypeDescriptor::String(StringConstraints::default()),
            TypeDescriptor::Int64(NumericConstraints::default()),
        ],
        nullable: false,
    };

    // Test with string
    let string_value = Value::String("hello".to_string());
    let result = validate(&string_value, &string_or_int);
    println!("  String 'hello': {}", result_str(&result));

    // Test with integer
    let int_value = Value::Int(42);
    let result = validate(&int_value, &string_or_int);
    println!("  Int 42: {}", result_str(&result));

    // Test with boolean (not in union)
    let bool_value = Value::Bool(true);
    let result = validate(&bool_value, &string_or_int);
    println!("  Bool true: {}", result_str(&result));

    // Test with null (not nullable)
    let null_value = Value::Null;
    let result = validate(&null_value, &string_or_int);
    println!("  Null (non-nullable): {}", result_str(&result));
    println!();
}

fn demonstrate_discriminated_union() {
    println!("2. Discriminated Unions");
    println!("-----------------------");

    println!("  Discriminated unions use a 'discriminator' field to determine the type.");
    println!();

    // Build a discriminated union for different message types
    let message_union = DiscriminatedUnionBuilder::new("type")
        .variant(
            "text",
            TypeDescriptor::Object {
                fields: vec![
                    FieldDescriptor::new(
                        "type",
                        TypeDescriptor::Literal {
                            values: vec![Value::String("text".to_string())],
                        },
                    ),
                    FieldDescriptor::new("content", TypeDescriptor::String(Default::default())),
                ],
                additional: None,
            },
        )
        .variant(
            "image",
            TypeDescriptor::Object {
                fields: vec![
                    FieldDescriptor::new(
                        "type",
                        TypeDescriptor::Literal {
                            values: vec![Value::String("image".to_string())],
                        },
                    ),
                    FieldDescriptor::new("url", TypeDescriptor::Url),
                    FieldDescriptor::new("width", TypeDescriptor::Int64(Default::default()))
                        .optional(),
                    FieldDescriptor::new("height", TypeDescriptor::Int64(Default::default()))
                        .optional(),
                ],
                additional: None,
            },
        )
        .variant(
            "file",
            TypeDescriptor::Object {
                fields: vec![
                    FieldDescriptor::new(
                        "type",
                        TypeDescriptor::Literal {
                            values: vec![Value::String("file".to_string())],
                        },
                    ),
                    FieldDescriptor::new("name", TypeDescriptor::String(Default::default())),
                    FieldDescriptor::new(
                        "size",
                        TypeDescriptor::Int64(NumericConstraints {
                            minimum: Some(0),
                            ..Default::default()
                        }),
                    ),
                ],
                additional: None,
            },
        )
        .build();

    // Test text message
    let text_msg = Value::Object(vec![
        ("type".to_string(), Value::String("text".to_string())),
        (
            "content".to_string(),
            Value::String("Hello, world!".to_string()),
        ),
    ]);

    println!("  Text message:");
    println!("    {}", validate_discriminated(&message_union, &text_msg));

    // Test image message
    let image_msg = Value::Object(vec![
        ("type".to_string(), Value::String("image".to_string())),
        (
            "url".to_string(),
            Value::String("https://example.com/image.png".to_string()),
        ),
        ("width".to_string(), Value::Int(800)),
        ("height".to_string(), Value::Int(600)),
    ]);

    println!("  Image message:");
    println!("    {}", validate_discriminated(&message_union, &image_msg));

    // Test file message
    let file_msg = Value::Object(vec![
        ("type".to_string(), Value::String("file".to_string())),
        (
            "name".to_string(),
            Value::String("document.pdf".to_string()),
        ),
        ("size".to_string(), Value::Int(1024)),
    ]);

    println!("  File message:");
    println!("    {}", validate_discriminated(&message_union, &file_msg));

    // Test unknown type
    let unknown_msg = Value::Object(vec![(
        "type".to_string(),
        Value::String("video".to_string()),
    )]);

    println!("  Unknown type:");
    println!(
        "    {}",
        validate_discriminated(&message_union, &unknown_msg)
    );
    println!();
}

fn demonstrate_nullable_union() {
    println!("3. Nullable Unions");
    println!("------------------");

    // Optional string (string or null)
    let optional_string = TypeDescriptor::Union {
        variants: vec![TypeDescriptor::String(StringConstraints::default())],
        nullable: true,
    };

    // Test with string
    let string_value = Value::String("hello".to_string());
    let result = validate(&string_value, &optional_string);
    println!("  String 'hello': {}", result_str(&result));

    // Test with null
    let null_value = Value::Null;
    let result = validate(&null_value, &optional_string);
    println!("  Null: {}", result_str(&result));

    // Test with wrong type
    let int_value = Value::Int(42);
    let result = validate(&int_value, &optional_string);
    println!("  Int 42: {}", result_str(&result));
    println!();

    // Using Optional type (convenience wrapper)
    println!("  Optional<T> is equivalent to Union<[T], nullable=true>:");
    let optional_int = TypeDescriptor::Optional(Box::new(TypeDescriptor::Int64(
        NumericConstraints::default(),
    )));

    let int_value = Value::Int(42);
    let result = validate(&int_value, &optional_int);
    println!("    Int 42: {}", result_str(&result));

    let null_value = Value::Null;
    let result = validate(&null_value, &optional_int);
    println!("    Null: {}", result_str(&result));
    println!();
}

fn result_str<T>(result: &Result<T, schema::ValidationErrors>) -> &'static str {
    if result.is_ok() {
        "OK"
    } else {
        "INVALID"
    }
}

fn validate_discriminated(union: &DiscriminatedUnion, value: &Value) -> &'static str {
    match union.validate(value) {
        Ok(_) => "OK",
        Err(_) => "INVALID",
    }
}
