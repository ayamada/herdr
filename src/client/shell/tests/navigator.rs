use super::*;

/// Three tabs in the focused workspace, so `2` and `3` have somewhere to go.
fn three_tabs() -> ClientShellSnapshot {
    let mut projected = snapshot();
    let template = projected.tabs[0].clone();
    projected.tabs = (1..=3)
        .map(|number| {
            let mut tab = template.clone();
            tab.tab_id = format!("tab_{number}");
            tab.number = number;
            tab.label = number.to_string();
            tab.focused = number == 1;
            tab
        })
        .collect();
    projected
}

fn navigator_state(config: &Config) -> ClientShellState {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(config));
    state.set_snapshot(Box::new(three_tabs()));
    state.set_pane_surface(surface());
    state.compose(110, 32).expect("compose");
    state.open_navigator_overlay();
    state
}

#[test]
fn navigator_digits_switch_to_that_tab_and_close() {
    let mut state = navigator_state(&Config::default());
    let outcome = state.handle_input_bytes(b"2");
    assert!(state.overlay.is_none());
    assert!(
        matches!(
            outcome.actions.as_slice(),
            [ClientShellAction::Endpoint { request, .. }]
                if matches!(
                    &request.method,
                    crate::api::schema::Method::TabFocus(target) if target.tab_id == "tab_2"
                )
        ),
        "{:?}",
        outcome.actions
    );
}

#[test]
fn navigator_digit_without_that_tab_is_ignored() {
    let mut state = navigator_state(&Config::default());
    let outcome = state.handle_input_bytes(b"9");
    assert!(matches!(
        state.overlay,
        Some(ClientShellOverlay::Navigator(_))
    ));
    assert!(outcome.actions.is_empty(), "{:?}", outcome.actions);
}

#[test]
fn navigator_digit_while_search_focused_is_search_text() {
    let mut state = navigator_state(&Config::default());
    let _ = state.handle_input_bytes(b"/");
    let outcome = state.handle_input_bytes(b"2");
    let Some(ClientShellOverlay::Navigator(navigator)) = state.overlay.as_ref() else {
        panic!("navigator closed");
    };
    assert!(navigator.search_focused);
    assert_eq!(navigator.query.as_str(), "2");
    assert!(outcome.actions.is_empty(), "{:?}", outcome.actions);
}

#[test]
fn navigator_digits_follow_the_configured_switch_tab_bindings() {
    let mut config = Config::default();
    config.keys.switch_tab = crate::config::BindingConfig::empty();
    let mut state = navigator_state(&config);
    let outcome = state.handle_input_bytes(b"2");
    assert!(matches!(
        state.overlay,
        Some(ClientShellOverlay::Navigator(_))
    ));
    assert!(outcome.actions.is_empty(), "{:?}", outcome.actions);
}
