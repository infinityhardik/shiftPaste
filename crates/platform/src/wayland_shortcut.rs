//! Wayland global shortcut through the XDG desktop portal
//! (`org.freedesktop.portal.GlobalShortcuts`: KDE Plasma, GNOME 48+, Hyprland).
//! The desktop shows its own confirmation and lets the user change the keys.

use std::collections::HashMap;

/// Ask the desktop for a global shortcut; `on_activate` runs (on a
/// background thread) each time it's pressed. `shortcut` is like "Ctrl+Shift+V".
pub fn bind(shortcut: &str, on_activate: impl Fn() + Send + 'static) {
    let trigger = portal_trigger(shortcut);
    let _ = std::thread::Builder::new().name("wayland-shortcut".into()).spawn(move || {
        if let Err(e) = run(&trigger, on_activate) {
            tracing::warn!("Wayland global shortcut unavailable ({e}); bind `shiftpaste --toggle` in your desktop's keyboard settings");
        }
    });
}

/// "Ctrl+Shift+V" → "CTRL+SHIFT+v" (XDG shortcut format).
fn portal_trigger(shortcut: &str) -> String {
    shortcut
        .split('+')
        .map(str::trim)
        .map(|p| match p.to_lowercase().as_str() {
            "ctrl" | "control" => "CTRL".to_string(),
            "shift" => "SHIFT".to_string(),
            "alt" => "ALT".to_string(),
            "cmd" | "win" | "super" | "meta" => "LOGO".to_string(),
            key => key.to_string(),
        })
        .collect::<Vec<_>>()
        .join("+")
}

fn run(trigger: &str, on_activate: impl Fn()) -> zbus::Result<()> {
    use zbus::blocking::{Connection, Proxy};
    use zbus::zvariant::{ObjectPath, OwnedValue, Value};

    const DEST: &str = "org.freedesktop.portal.Desktop";
    const PATH: &str = "/org/freedesktop/portal/desktop";
    let conn = Connection::session()?;
    let portal = Proxy::new(&conn, DEST, PATH, "org.freedesktop.portal.GlobalShortcuts")?;
    let sender = conn
        .unique_name()
        .map(|n| n.trim_start_matches(':').replace('.', "_"))
        .unwrap_or_default();

    // Portal calls answer through a Request object's Response signal;
    // subscribe before calling so the answer can't be missed.
    let call = |method: &str,
                token: &str,
                body: &dyn Fn(Value<'static>) -> zbus::Result<()>|
     -> zbus::Result<HashMap<String, OwnedValue>> {
        let request_path = format!("{PATH}/request/{sender}/{token}");
        let request = Proxy::new(
            &conn,
            DEST,
            request_path.as_str(),
            "org.freedesktop.portal.Request",
        )?;
        let mut responses = request.receive_signal("Response")?;
        body(Value::from(token.to_string()))?;
        let msg = responses
            .next()
            .ok_or_else(|| zbus::Error::Failure(format!("{method}: no response")))?;
        let (code, results): (u32, HashMap<String, OwnedValue>) = msg.body().deserialize()?;
        if code != 0 {
            return Err(zbus::Error::Failure(format!(
                "{method} was declined (code {code})"
            )));
        }
        Ok(results)
    };

    let session = call("CreateSession", "shiftpaste_session_req", &|token| {
        let mut opts: HashMap<&str, Value> = HashMap::new();
        opts.insert("handle_token", token);
        opts.insert("session_handle_token", Value::from("shiftpaste_session"));
        portal.call_method("CreateSession", &(opts,)).map(|_| ())
    })?;
    let handle: String = session
        .get("session_handle")
        .and_then(|v| v.try_clone().ok())
        .and_then(|v| String::try_from(v).ok())
        .ok_or_else(|| zbus::Error::Failure("no session handle".into()))?;
    let session_path = ObjectPath::try_from(handle.as_str())?;

    call("BindShortcuts", "shiftpaste_bind_req", &|token| {
        let mut props: HashMap<&str, Value> = HashMap::new();
        props.insert("description", Value::from("Open Shift Paste"));
        props.insert("preferred_trigger", Value::from(trigger));
        let shortcuts = vec![("open-popup", props)];
        let mut opts: HashMap<&str, Value> = HashMap::new();
        opts.insert("handle_token", token);
        portal
            .call_method("BindShortcuts", &(&session_path, shortcuts, "", opts))
            .map(|_| ())
    })?;
    tracing::info!("Wayland global shortcut bound through the desktop portal");

    for msg in portal.receive_signal("Activated")? {
        let (_session, id, _time, _opts): (ObjectPath, String, u64, HashMap<String, OwnedValue>) =
            msg.body().deserialize()?;
        if id == "open-popup" {
            on_activate();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn trigger_format() {
        assert_eq!(super::portal_trigger("Ctrl+Shift+V"), "CTRL+SHIFT+v");
        assert_eq!(super::portal_trigger("Super + Alt + F2"), "LOGO+ALT+f2");
    }
}
