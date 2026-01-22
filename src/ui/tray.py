"""System tray icon and menu for Shift Paste."""

from PySide6.QtWidgets import QSystemTrayIcon, QMenu
from PySide6.QtGui import QIcon, QAction
from PySide6.QtCore import Signal, QObject

class SystemTrayManager(QObject):
    """Manages the system tray icon and its context menu."""

    show_requested = Signal()
    settings_requested = Signal()
    clear_requested = Signal()
    exit_requested = Signal()

    def __init__(self, parent=None):
        super().__init__(parent)
        self.tray_icon = QSystemTrayIcon(parent)
        # Use a fallback pixmap icon if theme is missing (common on Windows)
        icon = QIcon.fromTheme("edit-paste")
        if icon.isNull():
            from PySide6.QtGui import QPixmap, QPainter, QColor
            pixmap = QPixmap(32, 32)
            pixmap.fill(Qt.GlobalColor.transparent)
            painter = QPainter(pixmap)
            painter.setBrush(QColor("#2196F3"))
            painter.setPen(Qt.GlobalColor.transparent)
            painter.drawEllipse(2, 2, 28, 28)
            painter.end()
            icon = QIcon(pixmap)
            
        self.tray_icon.setIcon(icon)
        self.tray_icon.setToolTip("Shift Paste")
        
        self._init_menu()
        self.tray_icon.activated.connect(self._on_activated)
        self.tray_icon.show()

    def _init_menu(self):
        """Build the tray context menu."""
        menu = QMenu()
        
        open_action = QAction("Open Shift Paste", self)
        open_action.triggered.connect(self.show_requested.emit)
        menu.addAction(open_action)
        
        settings_action = QAction("Settings", self)
        settings_action.triggered.connect(self.settings_requested.emit)
        menu.addAction(settings_action)
        
        menu.addSeparator()
        
        clear_action = QAction("Clear History", self)
        clear_action.triggered.connect(self.clear_requested.emit)
        menu.addAction(clear_action)
        
        menu.addSeparator()
        
        exit_action = QAction("Exit", self)
        exit_action.triggered.connect(self.exit_requested.emit)
        menu.addAction(exit_action)
        
        self.tray_icon.setContextMenu(menu)

    def _on_activated(self, reason):
        """Handle tray icon activation (e.g., left-click)."""
        if reason == QSystemTrayIcon.ActivationReason.Trigger:
            self.show_requested.emit()
            
    def set_icon(self, icon_path):
        """Set a custom icon for the tray."""
        self.tray_icon.setIcon(QIcon(icon_path))
