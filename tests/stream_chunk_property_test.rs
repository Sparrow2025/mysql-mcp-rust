// Feature: mysql-mcp-multi-datasource, Property 21: Stream chunk size limit
// Validates: Requirements 9.2

use mysql_mcp_server::tools::{ColumnMetadata, QueryResultStream, QueryRow};
use proptest::prelude::*;

/// Strategy to generate arbitrary column metadata
fn arbitrary_column_metadata() -> impl Strategy<Value = ColumnMetadata> {
    ("[a-zA-Z_][a-zA-Z0-9_]{0,19}", "[A-Z]{3,10}", any::<bool>())
        .prop_map(|(name, data_type, nullable)| ColumnMetadata {
            name,
            data_type,
            nullable,
        })
}

/// Strategy to generate a vector of column metadata
fn arbitrary_columns() -> impl Strategy<Value = Vec<ColumnMetadata>> {
    prop::collection::vec(arbitrary_column_metadata(), 1..10)
}

/// Strategy to generate arbitrary query rows
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

/// Strategy to generate a vector of query rows
fn arbitrary_rows(num_columns: usize, row_count: usize) -> impl Strategy<Value = Vec<QueryRow>> {
    prop::collection::vec(arbitrary_query_row(num_columns), row_count..=row_count)
}

/// Strategy to generate a valid chunk size (1 to 10000)
fn arbitrary_chunk_size() -> impl Strategy<Value = usize> {
    1usize..=10000usize
}

/// Strategy to generate a row count (0 to 20000)
fn arbitrary_row_count() -> impl Strategy<Value = usize> {
    0usize..=20000usize
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Property 21: Stream chunk size limit
    /// For any streaming query result, each chunk should contain no more than
    /// the configured chunk size (typically 1000 rows as per requirements).
    ///
    /// This test verifies that:
    /// 1. No chunk exceeds the specified chunk size
    /// 2. All chunks except possibly the last one contain exactly chunk_size rows
    /// 3. The last chunk contains the remaining rows (which may be less than chunk_size)
    /// 4. All rows are eventually delivered through the chunks
    #[test]
    fn test_stream_chunk_size_limit(
        columns in arbitrary_columns(),
        chunk_size in arbitrary_chunk_size(),
        row_count in arbitrary_row_count()
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            // Generate rows with the same number of columns
            let num_columns = columns.len();
            let rows = (0..row_count)
                .map(|i| QueryRow {
                    values: vec![serde_json::json!(i); num_columns],
                })
                .collect::<Vec<_>>();
            
            // Create a stream with the specified chunk size
            let stream = QueryResultStream::new(columns.clone(), rows.clone(), chunk_size);
            
            let mut total_rows_received = 0;
            let mut chunk_count = 0;
            let mut all_chunks = Vec::new();
            
            // Consume all chunks
            loop {
                match stream.next_chunk().await {
                    Ok(Some(chunk)) => {
                        chunk_count += 1;
                        let chunk_row_count = chunk.rows.len();
                        
                        // Property: No chunk should exceed the chunk size
                        prop_assert!(
                            chunk_row_count <= chunk_size,
                            "Chunk {} has {} rows, which exceeds the chunk size limit of {}",
                            chunk_count - 1,
                            chunk_row_count,
                            chunk_size
                        );
                        
                        // Property: Non-last chunks should have exactly chunk_size rows
                        if !chunk.is_last {
                            prop_assert_eq!(
                                chunk_row_count,
                                chunk_size,
                                "Non-last chunk {} has {} rows, expected exactly {}",
                                chunk_count - 1,
                                chunk_row_count,
                                chunk_size
                            );
                        }
                        
                        // Property: Last chunk should have the remaining rows
                        if chunk.is_last {
                            let expected_last_chunk_size = if row_count == 0 {
                                0
                            } else {
                                let remainder = row_count % chunk_size;
                                if remainder == 0 { chunk_size } else { remainder }
                            };
                            
                            prop_assert_eq!(
                                chunk_row_count,
                                expected_last_chunk_size,
                                "Last chunk has {} rows, expected {}",
                                chunk_row_count,
                                expected_last_chunk_size
                            );
                        }
                        
                        // Verify chunk metadata
                        prop_assert_eq!(
                            chunk.columns.len(),
                            columns.len(),
                            "Chunk columns count mismatch"
                        );
                        
                        prop_assert_eq!(
                            chunk.total_rows,
                            row_count,
                            "Chunk total_rows metadata is incorrect"
                        );
                        
                        prop_assert_eq!(
                            chunk.chunk_number,
                            chunk_count - 1,
                            "Chunk number is incorrect"
                        );
                        
                        total_rows_received += chunk_row_count;
                        all_chunks.push(chunk);
                    }
                    Ok(None) => {
                        // Stream completed
                        break;
                    }
                    Err(e) => {
                        return Err(proptest::test_runner::TestCaseError::fail(
                            format!("Stream error: {:?}", e)
                        ));
                    }
                }
            }
            
            // Property: All rows should be delivered
            prop_assert_eq!(
                total_rows_received,
                row_count,
                "Total rows received ({}) doesn't match expected ({})",
                total_rows_received,
                row_count
            );
            
            // Property: Expected number of chunks
            let expected_chunks = if row_count == 0 {
                0
            } else {
                (row_count + chunk_size - 1) / chunk_size
            };
            
            prop_assert_eq!(
                chunk_count,
                expected_chunks,
                "Expected {} chunks, but got {}",
                expected_chunks,
                chunk_count
            );
            
            // Property: Only the last chunk should have is_last = true
            if !all_chunks.is_empty() {
                for (i, chunk) in all_chunks.iter().enumerate() {
                    if i == all_chunks.len() - 1 {
                        prop_assert!(
                            chunk.is_last,
                            "Last chunk should have is_last = true"
                        );
                    } else {
                        prop_assert!(
                            !chunk.is_last,
                            "Non-last chunk {} should have is_last = false",
                            i
                        );
                    }
                }
            }
            
            Ok(())
        })?;
    }

    /// Property 21b: Chunk size of 1000 (requirement default)
    /// Specifically test the requirement's default chunk size of 1000 rows
    #[test]
    fn test_stream_chunk_size_1000_rows(
        columns in arbitrary_columns(),
        row_count in 0usize..=10000usize
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            let chunk_size = 1000; // Requirement 9.2 specifies 1000 rows
            
            let num_columns = columns.len();
            let rows = (0..row_count)
                .map(|i| QueryRow {
                    values: vec![serde_json::json!(i); num_columns],
                })
                .collect::<Vec<_>>();
            
            let stream = QueryResultStream::new(columns, rows, chunk_size);
            
            // Consume all chunks and verify none exceed 1000 rows
            loop {
                match stream.next_chunk().await {
                    Ok(Some(chunk)) => {
                        prop_assert!(
                            chunk.rows.len() <= 1000,
                            "Chunk has {} rows, exceeds the 1000 row limit specified in Requirement 9.2",
                            chunk.rows.len()
                        );
                    }
                    Ok(None) => break,
                    Err(e) => {
                        return Err(proptest::test_runner::TestCaseError::fail(
                            format!("Stream error: {:?}", e)
                        ));
                    }
                }
            }
            
            Ok(())
        })?;
    }

    /// Property 21c: Empty result set
    /// Verify that empty result sets are handled correctly
    #[test]
    fn test_stream_empty_result_set(
        columns in arbitrary_columns(),
        chunk_size in arbitrary_chunk_size()
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            let rows = vec![];
            let stream = QueryResultStream::new(columns, rows, chunk_size);
            
            // First call should return None for empty result set
            let result = stream.next_chunk().await;
            prop_assert!(result.is_ok(), "Empty stream should not error");
            prop_assert!(result.unwrap().is_none(), "Empty stream should return None immediately");
            
            // Subsequent calls should also return None
            let result = stream.next_chunk().await;
            prop_assert!(result.is_ok(), "Empty stream should not error on subsequent calls");
            prop_assert!(result.unwrap().is_none(), "Empty stream should continue returning None");
            
            Ok(())
        })?;
    }

    /// Property 21d: Single row result set
    /// Verify that single row result sets are handled correctly
    #[test]
    fn test_stream_single_row(
        columns in arbitrary_columns(),
        chunk_size in arbitrary_chunk_size()
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            let num_columns = columns.len();
            let rows = vec![QueryRow {
                values: vec![serde_json::json!(42); num_columns],
            }];
            
            let stream = QueryResultStream::new(columns, rows, chunk_size);
            
            // First call should return the single row
            let result = stream.next_chunk().await;
            prop_assert!(result.is_ok(), "Single row stream should not error");
            
            let chunk = result.unwrap();
            prop_assert!(chunk.is_some(), "Single row stream should return a chunk");
            
            let chunk = chunk.unwrap();
            prop_assert_eq!(chunk.rows.len(), 1, "Chunk should contain exactly 1 row");
            prop_assert!(chunk.is_last, "Single chunk should be marked as last");
            prop_assert_eq!(chunk.chunk_number, 0, "First chunk should be numbered 0");
            prop_assert_eq!(chunk.total_rows, 1, "Total rows should be 1");
            
            // Second call should return None
            let result = stream.next_chunk().await;
            prop_assert!(result.is_ok(), "Stream should not error after completion");
            prop_assert!(result.unwrap().is_none(), "Stream should return None after all rows consumed");
            
            Ok(())
        })?;
    }

    /// Property 21e: Exact multiple of chunk size
    /// Verify behavior when row count is exactly a multiple of chunk size
    #[test]
    fn test_stream_exact_multiple_of_chunk_size(
        columns in arbitrary_columns(),
        chunk_size in 1usize..=1000usize,
        multiplier in 1usize..=10usize
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            let row_count = chunk_size * multiplier;
            
            let num_columns = columns.len();
            let rows = (0..row_count)
                .map(|i| QueryRow {
                    values: vec![serde_json::json!(i); num_columns],
                })
                .collect::<Vec<_>>();
            
            let stream = QueryResultStream::new(columns, rows, chunk_size);
            
            let mut chunk_count = 0;
            
            loop {
                match stream.next_chunk().await {
                    Ok(Some(chunk)) => {
                        chunk_count += 1;
                        
                        // All chunks should have exactly chunk_size rows
                        prop_assert_eq!(
                            chunk.rows.len(),
                            chunk_size,
                            "When row count is exact multiple, all chunks should have chunk_size rows"
                        );
                        
                        // Last chunk should be marked correctly
                        if chunk_count == multiplier {
                            prop_assert!(chunk.is_last, "Last chunk should be marked as last");
                        } else {
                            prop_assert!(!chunk.is_last, "Non-last chunk should not be marked as last");
                        }
                    }
                    Ok(None) => break,
                    Err(e) => {
                        return Err(proptest::test_runner::TestCaseError::fail(
                            format!("Stream error: {:?}", e)
                        ));
                    }
                }
            }
            
            prop_assert_eq!(
                chunk_count,
                multiplier,
                "Should have exactly {} chunks",
                multiplier
            );
            
            Ok(())
        })?;
    }

    /// Property 21f: Chunk size larger than result set
    /// Verify behavior when chunk size is larger than the total number of rows
    #[test]
    fn test_stream_chunk_size_larger_than_rows(
        columns in arbitrary_columns(),
        row_count in 1usize..=100usize
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            let chunk_size = row_count * 2; // Chunk size is larger than row count
            
            let num_columns = columns.len();
            let rows = (0..row_count)
                .map(|i| QueryRow {
                    values: vec![serde_json::json!(i); num_columns],
                })
                .collect::<Vec<_>>();
            
            let stream = QueryResultStream::new(columns, rows, chunk_size);
            
            // Should get exactly one chunk with all rows
            let result = stream.next_chunk().await;
            prop_assert!(result.is_ok(), "Stream should not error");
            
            let chunk = result.unwrap();
            prop_assert!(chunk.is_some(), "Should return a chunk");
            
            let chunk = chunk.unwrap();
            prop_assert_eq!(
                chunk.rows.len(),
                row_count,
                "Single chunk should contain all rows"
            );
            prop_assert!(chunk.is_last, "Single chunk should be marked as last");
            prop_assert_eq!(chunk.chunk_number, 0, "Should be chunk 0");
            
            // Next call should return None
            let result = stream.next_chunk().await;
            prop_assert!(result.unwrap().is_none(), "Should return None after single chunk");
            
            Ok(())
        })?;
    }
}
