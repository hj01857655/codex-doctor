use std::path::PathBuf;

use gui::{load_dashboard_view_model, render_dashboard_text, DashboardViewModel};

fn main() {
    let codex_home = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().expect("current dir"));

    match load_dashboard_view_model(&codex_home) {
        Ok(view_model) => println!("{}", render_dashboard_text(&view_model)),
        Err(error) => println!("{}", render_error_screen(&codex_home, &error)),
    }
}

fn render_error_screen(codex_home: &PathBuf, error: &str) -> String {
    let fallback = DashboardViewModel {
        codex_home: codex_home.display().to_string(),
        summary_items: vec![],
        problems: vec![],
        preview_actions: vec![],
    };

    format!("{}\nError: {error}", render_dashboard_text(&fallback))
}
