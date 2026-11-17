//! Built-in "Popular apps" for the app picker, so nobody has to type app names.
//!
//! Each preset lists every identifier the app has across operating systems
//! (Windows exe name · macOS bundle id · Linux WM_CLASS / process name), all
//! lower-case. Settings store presets as `preset:<id>`.
//! Identifiers must be verified on real machines before release.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Platform {
    Windows,
    MacOs,
    Linux,
}

impl Platform {
    pub const fn current() -> Platform {
        if cfg!(target_os = "windows") {
            Platform::Windows
        } else if cfg!(target_os = "macos") {
            Platform::MacOs
        } else {
            Platform::Linux
        }
    }

    const fn bit(self) -> u8 {
        match self {
            Platform::Windows => WIN,
            Platform::MacOs => MAC,
            Platform::Linux => LINUX,
        }
    }
}

const WIN: u8 = 1;
const MAC: u8 = 2;
const LINUX: u8 = 4;
const ALL: u8 = WIN | MAC | LINUX;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Group {
    Office,
    Terminals,
    CodeEditors,
    Browsers,
    Chat,
    Design,
    PasswordManagers,
}

impl Group {
    pub const ALL: [Group; 7] = [
        Group::Office,
        Group::Terminals,
        Group::CodeEditors,
        Group::Browsers,
        Group::Chat,
        Group::Design,
        Group::PasswordManagers,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Group::Office => "Office",
            Group::Terminals => "Terminals",
            Group::CodeEditors => "Code editors",
            Group::Browsers => "Browsers",
            Group::Chat => "Chat",
            Group::Design => "Design",
            Group::PasswordManagers => "Password managers",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    pub group: Group,
    platforms: u8,
    pub identifiers: &'static [&'static str],
}

impl Preset {
    pub fn available_on(&self, p: Platform) -> bool {
        self.platforms & p.bit() != 0
    }

    /// The settings entry for this preset: `preset:<id>`.
    pub fn entry(&self) -> String {
        format!("{PRESET_PREFIX}{}", self.id)
    }
}

pub const PRESET_PREFIX: &str = "preset:";

macro_rules! p {
    ($id:literal, $name:literal, $group:ident, $plat:expr, [$($ident:literal),+ $(,)?]) => {
        Preset { id: $id, name: $name, group: Group::$group, platforms: $plat,
                 identifiers: &[$($ident),+] }
    };
}

pub const PRESETS: &[Preset] = &[
    // Office
    p!(
        "word",
        "Microsoft Word",
        Office,
        WIN | MAC,
        ["winword.exe", "com.microsoft.word"]
    ),
    p!(
        "excel",
        "Microsoft Excel",
        Office,
        WIN | MAC,
        ["excel.exe", "com.microsoft.excel"]
    ),
    p!(
        "powerpoint",
        "Microsoft PowerPoint",
        Office,
        WIN | MAC,
        ["powerpnt.exe", "com.microsoft.powerpoint"]
    ),
    p!(
        "outlook",
        "Microsoft Outlook",
        Office,
        WIN | MAC,
        ["outlook.exe", "olk.exe", "com.microsoft.outlook"]
    ),
    p!(
        "onenote",
        "Microsoft OneNote",
        Office,
        WIN | MAC,
        ["onenote.exe", "com.microsoft.onenote.mac"]
    ),
    p!(
        "libreoffice",
        "LibreOffice",
        Office,
        ALL,
        [
            "soffice.exe",
            "soffice.bin",
            "soffice",
            "libreoffice",
            "org.libreoffice.script"
        ]
    ),
    p!("pages", "Pages", Office, MAC, ["com.apple.iwork.pages"]),
    p!(
        "numbers",
        "Numbers",
        Office,
        MAC,
        ["com.apple.iwork.numbers"]
    ),
    p!(
        "keynote",
        "Keynote",
        Office,
        MAC,
        ["com.apple.iwork.keynote"]
    ),
    // Terminals
    p!(
        "windows-terminal",
        "Windows Terminal",
        Terminals,
        WIN,
        ["windowsterminal.exe"]
    ),
    p!(
        "powershell",
        "PowerShell",
        Terminals,
        WIN,
        ["powershell.exe", "pwsh.exe"]
    ),
    p!(
        "cmd",
        "Command Prompt",
        Terminals,
        WIN,
        ["cmd.exe", "conhost.exe"]
    ),
    p!(
        "macos-terminal",
        "Terminal",
        Terminals,
        MAC,
        ["com.apple.terminal"]
    ),
    p!(
        "iterm2",
        "iTerm2",
        Terminals,
        MAC,
        ["com.googlecode.iterm2"]
    ),
    p!(
        "warp",
        "Warp",
        Terminals,
        MAC | LINUX,
        ["dev.warp.warp-stable", "dev.warp.warp", "warp-terminal"]
    ),
    p!(
        "gnome-terminal",
        "GNOME Terminal",
        Terminals,
        LINUX,
        [
            "gnome-terminal-server",
            "gnome-terminal",
            "org.gnome.terminal"
        ]
    ),
    p!(
        "gnome-console",
        "GNOME Console",
        Terminals,
        LINUX,
        ["kgx", "org.gnome.console"]
    ),
    p!(
        "konsole",
        "Konsole",
        Terminals,
        LINUX,
        ["konsole", "org.kde.konsole"]
    ),
    p!(
        "xfce4-terminal",
        "Xfce Terminal",
        Terminals,
        LINUX,
        ["xfce4-terminal"]
    ),
    p!(
        "tilix",
        "Tilix",
        Terminals,
        LINUX,
        ["tilix", "com.gexperts.tilix"]
    ),
    p!("terminator", "Terminator", Terminals, LINUX, ["terminator"]),
    p!(
        "alacritty",
        "Alacritty",
        Terminals,
        ALL,
        ["alacritty.exe", "alacritty", "org.alacritty"]
    ),
    p!(
        "kitty",
        "kitty",
        Terminals,
        MAC | LINUX,
        ["kitty", "net.kovidgoyal.kitty"]
    ),
    p!(
        "wezterm",
        "WezTerm",
        Terminals,
        ALL,
        [
            "wezterm-gui.exe",
            "wezterm-gui",
            "org.wezfurlong.wezterm",
            "com.github.wez.wezterm"
        ]
    ),
    p!("xterm", "xterm", Terminals, LINUX, ["xterm", "uxterm"]),
    p!("foot", "foot", Terminals, LINUX, ["foot", "footclient"]),
    // Code editors
    p!(
        "vscode",
        "Visual Studio Code",
        CodeEditors,
        ALL,
        ["code.exe", "com.microsoft.vscode", "code"]
    ),
    // Browsers
    p!(
        "chrome",
        "Google Chrome",
        Browsers,
        ALL,
        ["chrome.exe", "com.google.chrome", "google-chrome", "chrome"]
    ),
    p!(
        "edge",
        "Microsoft Edge",
        Browsers,
        ALL,
        ["msedge.exe", "com.microsoft.edgemac", "microsoft-edge"]
    ),
    p!(
        "firefox",
        "Firefox",
        Browsers,
        ALL,
        ["firefox.exe", "org.mozilla.firefox", "firefox"]
    ),
    p!("safari", "Safari", Browsers, MAC, ["com.apple.safari"]),
    p!(
        "brave",
        "Brave",
        Browsers,
        ALL,
        ["brave.exe", "com.brave.browser", "brave-browser"]
    ),
    // Chat
    p!(
        "slack",
        "Slack",
        Chat,
        ALL,
        ["slack.exe", "com.tinyspeck.slackmacgap", "slack"]
    ),
    p!(
        "teams",
        "Microsoft Teams",
        Chat,
        WIN | MAC,
        [
            "ms-teams.exe",
            "teams.exe",
            "com.microsoft.teams2",
            "com.microsoft.teams"
        ]
    ),
    p!(
        "discord",
        "Discord",
        Chat,
        ALL,
        ["discord.exe", "com.hnc.discord", "discord"]
    ),
    // Design
    p!(
        "photoshop",
        "Adobe Photoshop",
        Design,
        WIN | MAC,
        ["photoshop.exe", "com.adobe.photoshop"]
    ),
    // Password managers (pre-ticked in the capture list)
    p!("keepass", "KeePass", PasswordManagers, WIN, ["keepass.exe"]),
    p!(
        "keepassxc",
        "KeePassXC",
        PasswordManagers,
        ALL,
        ["keepassxc.exe", "org.keepassxc.keepassxc", "keepassxc"]
    ),
    p!(
        "1password",
        "1Password",
        PasswordManagers,
        ALL,
        [
            "1password.exe",
            "com.1password.1password",
            "com.agilebits.onepassword7",
            "1password"
        ]
    ),
    p!(
        "bitwarden",
        "Bitwarden",
        PasswordManagers,
        ALL,
        ["bitwarden.exe", "com.bitwarden.desktop", "bitwarden"]
    ),
    p!(
        "lastpass",
        "LastPass",
        PasswordManagers,
        WIN | MAC,
        ["lastpass.exe", "com.lastpass.lastpass"]
    ),
    p!(
        "dashlane",
        "Dashlane",
        PasswordManagers,
        WIN | MAC,
        ["dashlane.exe", "com.dashlane.dashlanephonefinal"]
    ),
    p!(
        "keeper",
        "Keeper",
        PasswordManagers,
        ALL,
        [
            "keeperpasswordmanager.exe",
            "com.keepersecurity.passwordmanager",
            "keeper password manager"
        ]
    ),
    p!(
        "nordpass",
        "NordPass",
        PasswordManagers,
        ALL,
        ["nordpass.exe", "com.nordsec.nordpass", "nordpass"]
    ),
    p!(
        "enpass",
        "Enpass",
        PasswordManagers,
        ALL,
        ["enpass.exe", "in.sinew.enpass-desktop", "enpass"]
    ),
    p!(
        "roboform",
        "RoboForm",
        PasswordManagers,
        WIN | MAC,
        ["roboform.exe", "robotaskbaricon.exe", "com.siber.roboform"]
    ),
    p!(
        "proton-pass",
        "Proton Pass",
        PasswordManagers,
        ALL,
        [
            "proton pass.exe",
            "protonpass.exe",
            "me.proton.pass.electron",
            "proton-pass"
        ]
    ),
];

pub fn preset(id: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.id == id)
}

/// Presets of one group available on a platform, in catalogue order.
pub fn presets_in(group: Group, platform: Platform) -> impl Iterator<Item = &'static Preset> {
    PRESETS
        .iter()
        .filter(move |p| p.group == group && p.available_on(platform))
}

/// Settings entries (`preset:<id>`) for the password managers on this platform;
/// the default capture exclusion list.
pub fn password_manager_entries(platform: Platform) -> Vec<String> {
    presets_in(Group::PasswordManagers, platform)
        .map(Preset::entry)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn catalogue_is_well_formed() {
        let mut ids = HashSet::new();
        for p in PRESETS {
            assert!(ids.insert(p.id), "duplicate preset id {}", p.id);
            assert!(!p.identifiers.is_empty());
            for ident in p.identifiers {
                assert_eq!(*ident, ident.to_lowercase(), "{} must be lower-case", p.id);
            }
            assert!(p.platforms != 0);
        }
        for platform in [Platform::Windows, Platform::MacOs, Platform::Linux] {
            assert!(presets_in(Group::Terminals, platform).count() >= 3);
            assert!(!password_manager_entries(platform).is_empty());
        }
    }
}
