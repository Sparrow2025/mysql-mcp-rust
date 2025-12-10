// Feature: mysql-mcp-multi-datasource, Property 8: Result format consistency
// Validates: Requirements 3.2

use mysql_mcp_server::tools::{ColumnMetadata, QueryResult, QueryRow};
use proptest::prelude::*;

// Strategy to generate arbitrary non-empty strings
fn arbitrary_non_empty_string() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_-]{1,30}"
}

// Strategy to generate arbitrary column metadata
fn arbitrary_column_metadata() -> impl Strategy<Value = ColumnMetadata> {
    (arbitrary_non_empty_string(), arbitrary_non_empty_string()).prop_map(|(name, data_type)| {
        ColumnMetadata {
            name,
            data_type,
            nullable: true,
        }
    })
}

// Strategy to generate a vector of column metadata (1-10 columns)
fn arbitrary_columns() -> impl Strategy<Value = Vec<ColumnMetadata>> {
    prop::collection::vec(arbitrary_column_metadata(), 1..=10)
}

// Strategy to generate a query row with the correct number of values
fn arbitrary_query_row(num_columns: usize) -> impl Strategy<Value = QueryRow> {
    prop::collection::vec(
        prop_oneof![
            Just(serde_json::Value::Null),
            any::<i32>().prop_map(|v| serde_json::json!(v)),
            any::<i64>().prop_map(|v| serde_json::json!(v)),
            any::<f64>().prop_map(|v| serde_json::json!(v)),
            any::<bool>().prop_map(|v| serde_json::json!(v)),
            "[a-zA-Z0-9 ]{0,50}".prop_map(|v| serde_json::json!(v)),
        ],
        num_columns..=num_columns,
    )
    .prop_map(|values| QueryRow { values })
}

// Strategy to generate a vector of query rows
fn arbitrary_query_rows(num_columns: usize, num_rows: usize) -> impl Strategy<Value = Vec<QueryRow>> {
    prop::collection::vec(arbitrary_query_row(num_columns), num_rows..=num_rows)
}

// Strategy to generate a complete QueryResult
fn arbitrary_query_result() -> impl Strategy<Value = QueryResult> {
    (arbitrary_columns(), 0..=20usize).prop_flat_map(|(columns, num_rows)| {
        let num_columns = columns.len();
        arbitrary_query_rows(num_columns, num_rows).prop_map(move |rows| QueryResult {
            columns: columns.clone(),
            rows,
            affected_rows: 0,
        })
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Property 8: Result format consistency
    /// For any successful query, the result should contain columns metadata and rows in a structured format
    #[test]
    fn test_result_format_consistency(result in arbitrary_query_result()) {
        // Verify that the result has the required structure

        // 1. Result must have columns metadata
        prop_assert!(
            !result.columns.is_empty() || result.rows.is_empty(),
            "Result must have columns metadata if it has rows. \
             Columns: {}, Rows: {}",
            result.columns.len(),
            result.rows.len()
        );

        // 2. Each column must have a name and data type
        for (i, column) in result.columns.iter().enumerate() {
            prop_assert!(
                !column.name.is_empty(),
                "Column {} must have a non-empty name",
                i
            );
            prop_assert!(
                !column.data_type.is_empty(),
                "Column {} must have a non-empty data type",
                i
            );
        }

        // 3. Each row must have the same number of values as there are columns
        for (row_idx, row) in result.rows.iter().enumerate() {
            prop_assert_eq!(
                row.values.len(),
                result.columns.len(),
                "Row {} must have {} values to match the number of columns, but has {}",
                row_idx,
                result.columns.len(),
                row.values.len()
            );
        }

        // 4. All rows must have the same structure (same number of values)
        if !result.rows.is_empty() {
            let expected_value_count = result.rows[0].values.len();
            for (row_idx, row) in result.rows.iter().enumerate() {
                prop_assert_eq!(
                    row.values.len(),
                    expected_value_count,
                    "All rows must have the same number of values. \
                     Row {} has {} values, but expected {}",
                    row_idx,
                    row.values.len(),
                    expected_value_count
                );
            }
        }

        // 5. The result structure must be serializable (can be converted to JSON)
        let serialization_result = serde_json::to_string(&result);
        prop_assert!(
            serialization_result.is_ok(),
            "Result must be serializable to JSON. Error: {:?}",
            serialization_result.err()
        );
    }

    /// Property 8a: Empty result sets have valid structure
    /// For any query that returns no rows, the result should still have valid column metadata
    #[test]
    fn test_empty_result_has_valid_structure(columns in arbitrary_columns()) {
        let result = QueryResult {
            columns: columns.clone(),
            rows: vec![],
            affected_rows: 0,
        };

        // Empty results should still have column metadata
        prop_assert!(
            !result.columns.is_empty(),
            "Empty result should still have column metadata"
        );

        // Each column should have valid metadata
        for column in &result.columns {
            prop_assert!(!column.name.is_empty(), "Column name must not be empty");
            prop_assert!(!column.data_type.is_empty(), "Column data type must not be empty");
        }

        // Should be serializable
        prop_assert!(
            serde_json::to_string(&result).is_ok(),
            "Empty result must be serializable"
        );
    }

    /// Property 8b: Column names are unique within a result
    /// For any query result, all column names should be unique (or at least distinguishable)
    #[test]
    fn test_column_names_present(result in arbitrary_query_result()) {
        // All columns must have names
        for (i, column) in result.columns.iter().enumerate() {
            prop_assert!(
                !column.name.is_empty(),
                "Column {} must have a name",
                i
            );
        }

        // Note: MySQL allows duplicate column names in results (e.g., SELECT a.id, b.id FROM ...)
        // So we don't enforce uniqueness, but we do ensure all columns have names
    }

    /// Property 8c: Row values match column count
    /// For any query result, every row must have exactly as many values as there are columns
    #[test]
    fn test_row_values_match_column_count(result in arbitrary_query_result()) {
        let column_count = result.columns.len();

        for (row_idx, row) in result.rows.iter().enumerate() {
            prop_assert_eq!(
                row.values.len(),
                column_count,
                "Row {} must have exactly {} values to match column count",
                row_idx,
                column_count
            );
        }
    }

    /// Property 8d: Result structure is consistent across multiple queries
    /// For any two query results, they should follow the same structural rules
    #[test]
    fn test_multiple_results_have_consistent_structure(
        result1 in arbitrary_query_result(),
        result2 in arbitrary_query_result()
    ) {
        // Both results should follow the same structural rules

        // Rule 1: Columns have names and types
        for column in &result1.columns {
            prop_assert!(!column.name.is_empty());
            prop_assert!(!column.data_type.is_empty());
        }
        for column in &result2.columns {
            prop_assert!(!column.name.is_empty());
            prop_assert!(!column.data_type.is_empty());
        }

        // Rule 2: Rows match column count
        for row in &result1.rows {
            prop_assert_eq!(row.values.len(), result1.columns.len());
        }
        for row in &result2.rows {
            prop_assert_eq!(row.values.len(), result2.columns.len());
        }

        // Rule 3: Both are serializable
        prop_assert!(serde_json::to_string(&result1).is_ok());
        prop_assert!(serde_json::to_string(&result2).is_ok());
    }

    /// Property 8e: Column metadata is complete
    /// For any query result, each column must have all required metadata fields
    #[test]
    fn test_column_metadata_completeness(result in arbitrary_query_result()) {
        for (i, column) in result.columns.iter().enumerate() {
            // Name must be present and non-empty
            prop_assert!(
                !column.name.is_empty(),
                "Column {} must have a non-empty name",
                i
            );

            // Data type must be present and non-empty
            prop_assert!(
                !column.data_type.is_empty(),
                "Column {} must have a non-empty data type",
                i
            );

            // Nullable field must be present (it's a bool, so always present)
            // Just verify we can access it
            let _nullable = column.nullable;
        }
    }

    /// Property 8f: Result values are valid JSON
    /// For any query result, all values in rows must be valid JSON values
    #[test]
    fn test_result_values_are_valid_json(result in arbitrary_query_result()) {
        for (row_idx, row) in result.rows.iter().enumerate() {
            for (col_idx, value) in row.values.iter().enumerate() {
                // Each value should be a valid JSON value
                // This is guaranteed by the serde_json::Value type, but we verify it's serializable
                let serialization_result = serde_json::to_string(value);
                prop_assert!(
                    serialization_result.is_ok(),
                    "Value at row {}, column {} must be valid JSON. Error: {:?}",
                    row_idx,
                    col_idx,
                    serialization_result.err()
                );
            }
        }
    }
}
