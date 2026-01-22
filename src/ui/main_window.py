"""Main popup window for Shift Paste - Premium Overhaul."""

from PySide6.QtWidgets import (
    QWidget, QVBoxLayout, QLineEdit, QListWidget,
    QListWidgetItem, QLabel, QHBoxLayout, QStyledItemDelegate,
    QApplication, QFrame, QStyle
)
from PySide6.QtCore import Qt, Signal, QTimer, QSize, QEvent, QRect, QPoint
from PySide6.QtGui import QFont, QCursor, QPainter, QColor, QFontMetrics, QIcon, QPen, QKeySequence, QShortcut
from typing import List, Dict, Any
from datetime import datetime
from .styles import get_stylesheet


class ItemDelegate(QStyledItemDelegate):
    """Custom delegate for rendering clipboard and master items."""

    def __init__(self, parent=None):
        super().__init__(parent)
        self.item_height = 80
        self.icon_size = 20
        self.padding = 12

    def paint(self, painter: QPainter, option, index):
        painter.save()
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)

        item = index.data(Qt.ItemDataRole.UserRole)
        if not item:
            painter.restore()
            return

        is_selected = option.state & QStyle.StateFlag.State_Selected
        is_hovered = option.state & QStyle.StateFlag.State_MouseOver

        # Draw background
        rect = option.rect
        if is_selected:
            painter.fillRect(rect, QColor("#E3F2FD"))
        elif is_hovered:
            painter.fillRect(rect, QColor("#BBDEFB"))

        # Draw master item tint if applicable
        is_master = item.get('master_file_id') is not None
        if is_master and not is_selected:
            painter.fillRect(rect, QColor(255, 253, 231, 100)) # light yellow tint

        # Content area
        draw_rect = rect.adjusted(self.padding, self.padding, -self.padding, -self.padding)
        
        # 1. Draw Icon
        icon_rect = QRect(draw_rect.left(), draw_rect.top(), self.icon_size, self.icon_size)
        icon_text = "📌" if is_master else "📋"
        icon_font = QFont("Segoe UI Emoji", 12)
        painter.setFont(icon_font)
        painter.drawText(icon_rect, Qt.AlignmentFlag.AlignCenter, icon_text)

        # 2. Draw Title (First line)
        content = item.get('content', '').replace('\n', ' ').strip()
        title_text = content[:50]
        if len(content) > 50:
            title_text += "..."
            
        title_rect = QRect(icon_rect.right() + 8, draw_rect.top(), draw_rect.width() - 30, 20)
        title_font = QFont("Segoe UI", 10, QFont.Weight.Bold)
        painter.setFont(title_font)
        painter.setPen(QColor("#333333"))
        painter.drawText(title_rect, Qt.AlignmentFlag.AlignLeft | Qt.AlignmentFlag.AlignVCenter, title_text)

        # 3. Draw Subtitle (Second line)
        subtitle_text = content[50:150].strip()
        if subtitle_text:
            subtitle_rect = QRect(title_rect.left(), title_rect.bottom() + 2, title_rect.width(), 18)
            subtitle_font = QFont("Segoe UI", 9)
            painter.setFont(subtitle_font)
            painter.setPen(QColor("#666666"))
            painter.drawText(subtitle_rect, Qt.AlignmentFlag.AlignLeft | Qt.AlignmentFlag.AlignVCenter, subtitle_text)

        # 4. Draw Timestamp
        ts_val = item.get('last_copied_at') or item.get('master_modified') or datetime.now()
        # Relative time logic is handled by the search engine or we can do a simple one here
        # For simplicity, we'll assume the item has a 'time_ago' key if handled elsewhere, or just show it raw
        ts_text = item.get('time_ago', "Recently")
        
        ts_rect = QRect(title_rect.left(), draw_rect.bottom() - 15, title_rect.width(), 15)
        ts_font = QFont("Segoe UI", 8)
        ts_font.setItalic(True)
        painter.setFont(ts_font)
        painter.setPen(QColor("#999999"))
        painter.drawText(ts_rect, Qt.AlignmentFlag.AlignLeft | Qt.AlignmentFlag.AlignBottom, ts_text)

        painter.restore()

    def sizeHint(self, option, index):
        return QSize(400, self.item_height)


class MainWindow(QWidget):
    """Main popup window for Shift Paste."""

    item_selected = Signal(dict)
    search_changed = Signal(str)
    item_deleted = Signal(int)
    window_closed = Signal()
    settings_requested = Signal()
    clear_requested = Signal()

    def __init__(self):
        super().__init__()
        self.current_items: List[Dict[str, Any]] = []
        self._init_ui()
        self._setup_window()
        self._search_timer = QTimer(self)
        self._search_timer.setSingleShot(True)
        self._search_timer.timeout.connect(self._emit_search)
        self._setup_shortcuts()

    def _setup_shortcuts(self):
        """Setup global shortcuts for the window."""
        self.clear_shortcut = QShortcut(QKeySequence("Ctrl+Delete"), self)
        self.clear_shortcut.activated.connect(self.clear_requested.emit)
        
        self.settings_shortcut = QShortcut(QKeySequence("Ctrl+."), self)
        self.settings_shortcut.activated.connect(self.settings_requested.emit)

    def _init_ui(self):
        # Frameless window with rounded corners requires a container
        self.main_layout = QVBoxLayout(self)
        self.main_layout.setContentsMargins(0, 0, 0, 0)
        
        self.container = QFrame()
        self.container.setObjectName("MainWindowContainer")
        self.container_layout = QVBoxLayout(self.container)
        self.container_layout.setContentsMargins(0, 0, 0, 0)
        self.container_layout.setSpacing(0)

        # Header
        self.header = QWidget()
        self.header.setObjectName("HeaderWidget")
        self.header.setFixedHeight(60)
        header_layout = QHBoxLayout(self.header)
        header_layout.setContentsMargins(0, 0, 0, 0)

        self.search_input = QLineEdit()
        self.search_input.setObjectName("SearchInput")
        self.search_input.setPlaceholderText("🔍 Search clipboard and masters...")
        self.search_input.textChanged.connect(self._on_search_text_changed)
        self.search_input.returnPressed.connect(self._activate_current_item)
        header_layout.addWidget(self.search_input)
        
        # Settings button placeholder (can add icon later)
        self.settings_btn = QLabel("⚙️")
        self.settings_btn.setObjectName("SettingsButton")
        self.settings_btn.setMargin(15)
        self.settings_btn.setCursor(Qt.CursorShape.PointingHandCursor)
        self.settings_btn.installEventFilter(self)
        header_layout.addWidget(self.settings_btn)

        # List
        self.results_list = QListWidget()
        self.results_list.setObjectName("ResultsList")
        self.results_list.setItemDelegate(ItemDelegate(self.results_list))
        self.results_list.setMouseTracking(True)
        self.results_list.setVerticalScrollMode(QListWidget.ScrollMode.ScrollPerPixel)
        self.results_list.itemActivated.connect(self._on_item_activated)

        # Footer
        self.footer = QWidget()
        self.footer.setObjectName("FooterWidget")
        self.footer.setFixedHeight(35)
        footer_layout = QHBoxLayout(self.footer)
        footer_layout.setContentsMargins(15, 0, 15, 0)

        self.info_label = QLabel("Showing 0 items")
        self.info_label.setObjectName("FooterLabel")
        footer_layout.addWidget(self.info_label)
        
        footer_layout.addStretch()
        
        self.clear_btn = QLabel("[Clear All]")
        self.clear_btn.setObjectName("ClearButton")
        self.clear_btn.setCursor(Qt.CursorShape.PointingHandCursor)
        self.clear_btn.installEventFilter(self)
        footer_layout.addWidget(self.clear_btn)

        self.container_layout.addWidget(self.header)
        self.container_layout.addWidget(self.results_list)
        self.container_layout.addWidget(self.footer)
        
        self.main_layout.addWidget(self.container)
        self.setStyleSheet(get_stylesheet())

    def _setup_window(self):
        self.setWindowFlags(Qt.WindowType.FramelessWindowHint | Qt.WindowType.WindowStaysOnTopHint | Qt.WindowType.Tool)
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground)
        self.setFixedSize(400, 450)

    def show_near_cursor(self):
        # Clear search and reset list before showing
        self.search_input.clear()
        
        pos = QCursor.pos()
        screen = QApplication.primaryScreen().geometry()
        
        x = pos.x() - self.width() // 2
        y = pos.y() + 20
        
        # Bound to screen
        x = max(10, min(x, screen.width() - self.width() - 10))
        y = max(10, min(y, screen.height() - self.height() - 10))
        
        self.move(x, y)
        self.show()
        self.raise_()
        self.activateWindow()
        self.search_input.setFocus()

    def update_results(self, items: List[Dict[str, Any]]):
        self.current_items = items
        self.results_list.clear()
        for item in items:
            list_item = QListWidgetItem()
            list_item.setData(Qt.ItemDataRole.UserRole, item)
            self.results_list.addItem(list_item)
        
        self.info_label.setText(f"Showing {len(items)} items")
        if self.results_list.count() > 0:
            self.results_list.setCurrentRow(0)

    def _on_search_text_changed(self, text):
        # Debounce search (100ms)
        self._search_timer.start(100)

    def _emit_search(self):
        self.search_changed.emit(self.search_input.text())

    def _on_item_activated(self, item_widget):
        if not item_widget or not self.isVisible():
            return
        item = item_widget.data(Qt.ItemDataRole.UserRole)
        if item:
            self.item_selected.emit(item)
            self.close()

    def _activate_current_item(self):
        """Activate the currently selected item in the list."""
        if self.results_list.currentItem():
            self._on_item_activated(self.results_list.currentItem())

    def keyPressEvent(self, event):
        if event.key() == Qt.Key.Key_Escape:
            self.close()
        elif event.key() == Qt.Key.Key_Return or event.key() == Qt.Key.Key_Enter:
            self._activate_current_item()
            event.accept()
        elif event.key() == Qt.Key.Key_Delete:
            # Handle single item deletion if implemented in future
            pass
        else:
            super().keyPressEvent(event)

    def eventFilter(self, watched, event):
        if event.type() == QEvent.Type.MouseButtonPress:
            if watched == self.settings_btn:
                self.settings_requested.emit()
                return True
            elif watched == self.clear_btn:
                self.clear_requested.emit()
                return True
        return super().eventFilter(watched, event)

    def changeEvent(self, event):
        """Close window if it loses focus, unless a modal is open."""
        if event.type() == QEvent.Type.ActivationChange:
            if not self.isActiveWindow():
                # Check if we have a modal widget (like Settings) active
                if not QApplication.activeModalWidget():
                    self.close()
        super().changeEvent(event)

    def closeEvent(self, event):
        self.window_closed.emit()
        super().closeEvent(event)
