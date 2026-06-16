use super::{App, AppState, MainMenuItem, SettingsItem, TuiDisplayMode};
use crate::display::DisplaySettings;

#[derive(Debug, Clone, Copy)]
pub struct AppSnapshot<'a> {
    pub state: AppState,
    pub status_text: &'a str,
    pub instructions_text: &'a str,
    pub fps: f64,
    pub tui_display_mode: TuiDisplayMode,
    pub selected_main_menu_item: MainMenuItem,
    pub selected_problem_item: usize,
    pub selected_settings_item: SettingsItem,
    pub display_settings: DisplaySettings,
    pub source_scroll: usize,
    pub how_to_use_scroll: usize,
    pub should_quit: bool,
    pub should_open_file_dialog: bool,
}

impl App {
    pub fn snapshot(&self) -> AppSnapshot<'_> {
        AppSnapshot {
            state: self.state,
            status_text: self.status_text.as_str(),
            instructions_text: self.instructions_text.as_str(),
            fps: self.displayed_fps,
            tui_display_mode: self.tui_display_mode,
            selected_main_menu_item: self.selected_main_menu_item,
            selected_problem_item: self.selected_problem_item,
            selected_settings_item: self.selected_settings_item,
            display_settings: self.display_settings,
            source_scroll: self.source_scroll,
            how_to_use_scroll: self.how_to_use_scroll,
            should_quit: self.should_quit,
            should_open_file_dialog: self.should_open_file_dialog,
        }
    }
}
