"""Main popup window for Shift Paste.

UI Design:
- Frameless window with rounded corners
- Appears near cursor position
- Closes on focus loss or Escape key
- Keyboard navigation with arrow keys and Enter

Accessibility:
- High contrast text
- Visual focus indicators
- Keyboard-first navigation
"""

from PySide6.QtWidgets import (
    QWidget, QVBoxLayout, QLineEdit, QListWidget,
    QListWidgetItem, QLabel, QHBoxLayout, QStyledItemDelegate,
    QApplication, QFrame, QStyle
)
from PySide6.QtCore import Qt, Signal, QTimer, QSize, QEvent, QRect
from PySide6.QtGui import (
    QFont, QCursor, QPainter, QColor, QKeySequence, QShortcut,
    QPen, QBrush
)
from typing import List, Dict, Any, Optional
from .styles import get_stylesheet


class ItemDelegate(QStyledItemDelegate):
    """Custom delegate for rendering clipboard and master items.
    
    Visual design:
    - Icon indicates item type (clipboard vs master)
    - Title shows first line truncated
    - Subtitle shows additional context
    - Timestamp shows relative time
    """
    
    ITEM_HEIGHT = 76
    ICON_SIZE = 20
    PADDING = 12
    
    # Colors
    COLOR_SELECTED = QColor("#D4E6F1")
    COLOR_HOVER = QColor("#EBF5FB")
    COLOR_MASTER_TINT = QColor(255, 253, 231, 80)
    COLOR_TITLE = QColor("#333333")
    COLOR_SUBTITLE = QColor("#666666")
    COLOR_TIMESTAMP = QColor("#999999")
    COLOR_SELECTION_BORDER = QColor("#85C1E9")

    def __init__(self, parent=None):
        super().__init__(parent)
        
        # Font cache for performance
        self._title_font = QFont("Segoe UI", 10, QFont.Weight.DemiBold)
        self._subtitle_font = QFont("Segoe UI", 9)
        self._timestamp_font = QFont("Segoe UI", 8)
        self._timestamp_font.setItalic(True)
        self._icon_font = QFont("Segoe UI Emoji", 11)

    def paint(self, painter: QPainter, option, index):
        """Paint a single list item."""
        painter.save()
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)

        item = index.data(Qt.ItemDataRole.UserRole)
        if not item:
            painter.restore()
            return

        is_selected = bool(option.state & QStyle.StateFlag.State_Selected)
        is_hovered = bool(option.state & QStyle.StateFlag.State_MouseOver)
        is_master = item.get('master_file_id') is not None

        rect = option.rect
        
        # Draw background based on state
        if is_selected:
            painter.fillRect(rect, self.COLOR_SELECTED)
            # Draw selection border
            pen = QPen(self.COLOR_SELECTION_BORDER)
            pen.setWidth(1)
            painter.setPen(pen)
            painter.drawRoundedRect(rect.adjusted(1, 1, -1, -1), 6, 6)
        elif is_hovered:
            painter.fillRect(rect, self.COLOR_HOVER)
        
        # Master item tint (yellow background)
        if is_master and not is_selected:
            painter.fillRect(rect, self.COLOR_MASTER_TINT)

        # Content area with padding
        draw_rect = rect.adjusted(self.PADDING, self.PADDING, -self.PADDING, -self.PADDING)
        
        # Draw icon
        icon_rect = QRect(draw_rect.left(), draw_rect.top(), self.ICON_SIZE, self.ICON_SIZE)
        icon_text = "📌" if is_master else "📋"
        painter.setFont(self._icon_font)
        painter.setPen(self.COLOR_TITLE)
        painter.drawText(icon_rect, Qt.AlignmentFlag.AlignCenter, icon_text)

        # Calculate text area
        text_left = icon_rect.right() + 10
        text_width = draw_rect.width() - self.ICON_SIZE - 10

        # Get and process content
        content = item.get('content', '')
        # Replace newlines and multiple spaces for display
        content = ' '.join(content.split())
        
        # Draw title (first ~50 chars)
        title_text = content[:50]
        if len(content) > 50:
            title_text += "..."
            
        title_rect = QRect(text_left, draw_rect.top(), text_width, 20)
        painter.setFont(self._title_font)
        painter.setPen(self.COLOR_TITLE)
        
        # Elide text if too long
        fm = painter.fontMetrics()
        elided_title = fm.elidedText(title_text, Qt.TextElideMode.ElideRight, text_width)
        painter.drawText(title_rect, Qt.AlignmentFlag.AlignLeft | Qt.AlignmentFlag.AlignVCenter, elided_title)

        # Draw subtitle (next ~100 chars)
        if len(content) > 50:
            subtitle_text = content[50:150].strip()
            if len(content) > 150:
                subtitle_text += "..."
            
            subtitle_rect = QRect(text_left, title_rect.bottom() + 2, text_width, 18)
            painter.setFont(self._subtitle_font)
            painter.setPen(self.COLOR_SUBTITLE)
            
            elided_subtitle = fm.elidedText(subtitle_text, Qt.TextElideMode.ElideRight, text_width)
            painter.drawText(subtitle_rect, Qt.AlignmentFlag.AlignLeft | Qt.AlignmentFlag.AlignVCenter, elided_subtitle)

        # Draw timestamp
        ts_text = item.get('time_ago', "")
        if ts_text:
            ts_rect = QRect(text_left, draw_rect.bottom() - 15, text_width, 15)
            painter.setFont(self._timestamp_font)
            painter.setPen(self.COLOR_TIMESTAMP)
            painter.drawText(ts_rect, Qt.AlignmentFlag.AlignLeft | Qt.AlignmentFlag.AlignBottom, ts_text)

        painter.restore()

    def sizeHint(self, option, index) -> QSize:
        """Return the size hint for an item."""
        return QSize(400, self.ITEM_HEIGHT)


class MainWindow(QWidget):
    """Main popup window for Shift Paste.
    
    Signals:
        item_selected: Emitted when user selects an item to paste
        search_changed: Emitted when search text changes (debounced)
        item_deleted: Emitted to request item deletion
        window_closed: Emitted when window is closed
        settings_requested: Emitted to open settings
        clear_requested: Emitted to clear history
    """

    item_selected = Signal(dict)
    search_changed = Signal(str)
    item_deleted = Signal(int)
    window_closed = Signal()
    settings_requested = Signal()
    clear_requested = Signal()
    
    # Window dimensions
    WINDOW_WIDTH = 420
    WINDOW_HEIGHT = 480
    
    # Search debounce delay in milliseconds
    SEARCH_DEBOUNCE_MS = 80

    def __init__(self):
        super().__init__()
        self.current_items: List[Dict[str, Any]] = []
        self._init_ui()
        self._setup_window()
        self._setup_search_timer()
        self._setup_shortcuts()

    def _init_ui(self):
        """Initialize UI components."""
        # Main layout without margins (content goes in container)
        self.main_layout = QVBoxLayout(self)
        self.main_layout.setContentsMargins(0, 0, 0, 0)
        
        # Container for rounded corners with translucent parent
        self.container = QFrame()
        self.container.setObjectName("MainWindowContainer")
        self.container_layout = QVBoxLayout(self.container)
        self.container_layout.setContentsMargins(0, 0, 0, 0)
        self.container_layout.setSpacing(0)

        # Header with search bar
        self._init_header()
        
        # Results list
        self._init_results_list()
        
        # Footer with item count and clear button
        self._init_footer()
        
        self.main_layout.addWidget(self.container)
        self.setStyleSheet(get_stylesheet())

    def _init_header(self):
        """Initialize header with search input and settings button."""
        self.header = QWidget()
        self.header.setObjectName("HeaderWidget")
        self.header.setFixedHeight(70)
        
        header_layout = QHBoxLayout(self.header)
        header_layout.setContentsMargins(0, 0, 0, 0)
        header_layout.setSpacing(0)

        # Search input
        self.search_input = QLineEdit()
        self.search_input.setObjectName("SearchInput")
        self.search_input.setPlaceholderText("🔍 Search clipboard and masters...")
        self.search_input.textChanged.connect(self._on_search_text_changed)
        self.search_input.returnPressed.connect(self._activate_current_item)
        header_layout.addWidget(self.search_input, 1)
        
        # Settings button
        self.settings_btn = QLabel("⚙️")
        self.settings_btn.setObjectName("SettingsButton")
        self.settings_btn.setFixedSize(45, 45)
        self.settings_btn.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.settings_btn.setCursor(Qt.CursorShape.PointingHandCursor)
        self.settings_btn.installEventFilter(self)
        header_layout.addWidget(self.settings_btn)
        
        self.container_layout.addWidget(self.header)

    def _init_results_list(self):
        """Initialize the results list widget."""
        self.results_list = QListWidget()
        self.results_list.setObjectName("ResultsList")
        self.results_list.setItemDelegate(ItemDelegate(self.results_list))
        self.results_list.setMouseTracking(True)
        self.results_list.setVerticalScrollMode(QListWidget.ScrollMode.ScrollPerPixel)
        self.results_list.setHorizontalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAlwaysOff)
        self.results_list.itemActivated.connect(self._on_item_activated)
        self.results_list.itemClicked.connect(self._on_item_activated)
        
        self.container_layout.addWidget(self.results_list, 1)

    def _init_footer(self):
        """Initialize footer with item count and clear button."""
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
        
        self.container_layout.addWidget(self.footer)

    def _setup_window(self):
        """Configure window flags and attributes."""
        self.setWindowFlags(
            Qt.WindowType.FramelessWindowHint | 
            Qt.WindowType.WindowStaysOnTopHint | 
            Qt.WindowType.Tool
        )
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground)
        self.setAttribute(Qt.WidgetAttribute.WA_ShowWithoutActivating, False)
        self.setFixedSize(self.WINDOW_WIDTH, self.WINDOW_HEIGHT)

    def _setup_search_timer(self):
        """Setup debounced search timer."""
        self._search_timer = QTimer(self)
        self._search_timer.setSingleShot(True)
        self._search_timer.timeout.connect(self._emit_search)

    def _setup_shortcuts(self):
        """Setup keyboard shortcuts."""
        # Clear history shortcut
        self.clear_shortcut = QShortcut(QKeySequence("Ctrl+Delete"), self)
        self.clear_shortcut.activated.connect(self.clear_requested.emit)
        
        # Settings shortcut
        self.settings_shortcut = QShortcut(QKeySequence("Ctrl+,"), self)
        self.settings_shortcut.activated.connect(self.settings_requested.emit)

    def show_near_cursor(self):
        """Show the window near the current cursor position."""
        # Clear search and reset selection
        self.search_input.clear()
        
        # Position near cursor
        pos = QCursor.pos()
        screen = QApplication.primaryScreen()
        if screen:
            screen_rect = screen.availableGeometry()
        else:
            screen_rect = QApplication.desktop().availableGeometry() if hasattr(QApplication, 'desktop') else QRect(0, 0, 1920, 1080)
        
        # Center horizontally around cursor, below it vertically
        x = pos.x() - self.width() // 2
        y = pos.y() + 15
        
        # Keep within screen bounds with margin
        margin = 10
        x = max(margin, min(x, screen_rect.right() - self.width() - margin))
        y = max(margin, min(y, screen_rect.bottom() - self.height() - margin))
        
        # If window would be too close to bottom, show above cursor instead
        if y + self.height() > screen_rect.bottom() - margin:
            y = pos.y() - self.height() - 15
            y = max(margin, y)
        
        self.move(x, y)
        self.show()
        self.raise_()
        self.activateWindow()
        self.search_input.setFocus()

    def update_results(self, items: List[Dict[str, Any]]):
        """Update the results list with new items.
        
        Args:
            items: List of item dictionaries to display
        """
        self.current_items = items
        self.results_list.clear()
        
        for item in items:
            list_item = QListWidgetItem()
            list_item.setData(Qt.ItemDataRole.UserRole, item)
            self.results_list.addItem(list_item)
        
        count = len(items)
        self.info_label.setText(f"Showing {count} item{'s' if count != 1 else ''}")
        
        # Select first item
        if count > 0:
            self.results_list.setCurrentRow(0)

    def _on_search_text_changed(self, text: str):
        """Handle search text changes with debouncing."""
        self._search_timer.start(self.SEARCH_DEBOUNCE_MS)

    def _emit_search(self):
        """Emit the search signal after debounce delay."""
        self.search_changed.emit(self.search_input.text())

    def _on_item_activated(self, item_widget: Optional[QListWidgetItem]):
        """Handle item activation (double-click or Enter)."""
        if not item_widget or not self.isVisible():
            return
            
        item = item_widget.data(Qt.ItemDataRole.UserRole)
        if item:
            self.item_selected.emit(item)
            self.close()

    def _activate_current_item(self):
        """Activate the currently selected item."""
        current = self.results_list.currentItem()
        if current:
            self._on_item_activated(current)

    def keyPressEvent(self, event):
        """Handle key press events."""
        key = event.key()
        
        if key == Qt.Key.Key_Escape:
            self.close()
        elif key in (Qt.Key.Key_Return, Qt.Key.Key_Enter):
            self._activate_current_item()
            event.accept()
        elif key == Qt.Key.Key_Down:
            # Move selection down if in search field
            if self.search_input.hasFocus():
                current = self.results_list.currentRow()
                if current < self.results_list.count() - 1:
                    self.results_list.setCurrentRow(current + 1)
                event.accept()
            else:
                super().keyPressEvent(event)
        elif key == Qt.Key.Key_Up:
            # Move selection up if in search field
            if self.search_input.hasFocus():
                current = self.results_list.currentRow()
                if current > 0:
                    self.results_list.setCurrentRow(current - 1)
                event.accept()
            else:
                super().keyPressEvent(event)
        else:
            super().keyPressEvent(event)

    def eventFilter(self, watched, event) -> bool:
        """Handle events for settings and clear buttons."""
        if event.type() == QEvent.Type.MouseButtonPress:
            if watched == self.settings_btn:
                self.settings_requested.emit()
                return True
            elif watched == self.clear_btn:
                self.clear_requested.emit()
                return True
        return super().eventFilter(watched, event)

    def changeEvent(self, event):
        """Close window on focus loss."""
        if event.type() == QEvent.Type.ActivationChange:
            if not self.isActiveWindow():
                # Don't close if a modal dialog is open
                if not QApplication.activeModalWidget():
                    self.close()
        super().changeEvent(event)

    def closeEvent(self, event):
        """Handle window close."""
        self.window_closed.emit()
        super().closeEvent(event)
