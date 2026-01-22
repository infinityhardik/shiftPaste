"""Settings window for Shift Paste configuration."""

from PySide6.QtWidgets import (
    QDialog, QVBoxLayout, QHBoxLayout, QLabel, QLineEdit,
    QPushButton, QSpinBox, QComboBox, QCheckBox, QGroupBox,
    QFileDialog, QListWidget, QListWidgetItem, QScrollArea, QWidget
)
from PySide6.QtCore import Qt, Signal
from PySide6.QtGui import QFont, QIcon, QColor

class RemovableListItem(QWidget):
    """Custom widget for list items with a delete button."""
    deleted = Signal(object) # Emits data associated with item

    def __init__(self, text, data=None, parent=None):
        super().__init__(parent)
        self.data = data
        layout = QHBoxLayout(self)
        layout.setContentsMargins(5, 2, 5, 2)
        
        self.label = QLabel(text)
        layout.addWidget(self.label)
        layout.addStretch()
        
        self.delete_btn = QPushButton("✕")
        self.delete_btn.setFixedSize(20, 20)
        self.delete_btn.setCursor(Qt.CursorShape.PointingHandCursor)
        self.delete_btn.setStyleSheet("""
            QPushButton { 
                border: none; 
                color: #666; 
                font-weight: bold;
                border-radius: 10px;
            }
            QPushButton:hover { 
                background-color: #ffcdd2; 
                color: #c62828; 
            }
        """)
        self.delete_btn.clicked.connect(lambda: self.deleted.emit(self.data))
        layout.addWidget(self.delete_btn)


class SettingsWindow(QDialog):
    """Settings dialog for application configuration."""

    settings_changed = Signal()

    def __init__(self, db, master_manager, parent=None):
        """Initialize settings window."""
        super().__init__(parent)
        self.db = db
        self.master_manager = master_manager
        self.setWindowTitle("Shift Paste Settings")
        self.setFixedSize(500, 650)
        self._init_ui()

    def _init_ui(self):
        """Initialize UI components based on technical specifications."""
        main_layout = QVBoxLayout(self)
        
        scroll = QScrollArea()
        scroll.setWidgetResizable(True)
        scroll.setFrameShape(QScrollArea.Shape.NoFrame)
        
        content_widget = QWidget()
        layout = QVBoxLayout(content_widget)
        layout.setSpacing(15)

        # 1. Hotkey
        hotkey_group = QGroupBox("Hotkey")
        hk_layout = QVBoxLayout()
        hk_sub = QHBoxLayout()
        self.hotkey_input = QLineEdit(self.db.get_setting('hotkey', 'Ctrl+Shift+V'))
        hk_sub.addWidget(self.hotkey_input)
        hk_sub.addWidget(QPushButton("Change"))
        hk_layout.addLayout(hk_sub)
        hotkey_group.setLayout(hk_layout)
        layout.addWidget(hotkey_group)

        # 2. Clipboard History
        history_group = QGroupBox("Clipboard History")
        hist_layout = QVBoxLayout()
        
        limit_layout = QHBoxLayout()
        limit_layout.addWidget(QLabel("History Limit:"))
        self.limit_combo = QComboBox()
        self.limit_combo.addItems(["25", "50", "100", "200", "Unlimited"])
        self.limit_combo.setCurrentText(self.db.get_setting('history_limit', '50'))
        limit_layout.addWidget(self.limit_combo)
        hist_layout.addLayout(limit_layout)

        prev_layout = QHBoxLayout()
        prev_layout.addWidget(QLabel("Max Characters:"))
        self.max_chars = QSpinBox()
        self.max_chars.setRange(20, 200)
        self.max_chars.setValue(int(self.db.get_setting('preview_length', 50)))
        prev_layout.addWidget(self.max_chars)
        
        prev_layout.addWidget(QLabel("Max Lines:"))
        self.max_lines = QSpinBox()
        self.max_lines.setRange(1, 5)
        self.max_lines.setValue(int(self.db.get_setting('preview_max_lines', 2)))
        prev_layout.addWidget(self.max_lines)
        hist_layout.addLayout(prev_layout)

        self.cb_formatting = QCheckBox("Preserve text formatting (RTF/HTML)")
        self.cb_formatting.setChecked(self.db.get_setting('preserve_formatting', 'False') == 'True')
        hist_layout.addWidget(self.cb_formatting)

        self.cb_timestamps = QCheckBox("Show timestamps")
        self.cb_timestamps.setChecked(self.db.get_setting('show_timestamps', 'True') == 'True')
        hist_layout.addWidget(self.cb_timestamps)
        
        clear_layout = QHBoxLayout()
        clear_layout.addWidget(QLabel("Auto-clear:"))
        self.clear_combo = QComboBox()
        self.clear_combo.addItems(["Never", "Daily", "Weekly"])
        self.clear_combo.setCurrentText(self.db.get_setting('auto_clear', 'Never'))
        clear_layout.addWidget(self.clear_combo)
        hist_layout.addLayout(clear_layout)

        history_group.setLayout(hist_layout)
        layout.addWidget(history_group)

        # 3. Master Files
        master_group = QGroupBox("Master Files")
        m_layout = QVBoxLayout()
        self.cb_enable_masters = QCheckBox("Enable Master Search")
        self.cb_enable_masters.setChecked(self.db.get_setting('enable_masters', 'True') == 'True')
        m_layout.addWidget(self.cb_enable_masters)

        self.master_list = QListWidget()
        self.master_list.setFixedHeight(120)
        m_layout.addWidget(self.master_list)
        self._load_master_files()
        
        m_buttons = QHBoxLayout()
        btn_add = QPushButton("+ Add Master File")
        btn_add.clicked.connect(self._add_master_file)
        m_buttons.addWidget(btn_add)
        m_layout.addLayout(m_buttons)
        
        master_group.setLayout(m_layout)
        layout.addWidget(master_group)

        # 4. Startup & Security
        security_group = QGroupBox("Startup & Security")
        sec_layout = QVBoxLayout()
        self.cb_startup = QCheckBox("Run on system startup")
        self.cb_startup.setChecked(self.db.get_setting('run_on_startup', 'True') == 'True')
        sec_layout.addWidget(self.cb_startup)
        
        self.cb_exclude_pass = QCheckBox("Exclude password managers")
        self.cb_exclude_pass.setChecked(self.db.get_setting('exclude_password_managers', 'True') == 'True')
        sec_layout.addWidget(self.cb_exclude_pass)
        
        security_group.setLayout(sec_layout)
        layout.addWidget(security_group)

        # 5. Application Exclusion (New Feature)
        exclude_group = QGroupBox("Exclude Hotkey in Apps")
        ex_layout = QVBoxLayout()
        ex_layout.addWidget(QLabel("Ignore shortcut when these apps are active:"))
        self.exclude_list = QListWidget()
        self.exclude_list.setFixedHeight(100)
        ex_layout.addWidget(self.exclude_list)
        self._load_excluded_apps()
        
        ex_btns = QHBoxLayout()
        self.app_input = QLineEdit()
        self.app_input.setPlaceholderText("e.g. Photoshop.exe")
        ex_btns.addWidget(self.app_input)
        btn_add_app = QPushButton("Add")
        btn_add_app.clicked.connect(self._add_excluded_app)
        ex_btns.addWidget(btn_add_app)
        ex_layout.addLayout(ex_btns)
        
        exclude_group.setLayout(ex_layout)
        layout.addWidget(exclude_group)

        # 6. Advanced
        adv_group = QGroupBox("Advanced")
        adv_layout = QVBoxLayout()
        
        delay_layout = QHBoxLayout()
        delay_layout.addWidget(QLabel("Search delay (ms):"))
        self.search_delay = QSpinBox()
        self.search_delay.setRange(0, 1000)
        self.search_delay.setValue(int(self.db.get_setting('search_debounce_ms', 100)))
        delay_layout.addWidget(self.search_delay)
        adv_layout.addLayout(delay_layout)
        
        adv_group.setLayout(adv_layout)
        layout.addWidget(adv_group)

        scroll.setWidget(content_widget)
        main_layout.addWidget(scroll)

        # Form Buttons
        btn_layout = QHBoxLayout()
        btn_layout.addStretch()
        btn_save = QPushButton("Save")
        btn_save.clicked.connect(self._save_all)
        btn_save.setStyleSheet("background-color: #0078d4; color: white; padding: 8px 20px;")
        btn_layout.addWidget(btn_save)
        btn_cancel = QPushButton("Cancel")
        btn_cancel.clicked.connect(self.close)
        btn_layout.addWidget(btn_cancel)
        main_layout.addLayout(btn_layout)

    def _load_master_files(self):
        self.master_list.clear()
        cursor = self.db.conn.cursor()
        cursor.execute("SELECT id, file_path, is_enabled FROM master_files")
        for row in cursor.fetchall():
            path_str = row['file_path']
            filename = path_str.split('\\')[-1].split('/')[-1]
            status = "Enabled" if row['is_enabled'] else "Disabled"
            
            item = QListWidgetItem(self.master_list)
            custom_widget = RemovableListItem(f"{filename} ({status})", data=row['id'])
            custom_widget.deleted.connect(self._remove_master_file)
            item.setSizeHint(custom_widget.sizeHint())
            self.master_list.addItem(item)
            self.master_list.setItemWidget(item, custom_widget)

    def _remove_master_file(self, file_id):
        self.db.delete_master_file(file_id)
        self._load_master_files()
        if self.master_manager:
            self.master_manager.refresh_watcher()

    def _load_excluded_apps(self):
        self.exclude_list.clear()
        excluded_apps = self.db.get_setting('excluded_apps', 'Excel.exe,Winword.exe').split(',')
        for app in excluded_apps:
            if not app.strip(): continue
            item = QListWidgetItem(self.exclude_list)
            custom_widget = RemovableListItem(app.strip(), data=app.strip())
            custom_widget.deleted.connect(self._remove_excluded_app)
            item.setSizeHint(custom_widget.sizeHint())
            self.exclude_list.addItem(item)
            self.exclude_list.setItemWidget(item, custom_widget)

    def _remove_excluded_app(self, app_name):
        # We need to update the semicolon/comma separated string in DB or just reload from list
        # For now, just remove from UI, save will handle the rest
        for i in range(self.exclude_list.count()):
            item = self.exclude_list.item(i)
            widget = self.exclude_list.itemWidget(item)
            if widget.data == app_name:
                self.exclude_list.takeItem(i)
                break

    def _add_master_file(self):
        file_path, _ = QFileDialog.getOpenFileName(self, "Select Excel Master File", "", "Excel Files (*.xlsx)")
        if file_path:
            self.db.add_master_file(file_path)
            self._load_master_files()
            if self.master_manager:
                self.master_manager.refresh_watcher()
                self.master_manager.rebuild_index(file_path)

    def _add_excluded_app(self):
        app = self.app_input.text().strip()
        if app:
            item = QListWidgetItem(self.exclude_list)
            custom_widget = RemovableListItem(app, data=app)
            custom_widget.deleted.connect(self._remove_excluded_app)
            item.setSizeHint(custom_widget.sizeHint())
            self.exclude_list.addItem(item)
            self.exclude_list.setItemWidget(item, custom_widget)
            self.app_input.clear()

    def _save_all(self):
        """Persist all settings to the DB."""
        self.db.set_setting('hotkey', self.hotkey_input.text())
        self.db.set_setting('history_limit', self.limit_combo.currentText())
        self.db.set_setting('preview_length', self.max_chars.value())
        self.db.set_setting('preview_max_lines', self.max_lines.value())
        self.db.set_setting('preserve_formatting', self.cb_formatting.isChecked())
        self.db.set_setting('show_timestamps', self.cb_timestamps.isChecked())
        self.db.set_setting('auto_clear', self.clear_combo.currentText())
        self.db.set_setting('enable_masters', self.cb_enable_masters.isChecked())
        self.db.set_setting('run_on_startup', self.cb_startup.isChecked())
        self.db.set_setting('exclude_password_managers', self.cb_exclude_pass.isChecked())
        self.db.set_setting('search_debounce_ms', self.search_delay.value())
        
        # Save excluded apps
        apps = []
        for i in range(self.exclude_list.count()):
            item = self.exclude_list.item(i)
            widget = self.exclude_list.itemWidget(item)
            if widget:
                apps.append(widget.data)
        self.db.set_setting('excluded_apps', ','.join(apps))

        self.settings_changed.emit()
        self.close()
