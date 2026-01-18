"""Main popup window for Shift Paste - Windows 10 Clipboard style."""

from PySide6.QtWidgets import (
    QWidget, QVBoxLayout, QLineEdit, QListWidget,
    QListWidgetItem, QLabel, QHBoxLayout
)
from PySide6.QtCore import Qt, Signal, QTimer, QSize, QEvent, QCoreApplication
from PySide6.QtGui import QFont, QCursor, QKeyEvent
from PySide6.QtWidgets import QApplication
from typing import List, Dict, Any
from datetime import datetime
from .styles import get_stylesheet


class ClipboardItemWidget(QWidget):
    """Custom widget for displaying clipboard/master items."""

    def __init__(self, item: Dict[str, Any], preview_chars: int = 100):
        """Initialize item widget.

        Args:
            item: Item dictionary with content, timestamp, etc.
            preview_chars: Max characters to show in preview
        """
        super().__init__()
        self.item = item
        self.preview_chars = preview_chars
        self._init_ui()

    def _init_ui(self):
        """Initialize UI components."""
        layout = QVBoxLayout()
        layout.setContentsMargins(12, 10, 12, 10)
        layout.setSpacing(6)

        # Content preview
        content = self.item.get('content', '')
        if len(content) > self.preview_chars:
            content = content[:self.preview_chars] + "..."

        content_label = QLabel(content)
        content_label.setWordWrap(True)
        font = QFont("Segoe UI", 10)
        content_label.setFont(font)

        # Metadata row
        meta_layout = QHBoxLayout()
        meta_layout.setContentsMargins(0, 0, 0, 0)

        # Timestamp
        timestamp = self.item.get('timestamp', 0)
        time_diff = self._get_time_ago(timestamp)
        time_label = QLabel(f"🕐 {time_diff}")
        time_label.setStyleSheet("color: #888; font-size: 9pt;")
        meta_layout.addWidget(time_label)

        # Category (if master item)
        if self.item.get('source_table') == 'master':
            category = self.item.get('category', '')
            category_label = QLabel(f"📁 {category}")
            category_label.setStyleSheet("color: #0078d4; font-size: 9pt; font-weight: 600;")
            meta_layout.addWidget(category_label)

        meta_layout.addStretch()

        layout.addWidget(content_label)
        layout.addLayout(meta_layout)

        self.setLayout(layout)

    def _get_time_ago(self, timestamp: int) -> str:
        """Convert timestamp to 'time ago' string.

        Args:
            timestamp: Unix timestamp

        Returns:
            Human-readable time string
        """
        if not timestamp:
            return "Unknown"

        now = datetime.now()
        item_time = datetime.fromtimestamp(timestamp)
        diff = now - item_time

        seconds = diff.total_seconds()
        if seconds < 60:
            return "Just now"
        elif seconds < 3600:
            mins = int(seconds / 60)
            return f"{mins} min{'s' if mins > 1 else ''} ago"
        elif seconds < 86400:
            hours = int(seconds / 3600)
            return f"{hours} hour{'s' if hours > 1 else ''} ago"
        else:
            days = int(seconds / 86400)
            if days == 1:
                return "Yesterday"
            elif days < 7:
                return f"{days} days ago"
            else:
                return item_time.strftime("%b %d, %Y")

    def sizeHint(self) -> QSize:
        """Return suggested widget size."""
        return QSize(400, 70)


class MainWindow(QWidget):
    """Main popup window - Windows 10 Clipboard style."""

    item_selected = Signal(dict)  # Emits selected item
    search_changed = Signal(str)  # Emits search query
    item_deleted = Signal(int)    # Emits deleted item ID
    window_closed = Signal()

    def __init__(self, preview_chars: int = 100):
        """Initialize main window.

        Args:
            preview_chars: Max characters to show in item previews
        """
        super().__init__()
        self.preview_chars = preview_chars
        self.current_items: List[Dict[str, Any]] = []
        self.is_visible = False  # Track visibility to prevent early close
        self._init_ui()
        self._setup_window()
        # Install a global event filter to detect clicks outside the window
        app = QCoreApplication.instance()
        if app:
            app.installEventFilter(self)

    def _init_ui(self):
        """Initialize UI components."""
        layout = QVBoxLayout()
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(0)

        # Search input
        self.search_input = QLineEdit()
        self.search_input.setPlaceholderText("🔍 Search clipboard and master items...")
        self.search_input.setFont(QFont("Segoe UI", 11))
        self.search_input.textChanged.connect(self._on_search_changed)
        self.search_input.returnPressed.connect(self._on_enter_pressed)

        # Results list
        self.results_list = QListWidget()
        self.results_list.setVerticalScrollBarPolicy(Qt.ScrollBarAsNeeded)
        self.results_list.setHorizontalScrollBarPolicy(Qt.ScrollBarAlwaysOff)
        self.results_list.itemActivated.connect(self._on_item_activated)
        self.results_list.itemDoubleClicked.connect(self._on_item_activated)

        # Footer hints
        hints_layout = QHBoxLayout()
        hints_layout.setContentsMargins(12, 8, 12, 8)

        hints_label = QLabel("⌨️ Enter=Paste  Del=Remove  Esc=Close")
        hints_label.setStyleSheet("color: #888; font-size: 9pt;")
        hints_layout.addWidget(hints_label)
        hints_layout.addStretch()

        layout.addWidget(self.search_input)
        layout.addWidget(self.results_list)
        layout.addLayout(hints_layout)

        self.setLayout(layout)
        self.setStyleSheet(get_stylesheet())

    def _setup_window(self):
        """Configure window properties."""
        # Use Window instead of Tool to allow taskbar and focus
        self.setWindowFlags(
            Qt.FramelessWindowHint |
            Qt.WindowStaysOnTopHint |
            Qt.Window
        )
        self.setAttribute(Qt.WA_ShowWithoutActivating, False)
        self.setFixedSize(450, 400)

    def show_near_cursor(self):
        """Show window near cursor position and bring to foreground."""
        cursor_pos = QCursor.pos()
        x = cursor_pos.x() + 10
        y = cursor_pos.y() + 20

        # Keep on screen
        from PySide6.QtWidgets import QApplication
        screen = QApplication.primaryScreen().geometry()
        if x + self.width() > screen.width():
            x = screen.width() - self.width() - 10
        if y + self.height() > screen.height():
            y = screen.height() - self.height() - 10
        self.move(x, y)

        # Restore window if minimized or hidden
        if self.isMinimized() or not self.isVisible():
            self.show()
        else:
            self.showNormal()

        # Mark visible then use a short delayed activation to avoid
        # focus races with global hotkeys or the taskbar.
        self.is_visible = True

        # Show immediately then finalize activation shortly after
        self.show()
        self.setWindowState(self.windowState() & ~Qt.WindowMinimized)
        self.raise_()

        def _finalize():
            try:
                self.activateWindow()
                # Force window to foreground on Windows
                import ctypes
                import sys
                if sys.platform == "win32":
                    try:
                        hwnd = int(self.winId())
                        ctypes.windll.user32.ShowWindow(hwnd, 5)  # SW_SHOW
                        # Use SetWindowPos topmost toggle to work around
                        # Windows foreground restrictions.
                        SWP_NOSIZE = 0x0001
                        SWP_NOMOVE = 0x0002
                        HWND_TOPMOST = -1
                        HWND_NOTOPMOST = -2
                        ctypes.windll.user32.SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE)
                        ctypes.windll.user32.SetWindowPos(hwnd, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE)
                        ctypes.windll.user32.SetForegroundWindow(hwnd)
                    except Exception:
                        pass
            except Exception:
                pass

            # Ensure focus goes to the search input
            self.setFocus()
            self.search_input.setFocus()
            self.search_input.clear()

        # Small delay lets OS finish processing input (modifier keys, clicks)
        QTimer.singleShot(60, _finalize)

    def eventFilter(self, obj, event):
        """Global event filter to detect clicks outside this window.

        When the user clicks anywhere outside while the popup is visible,
        simulate an Escape key press so the UI closes consistently.
        """
        # Detect global mouse presses and close immediately if click is outside
        if event.type() == QEvent.MouseButtonPress and self.is_visible:
            try:
                pos = event.globalPos()
                # First try widgetAt which is more reliable for complex setups
                clicked_widget = QApplication.widgetAt(pos)
                inside = False
                if clicked_widget:
                    # If the clicked widget is this window or a child, treat as inside
                    if clicked_widget is self or self.isAncestorOf(clicked_widget):
                        inside = True

                if not inside:
                    # Close immediately and emit close signal
                    try:
                        self.close()
                    except Exception:
                        pass
                    self.is_visible = False
                    try:
                        self.window_closed.emit()
                    except Exception:
                        pass
            except Exception:
                pass

        return super().eventFilter(obj, event)

    def closeEvent(self, event):
        """Cleanup when window closes: remove event filter."""
        try:
            app = QCoreApplication.instance()
            if app:
                try:
                    app.removeEventFilter(self)
                except Exception:
                    pass
        except Exception:
            pass

        self.is_visible = False
        try:
            self.window_closed.emit()
        except Exception:
            pass

        super().closeEvent(event)

    def update_results(self, items: List[Dict[str, Any]]):
        """Update results list.

        Args:
            items: List of items to display
        """
        self.current_items = items
        self.results_list.clear()

        for item in items:
            list_item = QListWidgetItem()
            widget = ClipboardItemWidget(item, self.preview_chars)

            list_item.setSizeHint(widget.sizeHint())
            self.results_list.addItem(list_item)
            self.results_list.setItemWidget(list_item, widget)

        # Select first item
        if self.results_list.count() > 0:
            self.results_list.setCurrentRow(0)

    def _on_search_changed(self, text: str):
        """Handle search input changes.

        Args:
            text: Search query text
        """
        self.search_changed.emit(text)

    def _on_enter_pressed(self):
        """Handle Enter key - paste mode."""
        current_item = self.results_list.currentItem()
        if current_item:
            row = self.results_list.row(current_item)
            if row < len(self.current_items):
                self.item_selected.emit(self.current_items[row])

    def _on_item_activated(self, item: QListWidgetItem):
        """Handle item activation (double-click or Enter).

        Args:
            item: Activated list item
        """
        row = self.results_list.row(item)
        if row < len(self.current_items):
            self.is_visible = False
            self.item_selected.emit(self.current_items[row])

    def keyPressEvent(self, event):
        """Handle keyboard shortcuts.

        Args:
            event: Key press event
        """
        key = event.key()

        if key == Qt.Key_Escape:
            self.is_visible = False
            self.close()
            self.window_closed.emit()

        elif key == Qt.Key_Delete:
            # Delete selected clipboard item
            current_item = self.results_list.currentItem()
            if current_item:
                row = self.results_list.row(current_item)
                if row < len(self.current_items):
                    item = self.current_items[row]
                    if item.get('source_table') == 'clipboard':
                        # Emit delete so the DB is updated
                        self.item_deleted.emit(item['id'])
                        # Remove from display and from internal list
                        self.results_list.takeItem(row)
                        try:
                            self.current_items.pop(row)
                        except Exception:
                            pass

                        # Select next item
                        if row < len(self.current_items):
                            self.results_list.setCurrentRow(row)
                        elif row > 0:
                            self.results_list.setCurrentRow(row - 1)

        elif key == Qt.Key_Up:
            # Arrow up: move to previous item
            current_row = self.results_list.currentRow()
            if current_row > 0:
                self.results_list.setCurrentRow(current_row - 1)
            event.accept()

        elif key == Qt.Key_Down:
            # Arrow down: move to next item
            current_row = self.results_list.currentRow()
            if current_row < self.results_list.count() - 1:
                self.results_list.setCurrentRow(current_row + 1)
            event.accept()

        else:
            super().keyPressEvent(event)

    def focusOutEvent(self, event):
        """Close when focus is lost (click outside).

        Args:
            event: Focus out event
        """
        # Only close if window is actually visible (not during initialization).
        # Use a short delayed check to avoid races right after show(),
        # e.g. when global hotkey modifiers are still held or taskbar
        # activation is racing with focus events.
        if self.is_visible:
            def _delayed_close():
                # If window no longer has focus and no child has focus, close.
                active = self.isActiveWindow() or self.search_input.hasFocus()
                if not active and self.is_visible:
                    self.close()
                    self.is_visible = False
                    self.window_closed.emit()

            QTimer.singleShot(80, _delayed_close)

        super().focusOutEvent(event)
