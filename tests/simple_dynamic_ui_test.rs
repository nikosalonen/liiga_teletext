//! Simple integration test for dynamic UI functionality

use liiga_teletext::{
    data_fetcher::models::*,
    teletext_ui::{GameResultData, TeletextPage},
    ui::layout::{DetailLevel, LayoutCalculator},
};

#[tokio::test]
async fn test_basic_layout_calculator() {
    let mut calculator = LayoutCalculator::new();

    // Test with minimum size
    let config = calculator.calculate_layout((80, 24));
    assert_eq!(config.detail_level, DetailLevel::Minimal);
    assert!(config.content_width > 0);

    // Test with large size
    let config = calculator.calculate_layout((140, 40));
    assert_eq!(config.detail_level, DetailLevel::Extended);
    assert!(config.content_width > 0);
}

#[tokio::test]
async fn test_basic_page_creation() {
    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "RUNKOSARJA".to_string(),
        false,
        true,
        false,
    );

    // Test layout update
    page.update_layout((100, 30));

    // Test pagination
    let total_pages = page.total_pages();
    assert!(total_pages > 0);

    let current_page = page.get_current_page();
    assert!(current_page < total_pages);
}

#[tokio::test]
async fn test_page_with_games() {
    let mut page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "RUNKOSARJA".to_string(),
        false,
        true,
        false,
    );

    // Create a simple game
    let game = GameData {
        home_team: "HIFK".to_string(),
        away_team: "Tappara".to_string(),
        time: "18:30".to_string(),
        result: "3-2".to_string(),
        score_type: liiga_teletext::teletext_ui::ScoreType::Final,
        is_overtime: false,
        is_shootout: false,
        serie: "runkosarja".to_string(),
        goal_events: vec![],
        played_time: 3600,
        start: "2024-01-15T18:30:00Z".to_string(),
    };

    page.add_game_result(GameResultData::new(&game));

    // Test different terminal sizes
    let sizes = vec![(80, 24), (100, 30), (140, 40)];

    for (width, height) in sizes {
        page.update_layout((width, height));

        let total_pages = page.total_pages();
        assert!(
            total_pages > 0,
            "Should have pages for size {}x{}",
            width,
            height
        );

        let current_page = page.get_current_page();
        assert!(
            current_page < total_pages,
            "Current page should be valid for size {}x{}",
            width,
            height
        );
    }
}
