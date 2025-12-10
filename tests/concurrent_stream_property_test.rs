// Feature: mysql-mcp-multi-datasource, Property 22: Concurrent stream isolation
// Validates: Requirements 9.5

use mysql_mcp_server::tools::{ColumnMetadata, QueryResultStream, QueryRow, StreamManager};
use proptest::prelude::*;
use std::sync::Arc;
use tokio::task::JoinSet;

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
    prop::collection::vec(arbitrary_column_metadata(), 1..5)
}

/// Strategy to generate arbitrary query rows
fn arbitrary_query_row(num_columns: usize) -> impl Strategy<Value = QueryRow> {
    prop::collection::vec(
        prop_oneof![
            Just(serde_json::Value::Null),
            any::<i32>().prop_map(|v| serde_json::json!(v)),
            "[a-zA-Z0-9]{0,20}".prop_map(|v| serde_json::json!(v)),
        ],
        num_columns..=num_columns,
    )
    .prop_map(|values| QueryRow { values })
}

/// Strategy to generate a row count (0 to 5000)
fn arbitrary_row_count() -> impl Strategy<Value = usize> {
    0usize..=5000usize
}

/// Strategy to generate a chunk size (1 to 2000)
fn arbitrary_chunk_size() -> impl Strategy<Value = usize> {
    1usize..=2000usize
}

/// Strategy to generate number of concurrent streams (2 to 10)
fn arbitrary_stream_count() -> impl Strategy<Value = usize> {
    2usize..=10usize
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Property 22: Concurrent stream isolation
    /// For any two concurrent query streams, they should operate independently
    /// without interference.
    ///
    /// This test verifies that:
    /// 1. Multiple streams can be created and registered simultaneously
    /// 2. Each stream maintains its own independent position
    /// 3. Consuming chunks from one stream doesn't affect other streams
    /// 4. Cancelling one stream doesn't affect other streams
    /// 5. Each stream delivers its own complete set of rows
    #[test]
    fn test_concurrent_stream_isolation(
        columns in arbitrary_columns(),
        chunk_size in arbitrary_chunk_size(),
        stream_count in arbitrary_stream_count(),
        row_counts in prop::collection::vec(arbitrary_row_count(), 2..=10)
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            let manager = Arc::new(StreamManager::new());
            let num_columns = columns.len();
            
            // Take only the number of row counts we need
            let row_counts = &row_counts[..stream_count.min(row_counts.len())];
            
            // Create multiple streams with different row counts
            let mut stream_ids = Vec::new();
            let mut expected_totals = Vec::new();
            
            for &row_count in row_counts {
                let rows = (0..row_count)
                    .map(|i| QueryRow {
                        values: vec![serde_json::json!(i); num_columns],
                    })
                    .collect::<Vec<_>>();
                
                let stream = QueryResultStream::new(columns.clone(), rows, chunk_size);
                let stream_id = manager.register_stream(stream).await;
                
                stream_ids.push(stream_id);
                expected_totals.push(row_count);
            }
            
            // Property 1: All streams should be registered
            prop_assert_eq!(
                manager.active_stream_count().await,
                stream_ids.len(),
                "All streams should be registered"
            );
            
            // Property 2: Each stream should have its own independent position (all at 0)
            for stream_id in &stream_ids {
                let stream = manager.get_stream(stream_id).await;
                prop_assert!(stream.is_some(), "Stream should exist");
                let stream = stream.unwrap();
                prop_assert_eq!(
                    stream.current_position().await,
                    0,
                    "All streams should start at position 0"
                );
            }
            
            // Property 3: Consume one chunk from each stream in round-robin fashion
            // and verify positions are independent
            let mut consumed_rows = vec![0; stream_ids.len()];
            
            for (i, stream_id) in stream_ids.iter().enumerate() {
                let stream = manager.get_stream(stream_id).await.unwrap();
                
                // Consume one chunk
                if let Ok(Some(chunk)) = stream.next_chunk().await {
                    consumed_rows[i] += chunk.rows.len();
                    
                    // Verify this stream's position updated
                    prop_assert_eq!(
                        stream.current_position().await,
                        consumed_rows[i],
                        "Stream {} position should be updated",
                        i
                    );
                }
                
                // Verify other streams' positions are unchanged
                for (j, other_id) in stream_ids.iter().enumerate() {
                    if i != j {
                        let other_stream = manager.get_stream(other_id).await.unwrap();
                        prop_assert_eq!(
                            other_stream.current_position().await,
                            consumed_rows[j],
                            "Stream {} position should be unchanged when consuming from stream {}",
                            j,
                            i
                        );
                    }
                }
            }
            
            // Property 4: Cancel one stream and verify others are unaffected
            if !stream_ids.is_empty() {
                let cancelled_idx = 0;
                let cancelled_id = &stream_ids[cancelled_idx];
                let cancelled_stream = manager.get_stream(cancelled_id).await.unwrap();
                
                cancelled_stream.cancel().await.unwrap();
                prop_assert!(
                    cancelled_stream.is_cancelled().await,
                    "Cancelled stream should be marked as cancelled"
                );
                
                // Verify other streams are still active
                for (i, stream_id) in stream_ids.iter().enumerate() {
                    if i != cancelled_idx {
                        let stream = manager.get_stream(stream_id).await.unwrap();
                        prop_assert!(
                            !stream.is_cancelled().await,
                            "Stream {} should not be cancelled",
                            i
                        );
                        
                        // Verify stream is still functional by checking it's not cancelled
                        // (we don't consume here to avoid affecting the final count)
                    }
                }
            }
            
            // Property 5: Consume all remaining chunks from non-cancelled streams
            // and verify each delivers its complete set of rows
            for (i, stream_id) in stream_ids.iter().enumerate() {
                if i == 0 {
                    // Skip the cancelled stream - verify it returns error
                    let stream = manager.get_stream(stream_id).await.unwrap();
                    let result = stream.next_chunk().await;
                    prop_assert!(
                        result.is_err(),
                        "Cancelled stream should return error"
                    );
                    continue;
                }
                
                let stream = manager.get_stream(stream_id).await.unwrap();
                
                loop {
                    match stream.next_chunk().await {
                        Ok(Some(chunk)) => {
                            consumed_rows[i] += chunk.rows.len();
                        }
                        Ok(None) => break,
                        Err(_) => break,
                    }
                }
                
                prop_assert_eq!(
                    consumed_rows[i],
                    expected_totals[i],
                    "Stream {} should deliver all {} rows",
                    i,
                    expected_totals[i]
                );
            }
            
            Ok(())
        })?;
    }

    /// Property 22b: Concurrent consumption without interference
    /// Verify that multiple streams can be consumed concurrently in parallel
    /// without any data corruption or interference
    #[test]
    fn test_concurrent_consumption_parallel(
        columns in arbitrary_columns(),
        chunk_size in arbitrary_chunk_size(),
        stream_count in 2usize..=5usize,
        row_count in 100usize..=1000usize
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            let manager = Arc::new(StreamManager::new());
            let num_columns = columns.len();
            
            // Create multiple streams with the same row count
            let mut stream_ids = Vec::new();
            
            for _ in 0..stream_count {
                let rows = (0..row_count)
                    .map(|i| QueryRow {
                        values: vec![serde_json::json!(i); num_columns],
                    })
                    .collect::<Vec<_>>();
                
                let stream = QueryResultStream::new(columns.clone(), rows, chunk_size);
                let stream_id = manager.register_stream(stream).await;
                stream_ids.push(stream_id);
            }
            
            // Consume all streams concurrently using tasks
            let mut tasks = JoinSet::new();
            
            for stream_id in stream_ids.clone() {
                let manager_clone = Arc::clone(&manager);
                
                tasks.spawn(async move {
                    let stream = manager_clone.get_stream(&stream_id).await.unwrap();
                    let mut total_rows = 0;
                    let mut chunk_count = 0;
                    
                    loop {
                        match stream.next_chunk().await {
                            Ok(Some(chunk)) => {
                                total_rows += chunk.rows.len();
                                chunk_count += 1;
                            }
                            Ok(None) => break,
                            Err(e) => return Err(format!("Stream error: {:?}", e)),
                        }
                    }
                    
                    Ok((total_rows, chunk_count))
                });
            }
            
            // Wait for all tasks to complete
            let mut results = Vec::new();
            while let Some(result) = tasks.join_next().await {
                let task_result = result.unwrap();
                prop_assert!(task_result.is_ok(), "Task should complete successfully");
                results.push(task_result.unwrap());
            }
            
            // Property: All streams should deliver the same number of rows
            for (total_rows, _) in &results {
                prop_assert_eq!(
                    *total_rows,
                    row_count,
                    "Each stream should deliver all {} rows",
                    row_count
                );
            }
            
            // Property: All streams should have the same number of chunks
            let expected_chunks = if row_count == 0 {
                0
            } else {
                (row_count + chunk_size - 1) / chunk_size
            };
            
            for (_, chunk_count) in &results {
                prop_assert_eq!(
                    *chunk_count,
                    expected_chunks,
                    "Each stream should have {} chunks",
                    expected_chunks
                );
            }
            
            Ok(())
        })?;
    }

    /// Property 22c: Stream manager isolation
    /// Verify that operations on the stream manager (register, remove)
    /// don't interfere with active streams
    #[test]
    fn test_stream_manager_operations_isolation(
        columns in arbitrary_columns(),
        chunk_size in arbitrary_chunk_size(),
        row_count in 100usize..=1000usize
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            let manager = Arc::new(StreamManager::new());
            let num_columns = columns.len();
            
            // Create first stream
            let rows1 = (0..row_count)
                .map(|i| QueryRow {
                    values: vec![serde_json::json!(i); num_columns],
                })
                .collect::<Vec<_>>();
            
            let stream1 = QueryResultStream::new(columns.clone(), rows1, chunk_size);
            let stream_id1 = manager.register_stream(stream1).await;
            
            // Get stream and consume first chunk
            let stream1_ref = manager.get_stream(&stream_id1).await.unwrap();
            let chunk1 = stream1_ref.next_chunk().await.unwrap();
            prop_assert!(chunk1.is_some(), "First stream should have data");
            
            let position_after_first_chunk = stream1_ref.current_position().await;
            
            // Register a second stream while first is active
            let rows2 = (0..row_count)
                .map(|i| QueryRow {
                    values: vec![serde_json::json!(i * 2); num_columns],
                })
                .collect::<Vec<_>>();
            
            let stream2 = QueryResultStream::new(columns.clone(), rows2, chunk_size);
            let stream_id2 = manager.register_stream(stream2).await;
            
            // Property: First stream's position should be unchanged
            prop_assert_eq!(
                stream1_ref.current_position().await,
                position_after_first_chunk,
                "First stream position should be unchanged after registering second stream"
            );
            
            // Property: Second stream should start at position 0
            let stream2_ref = manager.get_stream(&stream_id2).await.unwrap();
            prop_assert_eq!(
                stream2_ref.current_position().await,
                0,
                "Second stream should start at position 0"
            );
            
            // Consume from second stream
            let chunk2 = stream2_ref.next_chunk().await.unwrap();
            prop_assert!(chunk2.is_some(), "Second stream should have data");
            
            // Property: First stream should still be at same position
            prop_assert_eq!(
                stream1_ref.current_position().await,
                position_after_first_chunk,
                "First stream position should be unchanged after consuming from second stream"
            );
            
            // Remove second stream
            manager.remove_stream(&stream_id2).await.unwrap();
            
            // Property: First stream should still be accessible and functional
            prop_assert_eq!(
                stream1_ref.current_position().await,
                position_after_first_chunk,
                "First stream position should be unchanged after removing second stream"
            );
            
            let chunk1_next = stream1_ref.next_chunk().await;
            prop_assert!(
                chunk1_next.is_ok(),
                "First stream should still be consumable after removing second stream"
            );
            
            Ok(())
        })?;
    }

    /// Property 22d: Interleaved consumption
    /// Verify that chunks can be consumed in an interleaved manner from
    /// multiple streams without any interference
    #[test]
    fn test_interleaved_consumption(
        columns in arbitrary_columns(),
        chunk_size in arbitrary_chunk_size(),
        row_count1 in 100usize..=500usize,
        row_count2 in 100usize..=500usize,
        row_count3 in 100usize..=500usize
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            let manager = Arc::new(StreamManager::new());
            let num_columns = columns.len();
            
            // Create three streams with different row counts
            let rows1 = (0..row_count1)
                .map(|i| QueryRow {
                    values: vec![serde_json::json!(i); num_columns],
                })
                .collect::<Vec<_>>();
            
            let rows2 = (0..row_count2)
                .map(|i| QueryRow {
                    values: vec![serde_json::json!(i * 10); num_columns],
                })
                .collect::<Vec<_>>();
            
            let rows3 = (0..row_count3)
                .map(|i| QueryRow {
                    values: vec![serde_json::json!(i * 100); num_columns],
                })
                .collect::<Vec<_>>();
            
            let stream1 = QueryResultStream::new(columns.clone(), rows1, chunk_size);
            let stream2 = QueryResultStream::new(columns.clone(), rows2, chunk_size);
            let stream3 = QueryResultStream::new(columns.clone(), rows3, chunk_size);
            
            let id1 = manager.register_stream(stream1).await;
            let id2 = manager.register_stream(stream2).await;
            let id3 = manager.register_stream(stream3).await;
            
            let s1 = manager.get_stream(&id1).await.unwrap();
            let s2 = manager.get_stream(&id2).await.unwrap();
            let s3 = manager.get_stream(&id3).await.unwrap();
            
            let mut total1 = 0;
            let mut total2 = 0;
            let mut total3 = 0;
            
            // Consume chunks in interleaved pattern: s1, s2, s3, s1, s2, s3, ...
            // Calculate max iterations needed: max(row_count1, row_count2, row_count3) / chunk_size + 1
            let max_rows = row_count1.max(row_count2).max(row_count3);
            let max_iterations = (max_rows + chunk_size - 1) / chunk_size + 10; // Add buffer for safety
            let mut iteration = 0;
            
            loop {
                iteration += 1;
                if iteration > max_iterations {
                    break;
                }
                
                let mut any_active = false;
                
                // Try to consume from stream 1
                if let Ok(Some(chunk)) = s1.next_chunk().await {
                    total1 += chunk.rows.len();
                    any_active = true;
                }
                
                // Try to consume from stream 2
                if let Ok(Some(chunk)) = s2.next_chunk().await {
                    total2 += chunk.rows.len();
                    any_active = true;
                }
                
                // Try to consume from stream 3
                if let Ok(Some(chunk)) = s3.next_chunk().await {
                    total3 += chunk.rows.len();
                    any_active = true;
                }
                
                if !any_active {
                    break;
                }
            }
            
            // Property: Each stream should deliver all its rows
            prop_assert_eq!(
                total1,
                row_count1,
                "Stream 1 should deliver all {} rows",
                row_count1
            );
            
            prop_assert_eq!(
                total2,
                row_count2,
                "Stream 2 should deliver all {} rows",
                row_count2
            );
            
            prop_assert_eq!(
                total3,
                row_count3,
                "Stream 3 should deliver all {} rows",
                row_count3
            );
            
            // Property: All streams should be fully consumed
            prop_assert!(
                s1.next_chunk().await.unwrap().is_none(),
                "Stream 1 should be fully consumed"
            );
            
            prop_assert!(
                s2.next_chunk().await.unwrap().is_none(),
                "Stream 2 should be fully consumed"
            );
            
            prop_assert!(
                s3.next_chunk().await.unwrap().is_none(),
                "Stream 3 should be fully consumed"
            );
            
            Ok(())
        })?;
    }

    /// Property 22e: Stream data integrity
    /// Verify that the data from each stream is correct and not mixed with
    /// data from other streams
    #[test]
    fn test_stream_data_integrity(
        columns in arbitrary_columns(),
        chunk_size in arbitrary_chunk_size(),
        row_count in 10usize..=100usize
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        
        rt.block_on(async {
            let manager = Arc::new(StreamManager::new());
            let num_columns = columns.len();
            
            // Create two streams with distinct, predictable data
            // Stream 1: values are row index
            // Stream 2: values are row index * 1000
            let rows1 = (0..row_count)
                .map(|i| QueryRow {
                    values: vec![serde_json::json!(i); num_columns],
                })
                .collect::<Vec<_>>();
            
            let rows2 = (0..row_count)
                .map(|i| QueryRow {
                    values: vec![serde_json::json!(i * 1000); num_columns],
                })
                .collect::<Vec<_>>();
            
            let stream1 = QueryResultStream::new(columns.clone(), rows1, chunk_size);
            let stream2 = QueryResultStream::new(columns.clone(), rows2, chunk_size);
            
            let id1 = manager.register_stream(stream1).await;
            let id2 = manager.register_stream(stream2).await;
            
            let s1 = manager.get_stream(&id1).await.unwrap();
            let s2 = manager.get_stream(&id2).await.unwrap();
            
            let mut row_index1 = 0;
            let mut row_index2 = 0;
            
            // Consume all chunks from both streams in interleaved fashion
            loop {
                let mut any_active = false;
                
                // Consume from stream 1
                if let Ok(Some(chunk)) = s1.next_chunk().await {
                    any_active = true;
                    
                    // Verify data integrity: each row should have value equal to its index
                    for row in &chunk.rows {
                        if let Some(serde_json::Value::Number(n)) = row.values.first() {
                            let value = n.as_i64().unwrap_or(-1);
                            prop_assert_eq!(
                                value,
                                row_index1 as i64,
                                "Stream 1 row {} should have value {}",
                                row_index1,
                                row_index1
                            );
                            row_index1 += 1;
                        }
                    }
                }
                
                // Consume from stream 2
                if let Ok(Some(chunk)) = s2.next_chunk().await {
                    any_active = true;
                    
                    // Verify data integrity: each row should have value equal to index * 1000
                    for row in &chunk.rows {
                        if let Some(serde_json::Value::Number(n)) = row.values.first() {
                            let value = n.as_i64().unwrap_or(-1);
                            prop_assert_eq!(
                                value,
                                (row_index2 * 1000) as i64,
                                "Stream 2 row {} should have value {}",
                                row_index2,
                                row_index2 * 1000
                            );
                            row_index2 += 1;
                        }
                    }
                }
                
                if !any_active {
                    break;
                }
            }
            
            // Property: All rows from both streams should be accounted for
            prop_assert_eq!(
                row_index1,
                row_count,
                "Stream 1 should have delivered all {} rows",
                row_count
            );
            
            prop_assert_eq!(
                row_index2,
                row_count,
                "Stream 2 should have delivered all {} rows",
                row_count
            );
            
            Ok(())
        })?;
    }
}
