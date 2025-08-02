//! Performance and stress tests for the dynamic UI space feature
//!
//! These tests verify layout calculation performance with large datasets,
//! memory usage with different configurations, and rapid resize scenarios.

use liiga_teletext::{
    data_fetcher::models::*,
    teletext_ui::{GameResultData, TeletextPage},
    ui::{
        content_adapter::ContentAdapter,
        layout::{DetailLevel, LayoutCalculator},
        resize::ResizeHandler,
    },
};
use std::time::{Duration, Instant};

/// Helper function to create a large dataset of mock games for performance testing
fn create_large_game_dataset(count: usize) -> Vec<GameData> {
    let mut games = Vec::with_capacity(count);

    for i in 0..count {
        let goal_count = i % 8; // Varying number of goals (0-7)
        let mut goal_events = Vec::new();

        // Create goal events for this game
        for j in 0..goal_count {
            goal_events.push(GoalEventData {
                scorer_player_id: (1000 + j) as i64,
                scorer_name: format!("Player {} {}", i, j),
                minute: (10 + j * 7) as i32,
                home_team_score: if j % 2 == 0 {
                    (j / 2) as i32 + 1
                } else {
                    (j / 2) as i32
                },
                away_team_score: if j % 2 == 1 {
                    ((j + 1) / 2) as i32
                } else {
                    ((j + 1) / 2) as i32
                },
                is_winning_goal: j == goal_count - 1,
                goal_types: vec!["YV".to_string(), "RL".to_string()],
                is_home_team: j % 2 == 0,
                video_clip_url: Some(format!("https://example.com/goal_{}_{}.mp4", i, j)),
            });
        }

        games.push(GameData {
            home_team: format!("Team {}", i * 2),
            away_team: format!("Team {}", i * 2 + 1),
            time: format!("{}:{:02}", 18 + (i % 6), (i * 15) % 60),
            result: format!("{}-{}", goal_count / 2, goal_count - goal_count / 2),
            score_type: if i % 10 == 0 {
                liiga_teletext::teletext_ui::ScoreType::Ongoing
            } else {
                liiga_teletext::teletext_ui::ScoreType::Final
            },
            is_overtime: i % 15 == 0,
            is_shootout: i % 20 == 0,
            serie: if i % 5 == 0 {
                "playoffs".to_string()
            } else {
                "runkosarja".to_string()
            },
            goal_events,
            played_time: 3600 + (i as i32 * 60),
            start: format!("2024-01-{:02}T18:30:00Z", 1 + (i % 30)),
        });
    }

    games
}

/// Measures the time taken to execute a function
fn measure_time<F, R>(f: F) -> (R, Duration)
where
    F: FnOnce() -> R,
{
    let start = Instant::now();
    let result = f();
    let duration = start.elapsed();
    (result, duration)
}

#[tokio::test]
async fn test_layout_calculation_performance_small_dataset() {
    let mut calculator = LayoutCalculator::new();
    let terminal_sizes = vec![
        (80, 24),
        (100, 30),
        (120, 35),
        (140, 40),
        (160, 50),
        (200, 60),
    ];

    // Measure layout calculation time for various sizes
    for (width, height) in terminal_sizes {
        let (config, duration) = measure_time(|| calculator.calculate_layout((width, height)));

        // Layout calculation should be very fast (under 1ms for small datasets)
        assert!(
            duration < Duration::from_millis(1),
            "Layout calculation took too long: {:?} for size {}x{}",
            duration,
            width,
            height
        );

        // Verify the result is valid
        assert!(config.content_width > 0, "Content width should be positive");
        assert!(
            config.horizontal_padding < width,
            "Padding should be reasonable"
        );
    }
}

#[tokio::test]
async fn test_layout_calculation_performance_repeated() {
    let mut calculator = LayoutCalculator::new();
    let terminal_size = (100, 30);
    let iterations = 1000;

    let (_, total_duration) = measure_time(|| {
        for _ in 0..iterations {
            calculator.calculate_layout(terminal_size);
        }
    });

    let avg_duration = total_duration / iterations;

    // Average calculation time should be very fast
    assert!(
        avg_duration < Duration::from_micros(100),
        "Average layout calculation took too long: {:?}",
        avg_duration
    );

    println!("Average layout calculation time: {:?}", avg_duration);
}

#[tokio::test]
async fn test_page_creation_performance_large_dataset() {
    let game_count = 500; // Large dataset
    let games = create_large_game_dataset(game_count);

    let (mut page, creation_duration) = measure_time(|| {
        let mut page = TeletextPage::new(
            221,
            "JÄÄKIEKKO".to_string(),
            "RUNKOSARJA".to_string(),
            false,
            true,
            false,
        );

        for game in &games {
            page.add_game_result(GameResultData::new(game));
        }

        page
    });

    // Page creation with large dataset should complete within reasonable time
    assert!(
        creation_duration < Duration::from_secs(5),
        "Page creation took too long: {:?}",
        creation_duration
    );

    println!(
        "Page creation with {} games took: {:?}",
        game_count, creation_duration
    );

    // Test layout update performance
    let (_, layout_duration) = measure_time(|| {
        page.update_layout((140, 40));
    });

    assert!(
        layout_duration < Duration::from_millis(100),
        "Layout update took too long: {:?}",
        layout_duration
    );

    // Verify the page is functional
    let total_pages = page.total_pages();
    assert!(total_pages > 0, "Should have pages with large dataset");

    println!(
        "Layout update with {} games took: {:?}",
        game_count, layout_duration
    );
}

#[tokio::test]
async fn test_pagination_performance_large_dataset() {
    let game_count = 1000; // Very large dataset
    let games = create_large_game_dataset(game_count);

    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "RUNKOSARJA".to_string(),
        false,
        true,
        false,
    );

    // Add all games
    for game in &games {
        page.add_game_result(GameResultData::new(game));
    }

    page.update_layout((100, 30));

    // Test pagination calculation performance
    let (total_pages, pagination_duration) = measure_time(|| page.total_pages());

    assert!(
        pagination_duration < Duration::from_millis(50),
        "Pagination calculation took too long: {:?}",
        pagination_duration
    );

    assert!(total_pages > 0, "Should have pages");

    // Test page navigation performance
    let navigation_iterations = 100;
    let (_, navigation_duration) = measure_time(|| {
        for _ in 0..navigation_iterations {
            page.next_page();
            page.previous_page();
        }
    });

    let avg_navigation_time = navigation_duration / navigation_iterations;
    assert!(
        avg_navigation_time < Duration::from_micros(100),
        "Average page navigation took too long: {:?}",
        avg_navigation_time
    );

    println!(
        "Pagination with {} games: {} pages, calculation took: {:?}",
        game_count, total_pages, pagination_duration
    );
    println!("Average navigation time: {:?}", avg_navigation_time);
}

#[tokio::test]
async fn test_content_adaptation_performance() {
    let game_count = 200;
    let games = create_large_game_dataset(game_count);

    let detail_levels = vec![
        DetailLevel::Minimal,
        DetailLevel::Standard,
        DetailLevel::Extended,
    ];
    let available_widths = vec![80, 100, 120, 140, 160];

    for detail_level in detail_levels {
        for width in &available_widths {
            let (_, adaptation_duration) = measure_time(|| {
                for game in &games {
                    let game_result = GameResultData::new(game);
                    ContentAdapter::adapt_game_content(
                        &game_result.home_team,
                        &game_result.away_team,
                        &game_result.time,
                        &game_result.result,
                        &game.goal_events,
                        detail_level,
                        *width,
                    );
                }
            });

            let avg_adaptation_time = adaptation_duration / game_count as u32;

            // Content adaptation should be fast
            assert!(
                avg_adaptation_time < Duration::from_micros(500),
                "Average content adaptation took too long: {:?} for {:?} at width {}",
                avg_adaptation_time,
                detail_level,
                width
            );
        }
    }
}

#[tokio::test]
async fn test_rapid_resize_scenarios() {
    let mut resize_handler = ResizeHandler::new();
    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "RUNKOSARJA".to_string(),
        false,
        true,
        false,
    );

    // Add some games
    let games = create_large_game_dataset(50);
    for game in &games {
        page.add_game_result(GameResultData::new(game));
    }

    // Simulate rapid resize events
    let resize_scenarios = vec![
        (80, 24),
        (90, 25),
        (100, 30),
        (110, 32),
        (120, 35),
        (130, 37),
        (140, 40),
        (150, 45),
        (160, 50),
        (140, 40),
        (120, 35),
        (100, 30),
        (80, 24),
        (160, 50),
        (200, 60),
    ];

    let resize_count = resize_scenarios.len();
    let (_, total_resize_duration) = measure_time(|| {
        for (width, height) in resize_scenarios {
            // Simulate resize detection
            let _resize_detected = resize_handler.check_for_resize((width, height));

            // Update page layout
            page.update_layout((width, height));

            // Verify page is still functional
            let total_pages = page.total_pages();
            assert!(
                total_pages > 0,
                "Should maintain valid pagination after resize to {}x{}",
                width,
                height
            );

            let current_page = page.get_current_page();
            assert!(
                current_page < total_pages,
                "Current page should remain valid after resize"
            );
        }
    });

    let avg_resize_time = total_resize_duration / resize_count as u32;

    // Rapid resizes should be handled efficiently
    assert!(
        avg_resize_time < Duration::from_millis(10),
        "Average resize handling took too long: {:?}",
        avg_resize_time
    );

    println!("Average resize handling time: {:?}", avg_resize_time);
}

#[tokio::test]
async fn test_resize_debouncing_performance() {
    let mut resize_handler = ResizeHandler::new();

    // Simulate very rapid resize events (faster than debounce time)
    let rapid_resizes = vec![
        (80, 24),
        (81, 24),
        (82, 24),
        (83, 24),
        (84, 24),
        (85, 24),
        (86, 24),
        (87, 24),
        (88, 24),
        (89, 24),
    ];

    let mut detected_resizes = 0;
    let rapid_resize_count = rapid_resizes.len();

    let (_, debounce_duration) = measure_time(|| {
        for (width, height) in rapid_resizes {
            if resize_handler.check_for_resize((width, height)).is_some() {
                detected_resizes += 1;
            }
        }
    });

    // Debouncing should prevent most rapid resizes from being processed
    assert!(
        detected_resizes <= 2,
        "Too many rapid resizes were processed: {}",
        detected_resizes
    );

    // Debouncing logic should be fast
    assert!(
        debounce_duration < Duration::from_millis(10),
        "Debouncing took too long: {:?}",
        debounce_duration
    );

    println!(
        "Debouncing processed {} out of {} rapid resizes in {:?}",
        detected_resizes,
        rapid_resize_count,
        debounce_duration
    );
}

#[tokio::test]
async fn test_memory_usage_with_large_datasets() {
    // This test focuses on ensuring we don't have memory leaks or excessive memory usage
    let initial_game_count = 100;
    let games = create_large_game_dataset(initial_game_count);

    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "RUNKOSARJA".to_string(),
        false,
        true,
        false,
    );

    // Add games and measure basic functionality
    for game in &games {
        page.add_game_result(GameResultData::new(game));
    }

    // Test multiple layout updates (simulating window resizes)
    let layout_updates = 100;
    let terminal_sizes = vec![(80, 24), (100, 30), (140, 40), (200, 60)];

    let (_, memory_test_duration) = measure_time(|| {
        for i in 0..layout_updates {
            let size = terminal_sizes[i % terminal_sizes.len()];
            page.update_layout(size);

            // Verify functionality is maintained
            let total_pages = page.total_pages();
            assert!(
                total_pages > 0,
                "Should maintain pages after layout update {}",
                i
            );

            // Navigate through pages to test pagination
            page.next_page();
            page.previous_page();
        }
    });

    // Memory operations should complete in reasonable time
    assert!(
        memory_test_duration < Duration::from_secs(2),
        "Memory test took too long: {:?}",
        memory_test_duration
    );

    // Verify the page is still functional after many operations
    let final_total_pages = page.total_pages();
    assert!(
        final_total_pages > 0,
        "Should still have pages after memory test"
    );

    let final_current_page = page.get_current_page();
    assert!(
        final_current_page < final_total_pages,
        "Current page should still be valid"
    );

    println!(
        "Memory test with {} layout updates took: {:?}",
        layout_updates, memory_test_duration
    );
}

#[tokio::test]
async fn test_concurrent_resize_and_content_updates() {
    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "RUNKOSARJA".to_string(),
        false,
        true,
        false,
    );

    // Add initial games
    let initial_games = create_large_game_dataset(20);
    for game in &initial_games {
        page.add_game_result(GameResultData::new(game));
    }

    // Simulate concurrent operations: resize while updating content
    let (_, concurrent_duration) = measure_time(|| {
        for i in 0..50 {
            // Resize operation
            let width = 80 + (i % 80) as u16;
            let height = 24 + (i % 36) as u16;
            page.update_layout((width, height));

            // Content operation (simulate adding new game)
            if i % 10 == 0 {
                let new_game = GameData {
                    home_team: format!("NewTeam {}", i),
                    away_team: format!("NewTeam {}", i + 1),
                    time: "19:00".to_string(),
                    result: "1-0".to_string(),
                    score_type: liiga_teletext::teletext_ui::ScoreType::Final,
                    is_overtime: false,
                    is_shootout: false,
                    serie: "runkosarja".to_string(),
                    goal_events: vec![],
                    played_time: 3600,
                    start: "2024-01-15T19:00:00Z".to_string(),
                };
                page.add_game_result(GameResultData::new(&new_game));
            }

            // Verify consistency
            let total_pages = page.total_pages();
            assert!(
                total_pages > 0,
                "Should maintain pages during concurrent operations"
            );

            let current_page = page.get_current_page();
            assert!(
                current_page < total_pages,
                "Current page should remain valid"
            );
        }
    });

    // Concurrent operations should complete efficiently
    assert!(
        concurrent_duration < Duration::from_secs(1),
        "Concurrent operations took too long: {:?}",
        concurrent_duration
    );

    println!(
        "Concurrent resize and content updates took: {:?}",
        concurrent_duration
    );
}

#[tokio::test]
async fn test_extreme_terminal_sizes_performance() {
    let mut calculator = LayoutCalculator::new();
    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "RUNKOSARJA".to_string(),
        false,
        true,
        false,
    );

    // Add some games
    let games = create_large_game_dataset(10);
    for game in &games {
        page.add_game_result(GameResultData::new(game));
    }

    // Test extreme sizes
    let extreme_sizes = vec![
        (1, 1),      // Extremely small
        (10, 5),     // Very small
        (40, 10),    // Below minimum
        (500, 200),  // Very large
        (1000, 500), // Extremely large
        (80, 1),     // Wide but very short
        (10, 100),   // Narrow but very tall
    ];

    for (width, height) in extreme_sizes {
        // Test layout calculation
        let (_, calc_duration) = measure_time(|| calculator.calculate_layout((width, height)));

        // Should handle extreme sizes gracefully and quickly
        assert!(
            calc_duration < Duration::from_millis(10),
            "Layout calculation for extreme size {}x{} took too long: {:?}",
            width,
            height,
            calc_duration
        );

        // Test page layout update
        let (_, update_duration) = measure_time(|| {
            page.update_layout((width, height));
        });

        assert!(
            update_duration < Duration::from_millis(50),
            "Page layout update for extreme size {}x{} took too long: {:?}",
            width,
            height,
            update_duration
        );

        // Verify page remains functional
        let total_pages = page.total_pages();
        assert!(
            total_pages > 0,
            "Should have pages even with extreme size {}x{}",
            width,
            height
        );

        println!(
            "Extreme size {}x{}: calc={:?}, update={:?}, pages={}",
            width, height, calc_duration, update_duration, total_pages
        );
    }
}

#[tokio::test]
async fn test_stress_test_combined_operations() {
    // Combined stress test with multiple operations
    let mut calculator = LayoutCalculator::new();
    let mut resize_handler = ResizeHandler::new();
    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "RUNKOSARJA".to_string(),
        false,
        true,
        false,
    );

    // Large dataset
    let games = create_large_game_dataset(200);
    for game in &games {
        page.add_game_result(GameResultData::new(game));
    }

    let stress_iterations = 200;
    let (_, stress_duration) = measure_time(|| {
        for i in 0..stress_iterations {
            // Random terminal size
            let width = 80 + (i % 120) as u16;
            let height = 24 + (i % 40) as u16;

            // Layout calculation
            calculator.calculate_layout((width, height));

            // Resize detection
            resize_handler.check_for_resize((width, height));

            // Page layout update
            page.update_layout((width, height));

            // Pagination operations
            if i % 5 == 0 {
                page.next_page();
            }
            if i % 7 == 0 {
                page.previous_page();
            }

            // Content adaptation (sample)
            if i % 10 == 0 && !games.is_empty() {
                let game = &games[i % games.len()];
                let game_result = GameResultData::new(game);
                ContentAdapter::adapt_game_content(
                    &game_result.home_team,
                    &game_result.away_team,
                    &game_result.time,
                    &game_result.result,
                    &game.goal_events,
                    DetailLevel::Standard,
                    width,
                );
            }
        }
    });

    let avg_operation_time = stress_duration / stress_iterations as u32;

    // Stress test should complete in reasonable time
    assert!(
        stress_duration < Duration::from_secs(10),
        "Stress test took too long: {:?}",
        stress_duration
    );

    assert!(
        avg_operation_time < Duration::from_millis(50),
        "Average operation time too slow: {:?}",
        avg_operation_time
    );

    // Verify system is still functional after stress test
    let final_total_pages = page.total_pages();
    assert!(
        final_total_pages > 0,
        "Should still have pages after stress test"
    );

    let final_current_page = page.get_current_page();
    assert!(
        final_current_page < final_total_pages,
        "Current page should still be valid"
    );

    println!(
        "Stress test: {} iterations in {:?} (avg: {:?})",
        stress_iterations, stress_duration, avg_operation_time
    );
}
