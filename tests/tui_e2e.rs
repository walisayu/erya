#[path = "e2e/helpers/auth_state.rs"]
mod auth_state;
#[path = "e2e/pages/editor_screen.rs"]
mod editor_screen;
#[path = "e2e/test_data/known_codes.rs"]
mod known_codes;
#[path = "e2e/fixtures/pty_fixture.rs"]
mod pty_fixture;
#[path = "e2e/helpers/pty_session.rs"]
mod pty_session;
#[path = "e2e/helpers/screen_contract.rs"]
mod screen_contract;
#[path = "e2e/helpers/screen_guard.rs"]
mod screen_guard;
#[path = "e2e/test_data/screen_manifest.rs"]
mod screen_manifest;
#[path = "e2e/test_data/state_transitions.rs"]
mod state_transitions;

#[path = "e2e/specs/state/document_state.rs"]
mod document_state;
#[path = "e2e/specs/journeys/edit_save_journey.rs"]
mod edit_save_journey;
#[path = "e2e/specs/contracts/editor_contract.rs"]
mod editor_contract;
#[path = "e2e/specs/generated/screen_health_generated.rs"]
mod screen_health_generated;
