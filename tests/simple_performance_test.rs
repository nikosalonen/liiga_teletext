//! Simple performance test to verify the testing approach

use liiga_teletext::ui::layout::{DetailLevel, LayoutCalculator};
use std::time::{Duration, Instant};

#[tokio::test]
async fn test_simple_performance() {
    let mut calculator = LayoutCalculator::new();

    let start = Instant::now();
    let config = calculator.calculate_layout((100, 30));
    let duration = start.elapsed();

    // Should be very fast
    assert!(duration < Duration::from_millis(10));
    assert_eq!(config.detail_level, DetailLevel::Standard);

    println!("Layout calculation took: {:?}", duration);
}

#[tokio::test]
async fn test_repeated_calculations() {
    let mut calculator = LayoutCalculator::new();
    let iterations = 100;

    let start = Instant::now();
    for _ in 0..iterations {
        calculator.calculate_layout((100, 30));
    }
    let total_duration = start.elapsed();

    let avg_duration = total_duration / iterations;

    // Average should be very fast
    assert!(avg_duration < Duration::from_micros(100));

    println!(
        "Average calculation time over {} iterations: {:?}",
        iterations, avg_duration
    );
}
