"""System tray icon and menu for Shift Paste.

Provides quick access to:
- Opening the clipboard manager
- Opening settings
- Clearing history
- Exiting the application
"""

from PySide6.QtWidgets import QSystemTrayIcon, QMenu
from PySide6.QtGui import QIcon, QAction, QPixmap, QPainter, QColor, QBrush, QPen
from PySide6.QtCore import Signal, QObject, Qt
from typing import Optional
import os


class SystemTrayManager(QObject):
    """Manages the system tray icon and its context menu.
    
    Signals:
        show_requested: User wants to open the main window
        settings_requested: User wants to open settings
        clear_requested: User wants to clear history
        exit_requested: User wants to exit the application
    """

    show_requested = Signal()
    settings_requested = Signal()
    clear_requested = Signal()
    exit_requested = Signal()

    def __init__(self, parent=None):
        super().__init__(parent)
        
        self.tray_icon = QSystemTrayIcon(parent)
        self._setup_icon()
        self._setup_menu()
        
        self.tray_icon.activated.connect(self._on_activated)
        self.tray_icon.show()

    def _setup_icon(self):
        """Setup the tray icon, with fallback if no system icon available."""
        # Try to load custom icon from resources
        icon = self._load_custom_icon()
        
        if icon is None or icon.isNull():
            # Try system theme icon
            icon = QIcon.fromTheme("edit-paste")
            
        if icon.isNull():
            # Create a fallback pixmap icon
            icon = self._create_fallback_icon()
        
        self.tray_icon.setIcon(icon)
        self.tray_icon.setToolTip("Shift Paste - Clipboard Manager\nCtrl+Shift+V to open")

    def _load_custom_icon(self) -> Optional[QIcon]:
        """Try to load custom application icon."""
        # Check for icon in common locations
        possible_paths = [
            "resources/icons/app_icon.ico",
            "resources/icons/app_icon.png",
            os.path.join(os.path.dirname(__file__), "..", "..", "resources", "icons", "app_icon.ico"),
            os.path.join(os.path.dirname(__file__), "..", "..", "resources", "icons", "app_icon.png"),
        ]
        
        for path in possible_paths:
            if os.path.exists(path):
                icon = QIcon(path)
                if not icon.isNull():
                    return icon
        
        return None

    def _create_fallback_icon(self) -> QIcon:
        """Create a simple fallback icon programmatically."""
        pixmap = QPixmap(32, 32)
        pixmap.fill(Qt.GlobalColor.transparent)
        
        painter = QPainter(pixmap)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)
        
        # Draw a blue circle
        painter.setBrush(QBrush(QColor("#0078D4")))
        painter.setPen(QPen(QColor("#005a9e"), 2))
        painter.drawEllipse(2, 2, 28, 28)
        
        # Draw a simple 'S' for Shift Paste
        painter.setPen(QPen(QColor("#ffffff"), 2))
        font = painter.font()
        font.setBold(True)
        font.setPixelSize(18)
        painter.setFont(font)
        painter.drawText(pixmap.rect(), Qt.AlignmentFlag.AlignCenter, "S")
        
        painter.end()
        
        return QIcon(pixmap)

    def _setup_menu(self):
        """Build the tray context menu."""
        menu = QMenu()
        
        # Open action
        open_action = QAction("📋 Open Shift Paste", self)
        open_action.triggered.connect(self.show_requested.emit)
        font = open_action.font()
        font.setBold(True)
        open_action.setFont(font)
        menu.addAction(open_action)
        
        menu.addSeparator()
        
        # Settings action
        settings_action = QAction("⚙️ Settings", self)
        settings_action.triggered.connect(self.settings_requested.emit)
        menu.addAction(settings_action)
        
        # Clear history action
        clear_action = QAction("🗑️ Clear History", self)
        clear_action.triggered.connect(self.clear_requested.emit)
        menu.addAction(clear_action)
        
        menu.addSeparator()
        
        # Exit action
        exit_action = QAction("❌ Exit", self)
        exit_action.triggered.connect(self.exit_requested.emit)
        menu.addAction(exit_action)
        
        self.tray_icon.setContextMenu(menu)

    def _on_activated(self, reason: QSystemTrayIcon.ActivationReason):
        """Handle tray icon activation.
        
        Left-click opens the main window.
        Right-click shows the context menu (handled by Qt).
        """
        if reason == QSystemTrayIcon.ActivationReason.Trigger:
            # Single left-click
            self.show_requested.emit()
        elif reason == QSystemTrayIcon.ActivationReason.DoubleClick:
            # Double-click also opens
            self.show_requested.emit()

    def set_icon(self, icon_path: str):
        """Set a custom icon for the tray.
        
        Args:
            icon_path: Path to an icon file (.ico or .png)
        """
        if os.path.exists(icon_path):
            icon = QIcon(icon_path)
            if not icon.isNull():
                self.tray_icon.setIcon(icon)

    def show_message(self, title: str, message: str, icon_type: QSystemTrayIcon.MessageIcon = QSystemTrayIcon.MessageIcon.Information):
        """Show a balloon notification from the tray icon.
        
        Args:
            title: Notification title
            message: Notification message
            icon_type: Icon to show in the notification
        """
        self.tray_icon.showMessage(title, message, icon_type, 3000)

    def hide(self):
        """Hide the tray icon."""
        self.tray_icon.hide()

    def show(self):
        """Show the tray icon."""
        self.tray_icon.show()
