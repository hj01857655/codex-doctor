use std::path::PathBuf;

use gui::{default_codex_home_from_env, CodexDoctorApp};

fn main() -> eframe::Result<()> {
    let codex_home = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            default_codex_home_from_env(
                std::env::var_os("CODEX_HOME").map(PathBuf::from),
                std::env::var_os("USERPROFILE").map(PathBuf::from),
                std::env::var_os("HOME").map(PathBuf::from),
                std::env::current_dir().expect("current dir"),
            )
        });

    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Codex Doctor",
        options,
        Box::new(move |_cc| {
            Ok(Box::new(CodexDoctorApp::new(
                codex_home.display().to_string(),
            )))
        }),
    )
}
