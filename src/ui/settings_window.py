"""Settings window for Shift Paste configuration."""

from PySide6.QtWidgets import (
    QDialog, QVBoxLayout, QHBoxLayout, QLabel, QLineEdit,
    QPushButton, QSpinBox, QComboBox, QCheckBox, QGroupBox
)
from PySide6.QtCore import Qt, Signal
from PySide6.QtGui import QFont
from src.data.config_manager import ConfigManager


class SettingsWindow(QDialog):
    """Settings dialog for application configuration."""

    settings_changed = Signal()

    def __init__(self, config: ConfigManager, parent=None):
        """Initialize settings window.

        Args:
            config: ConfigManager instance
            parent: Parent widget
        """
        super().__init__(parent)
        self.config = config
        self.setWindowTitle("Shift Paste Settings")
        self.setGeometry(100, 100, 500, 600)
        self._init_ui()

    def _init_ui(self):
        """Initialize UI components."""
        layout = QVBoxLayout()

        # Clipboard settings
        clipboard_group = QGroupBox("Clipboard")
        clipboard_layout = QVBoxLayout()

        # Max items
        max_items_layout = QHBoxLayout()
        max_items_layout.addWidget(QLabel("Max items to store:"))
        self.max_items_spinbox = QSpinBox()
        self.max_items_spinbox.setMinimum(10)
        self.max_items_spinbox.setMaximum(500)
        self.max_items_spinbox.setValue(
            self.config.get('clipboard.max_items', 20)
        )
        max_items_layout.addWidget(self.max_items_spinbox)
        max_items_layout.addStretch()

        # Preview chars
        preview_layout = QHBoxLayout()
        preview_layout.addWidget(QLabel("Preview characters:"))
        self.preview_spinbox = QSpinBox()
        self.preview_spinbox.setMinimum(50)
        self.preview_spinbox.setMaximum(200)
        self.preview_spinbox.setValue(
            self.config.get('clipboard.preview_chars', 100)
        )
        preview_layout.addWidget(self.preview_spinbox)
        preview_layout.addStretch()

        clipboard_layout.addLayout(max_items_layout)
        clipboard_layout.addLayout(preview_layout)
        clipboard_group.setLayout(clipboard_layout)

        # Shortcuts settings
        shortcuts_group = QGroupBox("Keyboard Shortcuts")
        shortcuts_layout = QVBoxLayout()

        # Windows shortcut
        win_shortcut_layout = QHBoxLayout()
        win_shortcut_layout.addWidget(QLabel("Windows:"))
        self.win_shortcut_input = QLineEdit()
        self.win_shortcut_input.setText(
            self.config.get('shortcuts.windows', 'shift+win+v')
        )
        win_shortcut_layout.addWidget(self.win_shortcut_input)

        # macOS shortcut
        mac_shortcut_layout = QHBoxLayout()
        mac_shortcut_layout.addWidget(QLabel("macOS:"))
        self.mac_shortcut_input = QLineEdit()
        self.mac_shortcut_input.setText(
            self.config.get('shortcuts.macos', 'shift+cmd+v')
        )
        mac_shortcut_layout.addWidget(self.mac_shortcut_input)

        # Linux shortcut
        linux_shortcut_layout = QHBoxLayout()
        linux_shortcut_layout.addWidget(QLabel("Linux:"))
        self.linux_shortcut_input = QLineEdit()
        self.linux_shortcut_input.setText(
            self.config.get('shortcuts.linux', 'shift+super+v')
        )
        linux_shortcut_layout.addWidget(self.linux_shortcut_input)

        shortcuts_layout.addLayout(win_shortcut_layout)
        shortcuts_layout.addLayout(mac_shortcut_layout)
        shortcuts_layout.addLayout(linux_shortcut_layout)
        shortcuts_group.setLayout(shortcuts_layout)

        # Master files settings
        master_group = QGroupBox("Master Files")
        master_layout = QVBoxLayout()

        # Auto reload
        self.auto_reload_checkbox = QCheckBox("Auto-reload when Excel files change")
        self.auto_reload_checkbox.setChecked(
            self.config.get('master_file.auto_reload', True)
        )
        master_layout.addWidget(self.auto_reload_checkbox)

        master_group.setLayout(master_layout)

        # UI settings
        ui_group = QGroupBox("User Interface")
        ui_layout = QVBoxLayout()

        # Theme
        theme_layout = QHBoxLayout()
        theme_layout.addWidget(QLabel("Theme:"))
        self.theme_combo = QComboBox()
        self.theme_combo.addItems(["System", "Light", "Dark"])
        current_theme = self.config.get('ui.theme', 'system').capitalize()
        self.theme_combo.setCurrentText(current_theme)
        theme_layout.addWidget(self.theme_combo)
        theme_layout.addStretch()

        # Max visible items
        visible_layout = QHBoxLayout()
        visible_layout.addWidget(QLabel("Max visible items:"))
        self.visible_spinbox = QSpinBox()
        self.visible_spinbox.setMinimum(5)
        self.visible_spinbox.setMaximum(20)
        self.visible_spinbox.setValue(
            self.config.get('ui.max_visible_items', 8)
        )
        visible_layout.addWidget(self.visible_spinbox)
        visible_layout.addStretch()

        ui_layout.addLayout(theme_layout)
        ui_layout.addLayout(visible_layout)
        ui_group.setLayout(ui_layout)

        # Startup settings
        startup_group = QGroupBox("Startup")
        startup_layout = QVBoxLayout()

        self.autostart_checkbox = QCheckBox("Run on system startup")
        self.autostart_checkbox.setChecked(
            self.config.get('startup.run_on_boot', False)
        )
        startup_layout.addWidget(self.autostart_checkbox)

        startup_group.setLayout(startup_layout)

        # Buttons
        button_layout = QHBoxLayout()
        button_layout.addStretch()

        save_button = QPushButton("Save")
        save_button.clicked.connect(self._save_settings)
        button_layout.addWidget(save_button)

        cancel_button = QPushButton("Cancel")
        cancel_button.clicked.connect(self.close)
        button_layout.addWidget(cancel_button)

        # Main layout
        layout.addWidget(clipboard_group)
        layout.addWidget(shortcuts_group)
        layout.addWidget(master_group)
        layout.addWidget(ui_group)
        layout.addWidget(startup_group)
        layout.addStretch()
        layout.addLayout(button_layout)

        self.setLayout(layout)

    def _save_settings(self):
        """Save settings to config."""
        self.config.set('clipboard.max_items', self.max_items_spinbox.value())
        self.config.set('clipboard.preview_chars', self.preview_spinbox.value())

        self.config.set('shortcuts.windows', self.win_shortcut_input.text())
        self.config.set('shortcuts.macos', self.mac_shortcut_input.text())
        self.config.set('shortcuts.linux', self.linux_shortcut_input.text())

        self.config.set('master_file.auto_reload', self.auto_reload_checkbox.isChecked())

        self.config.set('ui.theme', self.theme_combo.currentText().lower())
        self.config.set('ui.max_visible_items', self.visible_spinbox.value())

        self.config.set('startup.run_on_boot', self.autostart_checkbox.isChecked())

        self.settings_changed.emit()
        self.close()
