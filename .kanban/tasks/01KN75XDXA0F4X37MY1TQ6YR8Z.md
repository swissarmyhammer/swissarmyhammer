---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffff8b80
title: Test model-embedding BatchProcessor process_batch and process_all
---
File: model-embedding/src/batch.rs (0% coverage, 0/176 lines)\n\nEntirely untested. Key functions:\n- BatchStats::record_batch() and throughput methods (lines 80-130)\n- BatchProcessor::new(), with_config(), set_progress_callback() (lines 192-217)\n- process_batch() - core batch embedding with error recovery, memory monitoring, retry logic (lines 220-323)\n- process_all() - splits input into batches and processes sequentially (lines 327-445)\n- estimate_memory_usage() (lines 469-530)\n\nThis is critical infrastructure for bulk embedding. Tests can use a mock TextEmbedder that returns fixed vectors. Test error recovery, memory limit enforcement, and progress callbacks." #coverage-gap