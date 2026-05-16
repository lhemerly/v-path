use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let profile_path = std::env::args_os().nth(1).map(PathBuf::from);
    v_path::tui::run_with_live_profile_path(profile_path)
}
