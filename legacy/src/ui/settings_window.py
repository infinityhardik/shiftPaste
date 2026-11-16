"""Settings window for Shift Paste configuration.

Provides UI for:
- Hotkey configuration
- Clipboard history settings
- Master file management
- Startup and security options
- Application exclusion list
"""

from PySide6.QtWidgets import (
    QApplication, QDialog, QVBoxLayout, QHBoxLayout, QLabel, QLineEdit,
    QPushButton, QSpinBox, QComboBox, QCheckBox, QGroupBox,
    QFileDialog, QListWidget, QListWidgetItem, QScrollArea, QWidget,
    QMessageBox
)
from PySide6.QtCore import Qt, Signal, QSize
from PySide6.QtGui import QFont
from typing import Optional, List
from .styles import get_settings_stylesheet


class RemovableListItem(QWidget):
    """Custom widget for list items with a delete button.
    
    Used for master files and excluded apps lists.
    """
    
    deleted = Signal(object)  # Emits the data associated with this item

    def __init__(self, text: str, data=None, parent=None):
        super().__init__(parent)
        self.data = data
        
        layout = QHBoxLayout(self)
        layout.setContentsMargins(8, 4, 8, 4)
        layout.setSpacing(8)
        
        self.label = QLabel(text)
        self.label.setStyleSheet("color: #333333;")
        layout.addWidget(self.label, 1)
        
        self.delete_btn = QPushButton("✕")
        self.delete_btn.setFixedSize(26, 26)
        self.delete_btn.setCursor(Qt.CursorShape.PointingHandCursor)
        self.delete_btn.setToolTip("Delete this item")
        self.delete_btn.setStyleSheet("""
            QPushButton { 
                border: 1px solid #d0d0d0; 
                color: #ff4444;
                font-weight: bold;
                font-size: 10pt;
                border-radius: 13px;
                background-color: #f8f8f8;
            }
            QPushButton:hover { 
                background-color: #ff4444; 
                color: white; 
                border-color: #cc0000;
            }
            QPushButton:pressed {
                background-color: #cc0000;
            }
        """)
        self.delete_btn.clicked.connect(self._on_delete)
        layout.addWidget(self.delete_btn)

    def sizeHint(self):
        """Provide a hint for the list item size."""
        return QSize(200, 40)

    def _on_delete(self):
        """Handle delete button click."""
        self.deleted.emit(self.data)


class SettingsWindow(QDialog):
    """Settings dialog for application configuration."""

    settings_changed = Signal()
    
    # Target Window dimensions
    WINDOW_WIDTH = 580
    WINDOW_HEIGHT = 700

    def __init__(self, db, master_manager, parent=None):
        """Initialize settings window.
        
        Args:
            db: Database instance for reading/writing settings
            master_manager: MasterManager instance for file operations
            parent: Parent widget
        """
        super().__init__(parent)
        self.db = db
        self.master_manager = master_manager
        
        self.setWindowTitle("Shift Paste Settings")
        
        # Calculate safe window size based on screen
        screen = QApplication.primaryScreen().availableGeometry()
        width = min(self.WINDOW_WIDTH, screen.width() - 40)
        height = min(self.WINDOW_HEIGHT, screen.height() - 80)
        
        self.resize(width, height)
        self.setMinimumWidth(450)
        self.setMinimumHeight(min(500, height))
        
        self.setWindowFlags(self.windowFlags() & ~Qt.WindowType.WindowContextHelpButtonHint)
        
        self._init_ui()
        self.setStyleSheet(get_settings_stylesheet())

    def _init_ui(self):
        """Initialize UI components."""
        main_layout = QVBoxLayout(self)
        main_layout.setContentsMargins(0, 0, 0, 0)
        main_layout.setSpacing(0)
        
        # Scrollable content area
        scroll = QScrollArea()
        scroll.setWidgetResizable(True)
        scroll.setFrameShape(QScrollArea.Shape.NoFrame)
        
        content_widget = QWidget()
        content_layout = QVBoxLayout(content_widget)
        content_layout.setContentsMargins(16, 16, 16, 16)
        content_layout.setSpacing(16)

        # Add setting groups
        content_layout.addWidget(self._create_hotkey_group())
        content_layout.addWidget(self._create_history_group())
        content_layout.addWidget(self._create_masters_group())
        content_layout.addWidget(self._create_security_group())
        content_layout.addWidget(self._create_exclusions_group())
        content_layout.addWidget(self._create_advanced_group())
        
        content_layout.addStretch()
        
        scroll.setWidget(content_widget)
        main_layout.addWidget(scroll, 1)

        # Button bar
        self._create_button_bar(main_layout)

    def _create_hotkey_group(self) -> QGroupBox:
        """Create hotkey settings group."""
        group = QGroupBox("Hotkey")
        layout = QVBoxLayout(group)
        layout.setSpacing(8)
        
        row = QHBoxLayout()
        row.addWidget(QLabel("Keyboard Shortcut:"))
        
        self.hotkey_input = QLineEdit(self.db.get_setting('hotkey', 'Ctrl+Shift+V'))
        self.hotkey_input.setPlaceholderText("e.g., Ctrl+Shift+V")
        self.hotkey_input.setMaximumWidth(180)
        row.addWidget(self.hotkey_input)
        row.addStretch()
        
        layout.addLayout(row)
        layout.addWidget(QLabel(
            "<i style='color:#666'>Press your desired key combination in the field above</i>"
        ))
        
        return group

    def _create_history_group(self) -> QGroupBox:
        """Create clipboard history settings group."""
        group = QGroupBox("Clipboard History")
        layout = QVBoxLayout(group)
        layout.setSpacing(10)
        
        # History limit
        row1 = QHBoxLayout()
        row1.addWidget(QLabel("History Limit:"))
        self.limit_combo = QComboBox()
        self.limit_combo.addItems(["25", "50", "100", "200", "500", "Unlimited"])
        self.limit_combo.setCurrentText(self.db.get_setting('history_limit', '50'))
        self.limit_combo.setMaximumWidth(120)
        row1.addWidget(self.limit_combo)
        row1.addStretch()
        layout.addLayout(row1)
        
        # Preview settings
        row2 = QHBoxLayout()
        row2.addWidget(QLabel("Preview Length:"))
        self.max_chars = QSpinBox()
        self.max_chars.setRange(20, 300)
        self.max_chars.setValue(int(self.db.get_setting('preview_length', '50')))
        self.max_chars.setSuffix(" chars")
        self.max_chars.setMaximumWidth(100)
        row2.addWidget(self.max_chars)
        row2.addStretch()
        layout.addLayout(row2)

        # Formatting option
        self.cb_formatting = QCheckBox("Preserve text formatting (RTF/HTML)")
        self.cb_formatting.setChecked(self.db.get_setting('preserve_formatting', 'False') == 'True')
        layout.addWidget(self.cb_formatting)

        self.cb_timestamps = QCheckBox("Show timestamps in list")
        self.cb_timestamps.setChecked(self.db.get_setting('show_timestamps', 'True') == 'True')
        layout.addWidget(self.cb_timestamps)
        
        # Auto-clear
        row3 = QHBoxLayout()
        row3.addWidget(QLabel("Auto-clear history:"))
        self.clear_combo = QComboBox()
        self.clear_combo.addItems(["Never", "Daily", "Weekly", "Monthly"])
        self.clear_combo.setCurrentText(self.db.get_setting('auto_clear', 'Never'))
        self.clear_combo.setMaximumWidth(120)
        row3.addWidget(self.clear_combo)
        row3.addStretch()
        layout.addLayout(row3)
        
        return group

    def _create_masters_group(self) -> QGroupBox:
        """Create master files settings group."""
        group = QGroupBox("Master Files")
        layout = QVBoxLayout(group)
        layout.setSpacing(10)
        
        self.cb_enable_masters = QCheckBox("Enable Master Search")
        self.cb_enable_masters.setChecked(self.db.get_setting('enable_masters', 'True') == 'True')
        layout.addWidget(self.cb_enable_masters)
        
        layout.addWidget(QLabel("Registered Excel files:"))

        self.master_list = QListWidget()
        self.master_list.setFixedHeight(150)
        self.master_list.setAlternatingRowColors(True)
        layout.addWidget(self.master_list)
        self._load_master_files()
        
        btn_add = QPushButton("➕ Add Excel File...")
        btn_add.clicked.connect(self._add_master_file)
        layout.addWidget(btn_add)
        
        return group

    def _create_security_group(self) -> QGroupBox:
        """Create startup and security settings group."""
        group = QGroupBox("Startup & Security")
        layout = QVBoxLayout(group)
        layout.setSpacing(8)
        
        self.cb_startup = QCheckBox("Run on system startup")
        self.cb_startup.setChecked(self.db.get_setting('run_on_startup', 'True') == 'True')
        layout.addWidget(self.cb_startup)
        
        self.cb_exclude_pass = QCheckBox("Ignore clipboard from password managers")
        self.cb_exclude_pass.setChecked(self.db.get_setting('exclude_password_managers', 'True') == 'True')
        self.cb_exclude_pass.setToolTip(
            "When enabled, clipboard content copied from known password managers\n"
            "(KeePass, 1Password, Bitwarden, etc.) will not be saved to history."
        )
        layout.addWidget(self.cb_exclude_pass)
        
        return group

    def _create_exclusions_group(self) -> QGroupBox:
        """Create application exclusion settings group."""
        group = QGroupBox("Exclude Hotkey in Applications")
        layout = QVBoxLayout(group)
        layout.setSpacing(10)
        
        layout.addWidget(QLabel("Ignore the hotkey when these apps are active:"))
        
        self.exclude_list = QListWidget()
        self.exclude_list.setFixedHeight(120)
        self.exclude_list.setAlternatingRowColors(True)
        layout.addWidget(self.exclude_list)
        self._load_excluded_apps()
        
        # Add app row
        add_row = QHBoxLayout()
        self.app_input = QLineEdit()
        self.app_input.setPlaceholderText("Application name (e.g., Photoshop.exe)")
        self.app_input.returnPressed.connect(self._add_excluded_app)
        add_row.addWidget(self.app_input, 1)
        
        btn_add_app = QPushButton("Add")
        btn_add_app.clicked.connect(self._add_excluded_app)
        add_row.addWidget(btn_add_app)
        layout.addLayout(add_row)
        
        return group

    def _create_advanced_group(self) -> QGroupBox:
        """Create advanced settings group."""
        group = QGroupBox("Advanced")
        layout = QVBoxLayout(group)
        layout.setSpacing(8)
        
        row = QHBoxLayout()
        row.addWidget(QLabel("Search delay:"))
        self.search_delay = QSpinBox()
        self.search_delay.setRange(0, 500)
        self.search_delay.setValue(int(self.db.get_setting('search_debounce_ms', '80')))
        self.search_delay.setSuffix(" ms")
        self.search_delay.setMaximumWidth(100)
        self.search_delay.setToolTip("Delay before search executes (reduces CPU usage while typing)")
        row.addWidget(self.search_delay)
        row.addStretch()
        layout.addLayout(row)
        
        return group

    def _create_button_bar(self, parent_layout: QVBoxLayout):
        """Create save/cancel button bar."""
        bar = QWidget()
        bar.setStyleSheet("background-color: #f0f0f0; border-top: 1px solid #d0d0d0;")
        bar.setFixedHeight(60)
        
        layout = QHBoxLayout(bar)
        layout.setContentsMargins(16, 0, 16, 0)
        layout.addStretch()
        
        btn_save = QPushButton("Save Changes")
        btn_save.clicked.connect(self._save_all)
        btn_save.setStyleSheet("""
            QPushButton {
                background-color: #0078d4;
                color: white;
                padding: 10px 24px;
                border: none;
                border-radius: 6px;
                font-weight: bold;
            }
            QPushButton:hover {
                background-color: #106ebe;
            }
            QPushButton:pressed {
                background-color: #005a9e;
            }
        """)
        layout.addWidget(btn_save)
        
        btn_cancel = QPushButton("Cancel")
        btn_cancel.clicked.connect(self.close)
        btn_cancel.setStyleSheet("""
            QPushButton {
                padding: 10px 20px;
            }
        """)
        layout.addWidget(btn_cancel)
        
        parent_layout.addWidget(bar)

    def _load_master_files(self):
        """Load master files into the list widget."""
        self.master_list.clear()
        
        try:
            cursor = self.db.conn.cursor()
            cursor.execute("SELECT id, file_path, is_enabled, last_error FROM master_files")
            
            for row in cursor.fetchall():
                file_path = row['file_path']
                filename = file_path.replace('\\', '/').split('/')[-1]
                
                status = "✓ Enabled" if row['is_enabled'] else "✗ Disabled"
                if row['last_error']:
                    status = "⚠ Error"
                
                item = QListWidgetItem(self.master_list)
                widget = RemovableListItem(f"{filename}  ({status})", data=row['id'])
                widget.deleted.connect(self._remove_master_file)
                item.setSizeHint(widget.sizeHint())
                self.master_list.addItem(item)
                self.master_list.setItemWidget(item, widget)
        except Exception as e:
            print(f"[WARN] Could not load master files: {e}")

    def _remove_master_file(self, file_id: int):
        """Remove a master file from the database."""
        try:
            self.db.delete_master_file(file_id)
            self._load_master_files()
            if self.master_manager:
                self.master_manager.refresh_watcher()
        except Exception as e:
            QMessageBox.warning(self, "Error", f"Could not remove file: {e}")

    def _load_excluded_apps(self):
        """Load excluded apps into the list widget."""
        self.exclude_list.clear()
        
        excluded_str = self.db.get_setting('excluded_apps', '')
        if not excluded_str:
            return
            
        for app in excluded_str.split(','):
            app = app.strip()
            if not app:
                continue
                
            item = QListWidgetItem(self.exclude_list)
            widget = RemovableListItem(app, data=app)
            widget.deleted.connect(self._remove_excluded_app)
            item.setSizeHint(widget.sizeHint())
            self.exclude_list.addItem(item)
            self.exclude_list.setItemWidget(item, widget)

    def _remove_excluded_app(self, app_name: str):
        """Remove an excluded app from the list."""
        for i in range(self.exclude_list.count()):
            item = self.exclude_list.item(i)
            widget = self.exclude_list.itemWidget(item)
            if widget and widget.data == app_name:
                self.exclude_list.takeItem(i)
                break

    def _add_master_file(self):
        """Open file dialog to add a new master Excel file."""
        file_path, _ = QFileDialog.getOpenFileName(
            self, 
            "Select Excel Master File", 
            "", 
            "Excel Files (*.xlsx);;All Files (*)"
        )
        
        if file_path:
            try:
                self.db.add_master_file(file_path)
                self._load_master_files()
                
                if self.master_manager:
                    self.master_manager.refresh_watcher()
                    self.master_manager.rebuild_index(file_path)
            except Exception as e:
                QMessageBox.warning(self, "Error", f"Could not add file: {e}")

    def _add_excluded_app(self):
        """Add an app to the exclusion list."""
        app = self.app_input.text().strip()
        if not app:
            return
        
        # Check for duplicates
        for i in range(self.exclude_list.count()):
            item = self.exclude_list.item(i)
            widget = self.exclude_list.itemWidget(item)
            if widget and widget.data.lower() == app.lower():
                self.app_input.clear()
                return  # Already exists
        
        item = QListWidgetItem(self.exclude_list)
        widget = RemovableListItem(app, data=app)
        widget.deleted.connect(self._remove_excluded_app)
        item.setSizeHint(widget.sizeHint())
        self.exclude_list.addItem(item)
        self.exclude_list.setItemWidget(item, widget)
        self.app_input.clear()

    def _save_all(self):
        """Save all settings to the database."""
        try:
            self.db.set_setting('hotkey', self.hotkey_input.text().strip())
            self.db.set_setting('history_limit', self.limit_combo.currentText())
            self.db.set_setting('preview_length', str(self.max_chars.value()))
            self.db.set_setting('preserve_formatting', str(self.cb_formatting.isChecked()))
            self.db.set_setting('show_timestamps', str(self.cb_timestamps.isChecked()))
            self.db.set_setting('auto_clear', self.clear_combo.currentText())
            self.db.set_setting('enable_masters', str(self.cb_enable_masters.isChecked()))
            self.db.set_setting('run_on_startup', str(self.cb_startup.isChecked()))
            self.db.set_setting('exclude_password_managers', str(self.cb_exclude_pass.isChecked()))
            self.db.set_setting('search_debounce_ms', str(self.search_delay.value()))
            
            # Collect excluded apps
            apps = []
            for i in range(self.exclude_list.count()):
                item = self.exclude_list.item(i)
                widget = self.exclude_list.itemWidget(item)
                if widget and widget.data:
                    apps.append(widget.data)
            self.db.set_setting('excluded_apps', ','.join(apps))
            
            self.settings_changed.emit()
            self.close()
            
        except Exception as e:
            QMessageBox.critical(self, "Error", f"Could not save settings: {e}")
