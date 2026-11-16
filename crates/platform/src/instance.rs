//! Single instance: a second launch (or `shiftpaste --toggle`) talks to the
//! running app over a local socket and exits.

use std::io::{BufRead, BufReader, Write};

use interprocess::local_socket::{prelude::*, GenericNamespaced, ListenerOptions, Stream};

use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Toggle,
    Show,
    Editor,
    Settings,
    Quit,
}

impl Command {
    fn as_str(self) -> &'static str {
        match self {
            Command::Toggle => "toggle",
            Command::Show => "show",
            Command::Editor => "editor",
            Command::Settings => "settings",
            Command::Quit => "quit",
        }
    }

    fn parse(s: &str) -> Option<Command> {
        Some(match s.trim() {
            "toggle" => Command::Toggle,
            "show" => Command::Show,
            "editor" => Command::Editor,
            "settings" => Command::Settings,
            "quit" => Command::Quit,
            _ => return None,
        })
    }
}

fn socket_name() -> String {
    let user = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_default();
    format!("shiftpaste-{user}.sock")
}

/// Send `cmd` to a running instance. Returns true if one answered.
pub fn send_to_running(cmd: Command) -> bool {
    let Ok(name) = socket_name().to_ns_name::<GenericNamespaced>() else {
        return false;
    };
    match Stream::connect(name) {
        Ok(mut conn) => conn
            .write_all(format!("{}\n", cmd.as_str()).as_bytes())
            .is_ok(),
        Err(_) => false,
    }
}

/// Become the running instance: listen for commands on a background thread.
pub fn listen(on_command: impl Fn(Command) + Send + 'static) -> Result<()> {
    let name = socket_name()
        .to_ns_name::<GenericNamespaced>()
        .map_err(Error::new)?;
    let listener = ListenerOptions::new()
        .name(name)
        .try_overwrite(true)
        .create_sync()
        .map_err(Error::new)?;
    std::thread::Builder::new()
        .name("instance-ipc".into())
        .spawn(move || {
            for conn in listener.incoming().filter_map(|c| c.ok()) {
                let mut line = String::new();
                if BufReader::new(conn).read_line(&mut line).is_ok() {
                    if let Some(cmd) = Command::parse(&line) {
                        on_command(cmd);
                    }
                }
            }
        })
        .map_err(Error::new)?;
    Ok(())
}
