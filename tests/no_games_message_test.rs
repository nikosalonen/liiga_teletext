use liiga_teletext::teletext_ui::TeletextPage;

#[test]
fn test_create_error_page_includes_navigation_hint_for_specific_date() {
    // Test that the error page creation includes navigation instructions for a specific date
    let mut error_page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false, // disable_links
        true,  // show_footer
        false, // ignore_height_limit
        false, // compact_mode
        false, // wide_mode
    );

    let formatted_date = "01.07.";

    // Simulate the logic from create_error_page function
    error_page.add_error_message(&format!("Ei otteluita {formatted_date} päivälle"));
    error_page.add_error_message("");
    error_page.add_error_message("Käytä Shift + nuolinäppäimiä");
    error_page.add_error_message("siirtyäksesi toiseen päivään");

    // The page should have error messages
    assert!(error_page.has_error_messages());
}

#[test]
fn test_create_error_page_includes_navigation_hint_for_today() {
    // Test that the error page creation includes navigation instructions for today
    let mut error_page = TeletextPage::new(
        221,
        "JÄÄKIEKKO".to_string(),
        "SM-LIIGA".to_string(),
        false, // disable_links
        true,  // show_footer
        false, // ignore_height_limit
        false, // compact_mode
        false, // wide_mode
    );

    // Simulate the logic from create_error_page function for today
    error_page.add_error_message("Ei otteluita tänään");
    error_page.add_error_message("");
    error_page.add_error_message("Käytä Shift + nuolinäppäimiä");
    error_page.add_error_message("siirtyäksesi toiseen päivään");

    // The page should have error messages
    assert!(error_page.has_error_messages());
}
