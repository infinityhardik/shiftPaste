//! Tray / menu-bar icon with: Open · Open Editor · Pause recording · Settings ·
//! Clear history… · Quit. Left-click opens the popup (where the OS allows).
//!
//! Windows/macOS: the icon lives on the main (UI) thread. Linux: AppIndicator
//! needs a GTK loop, so the icon runs on its own thread.

use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIconBuilder, TrayIconEvent};

use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    Open,
    Editor,
    TogglePause,
    Settings,
    ClearHistory,
    Quit,
}

/// Full-colour logo for Windows/Linux; a black+alpha "template" on macOS, which
/// the menu bar tints for light and dark mode.
#[cfg(not(target_os = "macos"))]
const ICON_PNG: &[u8] = include_bytes!("../assets/tray-64.png");
#[cfg(target_os = "macos")]
const ICON_PNG: &[u8] = include_bytes!("../assets/tray-template-36.png");

fn icon() -> Result<Icon> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(ICON_PNG));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::ALPHA);
    let mut reader = decoder.read_info().map_err(Error::new)?;
    let mut buf = vec![0; reader.output_buffer_size().unwrap_or(64 * 64 * 4)];
    let info = reader.next_frame(&mut buf).map_err(Error::new)?;
    buf.truncate(info.buffer_size());
    Icon::from_rgba(buf, info.width, info.height).map_err(Error::new)
}

fn build_menu(paused: bool) -> Result<(Menu, CheckMenuItem)> {
    let menu = Menu::new();
    let pause = CheckMenuItem::with_id("pause", "Pause recording", true, paused, None);
    menu.append_items(&[
        &MenuItem::with_id("open", "Open Shift Paste", true, None),
        &MenuItem::with_id("editor", "Open Editor", true, None),
        &PredefinedMenuItem::separator(),
        &pause,
        &MenuItem::with_id("settings", "Settings…", true, None),
        &MenuItem::with_id("clear", "Clear history…", true, None),
        &PredefinedMenuItem::separator(),
        &MenuItem::with_id("quit", "Quit Shift Paste", true, None),
    ])
    .map_err(Error::new)?;
    Ok((menu, pause))
}

fn command_for(id: &str) -> Option<TrayCommand> {
    Some(match id {
        "open" => TrayCommand::Open,
        "editor" => TrayCommand::Editor,
        "pause" => TrayCommand::TogglePause,
        "settings" => TrayCommand::Settings,
        "clear" => TrayCommand::ClearHistory,
        "quit" => TrayCommand::Quit,
        _ => return None,
    })
}

fn install_handlers(on_command: impl Fn(TrayCommand) + Send + Sync + Clone + 'static) {
    let menu_cb = on_command.clone();
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
        if let Some(cmd) = command_for(e.id.0.as_str()) {
            menu_cb(cmd);
        }
    }));
    TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
        if let TrayIconEvent::Click {
            button: tray_icon::MouseButton::Left,
            button_state: tray_icon::MouseButtonState::Up,
            ..
        } = e
        {
            on_command(TrayCommand::Open);
        }
    }));
}

/// The running tray icon. Keep it alive for as long as the app runs.
pub struct Tray {
    #[cfg(not(target_os = "linux"))]
    _icon: tray_icon::TrayIcon,
    #[cfg(not(target_os = "linux"))]
    pause: CheckMenuItem,
    #[cfg(target_os = "linux")]
    pause_tx: std::sync::mpsc::Sender<bool>,
}

impl Tray {
    /// Create the tray icon. `on_command` may be called from any thread.
    pub fn create(
        paused: bool,
        on_command: impl Fn(TrayCommand) + Send + Sync + Clone + 'static,
    ) -> Result<Tray> {
        install_handlers(on_command);
        #[cfg(not(target_os = "linux"))]
        {
            let (menu, pause) = build_menu(paused)?;
            let builder = TrayIconBuilder::new()
                .with_menu(Box::new(menu))
                .with_menu_on_left_click(false)
                .with_tooltip("Shift Paste");
            #[cfg(target_os = "macos")]
            let builder = builder.with_icon_templated(icon()?);
            #[cfg(not(target_os = "macos"))]
            let builder = builder.with_icon(icon()?);
            let icon = builder.build().map_err(Error::new)?;
            Ok(Tray { _icon: icon, pause })
        }
        #[cfg(target_os = "linux")]
        {
            let (pause_tx, pause_rx) = std::sync::mpsc::channel::<bool>();
            let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<()>>();
            std::thread::Builder::new()
                .name("tray-gtk".into())
                .spawn(move || {
                    if let Err(e) = gtk::init() {
                        let _ = ready_tx.send(Err(Error::new(format!("GTK unavailable: {e}"))));
                        return;
                    }
                    let built = build_menu(paused).and_then(|(menu, pause)| {
                        let icon = TrayIconBuilder::new()
                            .with_menu(Box::new(menu))
                            .with_tooltip("Shift Paste")
                            .with_icon(icon()?)
                            .build()
                            .map_err(Error::new)?;
                        Ok((icon, pause))
                    });
                    match built {
                        Ok((icon, pause)) => {
                            let _ = ready_tx.send(Ok(()));
                            gtk::glib::timeout_add_local(
                                std::time::Duration::from_millis(250),
                                move || {
                                    while let Ok(p) = pause_rx.try_recv() {
                                        pause.set_checked(p);
                                    }
                                    let _keep_alive = &icon;
                                    gtk::glib::ControlFlow::Continue
                                },
                            );
                            gtk::main();
                        }
                        Err(e) => {
                            let _ = ready_tx.send(Err(e));
                        }
                    }
                })
                .map_err(Error::new)?;
            ready_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .map_err(|_| Error::new("The tray icon didn't start in time."))??;
            Ok(Tray { pause_tx })
        }
    }

    /// Keep the menu's checkmark in sync when pausing from elsewhere.
    pub fn set_paused(&self, paused: bool) {
        #[cfg(not(target_os = "linux"))]
        self.pause.set_checked(paused);
        #[cfg(target_os = "linux")]
        let _ = self.pause_tx.send(paused);
    }
}
