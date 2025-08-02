//! Performance tests for layout calculation caching
//!
//! These tests verify that the layout caching system provides the expected
//! performance improvements and doesn't introduce memory leaks.

use liiga_teletext::ui::layout::LayoutCalculator;
use std::time::Instant;

#[test]
fn test_layout_cache_performance() {
    let mut calculator = LayoutCalculator::new();
    let terminal_size = (100, 30);

    // First calculation (cache miss)
    let start = Instant::now();
    let _layout1 = calculator.calculate_layout(terminal_size);
    let first_duration = start.elapsed();

    // Second calculation (cache hit)
    let start = Instant::now();
    let _layout2 = calculator.calculate_layout(terminal_size);
    let second_duration = start.elapsed();

    // Cache hit should be significantly faster
    // Allow some variance for system load, but cache hit should be at least 2x faster
    assert!(
        second_duration < first_duration / 2,
        "Cache hit ({:?}) should be faster than cache miss ({:?})",
        second_duration,
        first_duration
    );

    // Both should be reasonably fast (under 10ms)
    assert!(
        first_duration.as_millis() < 10,
        "Layout calculation too slow: {:?}",
        first_duration
    );
    assert!(
        second_duration.as_millis() < 5,
        "Cached layout calculation too slow: {:?}",
        second_duration
    );
}

#[test]
fn test_incremental_update_performance() {
    let mut calculator = LayoutCalculator::new();

    // Create initial layout
    let start = Instant::now();
    let _layout1 = calculator.calculate_layout((100, 30));
    let initial_duration = start.elapsed();

    // Small change that should trigger incremental update
    let start = Instant::now();
    let _layout2 = calculator.calculate_layout((103, 32));
    let incremental_duration = start.elapsed();

    // Incremental update should be fast
    assert!(
        incremental_duration.as_millis() < 5,
        "Incremental update too slow: {:?}",
        incremental_duration
    );

    // Should be faster than or similar to initial calculation
    assert!(
        incremental_duration <= initial_duration * 2,
        "Incremental update ({:?}) should not be much slower than initial ({:?})",
        incremental_duration,
        initial_duration
    );
}

#[test]
fn test_cache_memory_management() {
    let mut calculator = LayoutCalculator::new();

    // Fill cache with many different sizes
    for width in 80..130 {
        for height in 24..50 {
            calculator.calculate_layout((width, height));
        }
    }

    let stats = calculator.get_cache_stats();

    // Cache should not grow unbounded
    assert!(
        stats.total_entries <= 50,
        "Cache has too many entries: {}",
        stats.total_entries
    );

    // Should have some active entries
    assert!(stats.active_entries > 0, "Cache should have active entries");

    // Clear cache and verify it's empty
    calculator.clear_cache();
    let stats_after = calculator.get_cache_stats();
    assert_eq!(stats_after.total_entries, 0);
    assert_eq!(stats_after.active_entries, 0);
}

#[test]
fn test_string_buffer_pool_performance() {
    let mut calculator = LayoutCalculator::new();

    // Test buffer allocation performance
    let start = Instant::now();
    let mut buffers = Vec::new();

    // Allocate many buffers
    for _ in 0..100 {
        let buffer = calculator.get_string_buffer(1024);
        buffers.push(buffer);
    }

    let allocation_duration = start.elapsed();

    // Return buffers to pool
    let start = Instant::now();
    for buffer in buffers {
        calculator.return_string_buffer(buffer);
    }
    let return_duration = start.elapsed();

    // Operations should be fast
    assert!(
        allocation_duration.as_millis() < 10,
        "Buffer allocation too slow: {:?}",
        allocation_duration
    );
    assert!(
        return_duration.as_millis() < 10,
        "Buffer return too slow: {:?}",
        return_duration
    );

    // Test reuse performance
    let start = Instant::now();
    let _reused_buffer = calculator.get_string_buffer(1024);
    let reuse_duration = start.elapsed();

    // Reusing buffer should be very fast
    assert!(
        reuse_duration.as_micros() < 100,
        "Buffer reuse too slow: {:?}",
        reuse_duration
    );
}

#[test]
fn test_cache_cleanup_performance() {
    let mut calculator = LayoutCalculator::new();

    // Fill cache with entries
    for i in 80..130 {
        calculator.calculate_layout((i, 24));
    }

    // Force cache cleanup
    let start = Instant::now();
    calculator.clear_cache();
    let cleanup_duration = start.elapsed();

    // Cleanup should be fast
    assert!(
        cleanup_duration.as_millis() < 5,
        "Cache cleanup too slow: {:?}",
        cleanup_duration
    );

    // Cache should be empty after cleanup
    let stats = calculator.get_cache_stats();
    assert_eq!(stats.total_entries, 0);
}

#[test]
fn test_concurrent_layout_calculations() {
    let mut calculator = LayoutCalculator::new();

    // Simulate rapid resize events
    let sizes = vec![
        (80, 24),
        (90, 30),
        (100, 35),
        (110, 40),
        (120, 45),
        (110, 40),
        (100, 35),
        (90, 30),
        (80, 24),
    ];

    let start = Instant::now();

    for size in sizes {
        let _layout = calculator.calculate_layout(size);
    }

    let total_duration = start.elapsed();

    // All calculations should complete quickly
    assert!(
        total_duration.as_millis() < 50,
        "Rapid layout calculations too slow: {:?}",
        total_duration
    );

    // Cache should have reasonable number of entries
    let stats = calculator.get_cache_stats();
    assert!(stats.total_entries > 0);
    assert!(stats.total_entries <= 10); // Should not cache every single size
}

#[test]
fn test_memory_stability() {
    let mut calculator = LayoutCalculator::new();

    // Perform many operations to test for memory leaks
    for cycle in 0..10 {
        // Fill cache
        for i in 80..120 {
            calculator.calculate_layout((i, 24 + cycle));
        }

        // Use string buffers
        let mut buffers = Vec::new();
        for _ in 0..20 {
            buffers.push(calculator.get_string_buffer(1024));
        }
        for buffer in buffers {
            calculator.return_string_buffer(buffer);
        }

        // Periodic cleanup
        if cycle % 3 == 0 {
            calculator.clear_cache();
        }
    }

    // Final state should be reasonable
    let stats = calculator.get_cache_stats();
    assert!(stats.total_entries <= 50);
    assert!(stats.buffer_pool_size <= 8);
}
