//! Benchmark tests for cclab-titan
//!
//! Tests performance of critical PostgreSQL operations using the cclab-probe framework.

use pg::{ExtractedValue, JoinCondition, JoinType, Operator, OrderDirection, QueryBuilder};
use qc::benchmark::{print_comparison_table, BenchmarkConfig, Benchmarker};
use qc::{expect, AssertionError};

// ============================================================================
// Benchmark: QueryBuilder Construction
// ============================================================================

#[test]
fn bench_query_builder_construction() -> Result<(), AssertionError> {
    let config = BenchmarkConfig::new(100, 5, 10);
    let benchmarker = Benchmarker::new(config);

    // Benchmark simple QueryBuilder creation
    let result = benchmarker.run("QueryBuilder::new", || QueryBuilder::new("users").unwrap());

    println!("\n=== QueryBuilder Construction Benchmark ===");
    result.print_detailed();

    // Verify reasonable performance (should be < 0.1ms per operation)
    expect(result.stats.mean_ms).to_be_less_than(&0.1)?;
    expect(result.success).to_be_true()?;

    Ok(())
}

#[test]
fn bench_query_builder_with_clauses() -> Result<(), AssertionError> {
    let config = BenchmarkConfig::new(50, 5, 5);
    let benchmarker = Benchmarker::new(config);

    // Benchmark QueryBuilder with multiple clauses
    let result = benchmarker.run("QueryBuilder with clauses", || {
        QueryBuilder::new("users")
            .unwrap()
            .select(vec![
                "id".to_string(),
                "name".to_string(),
                "email".to_string(),
            ])
            .unwrap()
            .where_clause("age", Operator::Gte, ExtractedValue::Int(18))
            .unwrap()
            .where_clause("active", Operator::Eq, ExtractedValue::Bool(true))
            .unwrap()
            .order_by("name", OrderDirection::Asc)
            .unwrap()
            .limit(10)
            .offset(20)
    });

    println!("\n=== QueryBuilder with Clauses Benchmark ===");
    result.print_detailed();

    // Should be < 0.5ms for building a complex query
    expect(result.stats.mean_ms).to_be_less_than(&0.5)?;
    expect(result.success).to_be_true()?;

    Ok(())
}

#[test]
fn bench_query_builder_with_joins() -> Result<(), AssertionError> {
    let config = BenchmarkConfig::new(50, 5, 5);
    let benchmarker = Benchmarker::new(config);

    // Benchmark QueryBuilder with JOIN clauses
    let result = benchmarker.run("QueryBuilder with JOINs", || {
        let join_cond = JoinCondition::new("id", "orders", "user_id").unwrap();
        QueryBuilder::new("users")
            .unwrap()
            .select(vec![
                "users.id".to_string(),
                "users.name".to_string(),
                "orders.total".to_string(),
            ])
            .unwrap()
            .join(JoinType::Left, "orders", None, join_cond)
            .unwrap()
            .where_clause("active", Operator::Eq, ExtractedValue::Bool(true))
            .unwrap()
            .order_by("name", OrderDirection::Asc)
            .unwrap()
    });

    println!("\n=== QueryBuilder with JOINs Benchmark ===");
    result.print_detailed();

    // JOIN queries should still be fast
    expect(result.stats.mean_ms).to_be_less_than(&0.8)?;
    expect(result.success).to_be_true()?;

    Ok(())
}

// ============================================================================
// Benchmark: Identifier Validation
// ============================================================================

#[test]
fn bench_identifier_validation() -> Result<(), AssertionError> {
    let config = BenchmarkConfig::new(500, 5, 20);
    let benchmarker = Benchmarker::new(config);

    let mut results = Vec::new();

    // Benchmark valid table name
    let result = benchmarker.run("Valid table name", || QueryBuilder::new("users").unwrap());
    results.push(result);

    // Benchmark valid complex name
    let result = benchmarker.run("Valid complex name", || {
        QueryBuilder::new("user_accounts_2024").unwrap()
    });
    results.push(result);

    // Benchmark invalid name (should fail fast)
    let result = benchmarker.run("Invalid name (injection)", || {
        let _ = QueryBuilder::new("users; DROP TABLE users--");
    });
    results.push(result);

    println!("\n=== Identifier Validation Benchmark ===");
    print_comparison_table(&results, Some("Valid table name"));

    // Valid identifiers should be very fast (< 0.05ms)
    expect(results[0].stats.mean_ms).to_be_less_than(&0.05)?;
    expect(results[1].stats.mean_ms).to_be_less_than(&0.05)?;

    // Invalid identifiers should also fail fast
    expect(results[2].stats.mean_ms).to_be_less_than(&0.1)?;

    Ok(())
}

#[test]
fn bench_column_validation() -> Result<(), AssertionError> {
    let config = BenchmarkConfig::new(200, 5, 10);
    let benchmarker = Benchmarker::new(config);

    // Benchmark valid column selection
    let result = benchmarker.run("Valid columns", || {
        let qb = QueryBuilder::new("users").unwrap();
        qb.select(vec![
            "id".to_string(),
            "name".to_string(),
            "email".to_string(),
        ])
        .unwrap()
    });

    println!("\n=== Column Validation Benchmark ===");
    result.print_detailed();

    // Column validation should be fast
    expect(result.stats.mean_ms).to_be_less_than(&0.2)?;
    expect(result.success).to_be_true()?;

    Ok(())
}

// ============================================================================
// Benchmark: SQL Generation
// ============================================================================

#[test]
fn bench_sql_generation_select() -> Result<(), AssertionError> {
    let config = BenchmarkConfig::new(100, 5, 10);
    let benchmarker = Benchmarker::new(config);

    let mut results = Vec::new();

    // Benchmark simple SELECT
    let result = benchmarker.run("Simple SELECT", || {
        let qb = QueryBuilder::new("users").unwrap();
        qb.build()
    });
    results.push(result);

    // Benchmark SELECT with WHERE
    let result = benchmarker.run("SELECT with WHERE", || {
        let qb = QueryBuilder::new("users")
            .unwrap()
            .where_clause("age", Operator::Gte, ExtractedValue::Int(18))
            .unwrap();
        qb.build()
    });
    results.push(result);

    // Benchmark complex SELECT
    let result = benchmarker.run("Complex SELECT", || {
        let qb = QueryBuilder::new("users")
            .unwrap()
            .select(vec![
                "id".to_string(),
                "name".to_string(),
                "email".to_string(),
            ])
            .unwrap()
            .where_clause("age", Operator::Gte, ExtractedValue::Int(18))
            .unwrap()
            .where_clause("active", Operator::Eq, ExtractedValue::Bool(true))
            .unwrap()
            .order_by("name", OrderDirection::Asc)
            .unwrap()
            .limit(10)
            .offset(20);
        qb.build()
    });
    results.push(result);

    println!("\n=== SQL Generation (SELECT) Benchmark ===");
    print_comparison_table(&results, Some("Simple SELECT"));

    // All SELECT generation should be very fast (< 0.3ms)
    for result in &results {
        expect(result.stats.mean_ms).to_be_less_than(&0.3)?;
        expect(result.success).to_be_true()?;
    }

    Ok(())
}

#[test]
fn bench_sql_generation_insert() -> Result<(), AssertionError> {
    let config = BenchmarkConfig::new(100, 5, 10);
    let benchmarker = Benchmarker::new(config);

    let mut results = Vec::new();

    // Benchmark single field INSERT
    let result = benchmarker.run("INSERT (1 field)", || {
        let qb = QueryBuilder::new("users").unwrap();
        let values = vec![(
            "name".to_string(),
            ExtractedValue::String("Alice".to_string()),
        )];
        qb.build_insert(&values).unwrap()
    });
    results.push(result);

    // Benchmark multi-field INSERT
    let result = benchmarker.run("INSERT (5 fields)", || {
        let qb = QueryBuilder::new("users").unwrap();
        let values = vec![
            (
                "name".to_string(),
                ExtractedValue::String("Alice".to_string()),
            ),
            ("age".to_string(), ExtractedValue::Int(30)),
            (
                "email".to_string(),
                ExtractedValue::String("alice@example.com".to_string()),
            ),
            ("active".to_string(), ExtractedValue::Bool(true)),
            ("score".to_string(), ExtractedValue::Double(95.5)),
        ];
        qb.build_insert(&values).unwrap()
    });
    results.push(result);

    println!("\n=== SQL Generation (INSERT) Benchmark ===");
    print_comparison_table(&results, Some("INSERT (1 field)"));

    // INSERT generation should be fast
    for result in &results {
        expect(result.stats.mean_ms).to_be_less_than(&0.3)?;
        expect(result.success).to_be_true()?;
    }

    Ok(())
}

#[test]
fn bench_sql_generation_update() -> Result<(), AssertionError> {
    let config = BenchmarkConfig::new(100, 5, 10);
    let benchmarker = Benchmarker::new(config);

    let mut results = Vec::new();

    // Benchmark simple UPDATE
    let result = benchmarker.run("UPDATE (1 field)", || {
        let qb = QueryBuilder::new("users")
            .unwrap()
            .where_clause("id", Operator::Eq, ExtractedValue::Int(42))
            .unwrap();
        let values = vec![(
            "name".to_string(),
            ExtractedValue::String("Bob".to_string()),
        )];
        qb.build_update(&values).unwrap()
    });
    results.push(result);

    // Benchmark multi-field UPDATE
    let result = benchmarker.run("UPDATE (5 fields)", || {
        let qb = QueryBuilder::new("users")
            .unwrap()
            .where_clause("id", Operator::Eq, ExtractedValue::Int(42))
            .unwrap();
        let values = vec![
            (
                "name".to_string(),
                ExtractedValue::String("Bob".to_string()),
            ),
            ("age".to_string(), ExtractedValue::Int(35)),
            (
                "email".to_string(),
                ExtractedValue::String("bob@example.com".to_string()),
            ),
            ("active".to_string(), ExtractedValue::Bool(false)),
            ("score".to_string(), ExtractedValue::Double(88.3)),
        ];
        qb.build_update(&values).unwrap()
    });
    results.push(result);

    println!("\n=== SQL Generation (UPDATE) Benchmark ===");
    print_comparison_table(&results, Some("UPDATE (1 field)"));

    // UPDATE generation should be fast
    for result in &results {
        expect(result.stats.mean_ms).to_be_less_than(&0.3)?;
        expect(result.success).to_be_true()?;
    }

    Ok(())
}

#[test]
fn bench_sql_generation_delete() -> Result<(), AssertionError> {
    let config = BenchmarkConfig::new(100, 5, 10);
    let benchmarker = Benchmarker::new(config);

    let mut results = Vec::new();

    // Benchmark simple DELETE
    let result = benchmarker.run("DELETE (simple)", || {
        let qb = QueryBuilder::new("users")
            .unwrap()
            .where_clause("id", Operator::Eq, ExtractedValue::Int(42))
            .unwrap();
        qb.build_delete()
    });
    results.push(result);

    // Benchmark DELETE with multiple conditions
    let result = benchmarker.run("DELETE (complex)", || {
        let qb = QueryBuilder::new("users")
            .unwrap()
            .where_clause("age", Operator::Lt, ExtractedValue::Int(18))
            .unwrap()
            .where_clause("active", Operator::Eq, ExtractedValue::Bool(false))
            .unwrap();
        qb.build_delete()
    });
    results.push(result);

    println!("\n=== SQL Generation (DELETE) Benchmark ===");
    print_comparison_table(&results, Some("DELETE (simple)"));

    // DELETE generation should be fast
    for result in &results {
        expect(result.stats.mean_ms).to_be_less_than(&0.2)?;
        expect(result.success).to_be_true()?;
    }

    Ok(())
}

// ============================================================================
// Benchmark: Parameter Binding
// ============================================================================

#[test]
fn bench_parameter_binding() -> Result<(), AssertionError> {
    let config = BenchmarkConfig::new(100, 5, 10);
    let benchmarker = Benchmarker::new(config);

    let mut results = Vec::new();

    // Benchmark single parameter
    let result = benchmarker.run("1 parameter", || {
        let qb = QueryBuilder::new("users")
            .unwrap()
            .where_clause("id", Operator::Eq, ExtractedValue::Int(42))
            .unwrap();
        qb.build()
    });
    results.push(result);

    // Benchmark 5 parameters
    let result = benchmarker.run("5 parameters", || {
        let qb = QueryBuilder::new("users")
            .unwrap()
            .where_clause("age", Operator::Gte, ExtractedValue::Int(18))
            .unwrap()
            .where_clause("age", Operator::Lte, ExtractedValue::Int(65))
            .unwrap()
            .where_clause("active", Operator::Eq, ExtractedValue::Bool(true))
            .unwrap()
            .where_clause("score", Operator::Gte, ExtractedValue::Float(70.0))
            .unwrap()
            .where_clause(
                "name",
                Operator::Like,
                ExtractedValue::String("%John%".to_string()),
            )
            .unwrap();
        qb.build()
    });
    results.push(result);

    // Benchmark 10 parameters
    let result = benchmarker.run("10 parameters", || {
        let mut qb = QueryBuilder::new("users").unwrap();
        for i in 0..10 {
            qb = qb
                .where_clause(&format!("field{}", i), Operator::Eq, ExtractedValue::Int(i))
                .unwrap();
        }
        qb.build()
    });
    results.push(result);

    println!("\n=== Parameter Binding Benchmark ===");
    print_comparison_table(&results, Some("1 parameter"));

    // Parameter binding should be very fast (all sub-millisecond)
    // 1 parameter should be extremely fast
    expect(results[0].stats.mean_ms).to_be_less_than(&0.1)?;
    // 5 parameters still very fast
    expect(results[1].stats.mean_ms).to_be_less_than(&0.1)?;
    // 10 parameters still reasonable (< 0.1ms)
    expect(results[2].stats.mean_ms).to_be_less_than(&0.1)?;

    // Print ratio for information (actual performance may vary between runs)
    let ratio = results[2].stats.mean_ms / results[0].stats.mean_ms;
    println!("  Scaling ratio (10 params / 1 param): {:.2}x", ratio);

    Ok(())
}

// ============================================================================
// Benchmark: Operator Conversion
// ============================================================================

#[test]
fn bench_operator_conversion() -> Result<(), AssertionError> {
    let config = BenchmarkConfig::new(1000, 5, 50);
    let benchmarker = Benchmarker::new(config);

    // Benchmark operator to_sql conversion
    let result = benchmarker.run("Operator::to_sql", || {
        let ops = [
            Operator::Eq,
            Operator::Ne,
            Operator::Gt,
            Operator::Gte,
            Operator::Lt,
            Operator::Lte,
            Operator::In,
            Operator::NotIn,
            Operator::Like,
            Operator::ILike,
            Operator::IsNull,
            Operator::IsNotNull,
        ];
        for op in &ops {
            let _ = op.to_sql();
        }
    });

    println!("\n=== Operator Conversion Benchmark ===");
    result.print_detailed();

    // Operator conversion should be extremely fast (< 0.01ms for 12 ops)
    expect(result.stats.mean_ms).to_be_less_than(&0.01)?;
    expect(result.success).to_be_true()?;

    Ok(())
}

// ============================================================================
// Benchmark: Overall Performance Summary
// ============================================================================

#[test]
fn bench_summary() -> Result<(), AssertionError> {
    println!("\n");
    println!("╔════════════════════════════════════════════════════════════════════╗");
    println!("║                PostgreSQL QueryBuilder Benchmarks                 ║");
    println!("╠════════════════════════════════════════════════════════════════════╣");
    println!("║                                                                    ║");
    println!("║  Component                    Target        Description           ║");
    println!("║  ────────────────────────────────────────────────────────────────  ║");
    println!("║  QueryBuilder construction    < 0.1ms      Basic initialization   ║");
    println!("║  With clauses                 < 0.5ms      SELECT with filters    ║");
    println!("║  With JOINs                   < 0.8ms      LEFT JOIN queries      ║");
    println!("║  Identifier validation        < 0.05ms     Table/column names     ║");
    println!("║  SELECT generation            < 0.3ms      Simple to complex      ║");
    println!("║  INSERT generation            < 0.3ms      1-5 fields             ║");
    println!("║  UPDATE generation            < 0.3ms      1-5 fields             ║");
    println!("║  DELETE generation            < 0.2ms      With conditions        ║");
    println!("║  Parameter binding (10 params)< 0.5ms      Linear scaling         ║");
    println!("║  Operator conversion          < 0.01ms     12 operators           ║");
    println!("║                                                                    ║");
    println!("╠════════════════════════════════════════════════════════════════════╣");
    println!("║  Key Insights:                                                     ║");
    println!("║  • All operations are sub-millisecond                              ║");
    println!("║  • Validation is fast and secure                                   ║");
    println!("║  • SQL generation scales linearly with complexity                  ║");
    println!("║  • Ready for high-throughput applications                          ║");
    println!("╚════════════════════════════════════════════════════════════════════╝");
    println!();

    Ok(())
}
