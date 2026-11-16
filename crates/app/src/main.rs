// No console window for release builds on Windows.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::path::Path;

use shiftpaste_app::app::{post, App};
use shiftpaste_app::{editor_ctl, prefs_ctl};
use shiftpaste_core::library::Library;
use shiftpaste_core::paths::AppPaths;
use shiftpaste_core::settings::Settings;
use shiftpaste_platform as platform;
use shiftpaste_platform::instance::Command;
use slint::ComponentHandle;

const USAGE: &str = "Shift Paste: clipboard history and snippet collections

Usage: shiftpaste [--toggle | --editor | --settings | --quit]

  (no option)  Start, or open the editor if already running
  --toggle     Open or close the popup (bind this to a key on Wayland)
  --editor     Open the editor
  --settings   Open Settings
  --quit       Quit the running instance";

fn main() {
    platform::prepare_process();
    let arg = std::env::args().nth(1);
    let command = match arg.as_deref() {
        None => None,
        Some("--toggle") => Some(Command::Toggle),
        Some("--editor") => Some(Command::Editor),
        Some("--settings") => Some(Command::Settings),
        Some("--quit") => Some(Command::Quit),
        Some("--help" | "-h") => {
            println!("{USAGE}");
            return;
        }
        Some(other) => {
            eprintln!("Unknown option {other}\n\n{USAGE}");
            std::process::exit(2);
        }
    };

    // Already running? Hand the request over and exit.
    if platform::instance::send_to_running(command.unwrap_or(Command::Editor)) {
        return;
    }
    if command == Some(Command::Quit) {
        return;
    }

    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf));
    let paths = match AppPaths::resolve(exe_dir.as_deref()) {
        Ok(p) => p,
        Err(e) => return fatal(&e.to_string()),
    };
    init_logging(&paths.log_dir);
    tracing::info!(
        "Shift Paste {} starting; data in {}",
        env!("CARGO_PKG_VERSION"),
        paths.data_dir.display()
    );

    let loaded = Settings::load(&paths.settings_file());
    let mut notices: Vec<String> = loaded.warning.into_iter().collect();
    notices.extend(paths.portable_problem.clone());
    let lib = match Library::open(&paths.database_file()) {
        Ok((lib, recovered)) => {
            if let Some(backup) = recovered {
                notices.push(format!(
                    "Your data file was damaged, so a fresh one was started. The old file was kept as “{}”.",
                    backup.display()
                ));
            }
            lib
        }
        Err(e) => return fatal(&format!("Couldn't open your data: {e}")),
    };
    let first_run = !loaded.settings.general.welcome_done;

    if let Err(e) = select_backend() {
        return fatal(&format!("Couldn't start the user interface: {e}"));
    }
    let notice = (!notices.is_empty()).then(|| notices.join("\n"));
    let app = App::start(paths, lib, loaded.settings, notice);

    if let Err(e) = platform::instance::listen(|cmd| post(move |app| run_command(app, cmd))) {
        tracing::warn!("single-instance listener unavailable: {e}");
    }
    match command {
        Some(cmd) => run_command(&app, cmd),
        None if first_run || app.notice.borrow().is_some() => editor_ctl::open_editor(&app, None),
        None => {}
    }
    if let Err(e) = slint::run_event_loop_until_quit() {
        tracing::error!("event loop ended with an error: {e}");
    }
    tracing::info!("Shift Paste stopped");
}

fn run_command(app: &std::rc::Rc<App>, cmd: Command) {
    match cmd {
        Command::Toggle if app.popup.window().is_visible() => app.hide_popup(),
        Command::Toggle | Command::Show => {
            let front = platform::foreground::current_app().map(shiftpaste_app::app::to_identity);
            app.open_popup_for(front);
        }
        Command::Editor => editor_ctl::open_editor(app, None),
        Command::Settings => prefs_ctl::open_settings(app, None),
        Command::Quit => app.quit(),
    }
}

/// winit backend; the popup stays out of the Windows taskbar and Alt+Tab.
fn select_backend() -> Result<(), slint::PlatformError> {
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .with_winit_window_attributes_hook(shiftpaste_app::adjust_window_attributes)
        .select()
}

fn fatal(message: &str) {
    tracing::error!("{message}");
    eprintln!("{message}");
    platform::shell::error_dialog("Shift Paste can't start", message);
}

/// Log to stderr and to `<data>/logs/shiftpaste.log` (trimmed when large).
fn init_logging(dir: &Path) {
    use tracing_subscriber::fmt::writer::MakeWriterExt;
    let _ = std::fs::create_dir_all(dir);
    let file_path = dir.join("shiftpaste.log");
    if std::fs::metadata(&file_path).is_ok_and(|m| m.len() > 2_000_000) {
        let _ = std::fs::rename(&file_path, dir.join("shiftpaste.old.log"));
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file_path);
    let filter = tracing_subscriber::EnvFilter::try_from_env("SHIFTPASTE_LOG")
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let builder = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(false);
    match file {
        Ok(f) => builder
            .with_writer(std::io::stderr.and(std::sync::Mutex::new(f)))
            .init(),
        Err(_) => builder.with_writer(std::io::stderr).init(),
    }
}
