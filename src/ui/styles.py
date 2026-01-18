"""QSS stylesheets for Shift Paste UI."""


def get_stylesheet() -> str:
    """Get the main application stylesheet.

    Returns:
        QSS stylesheet string
    """
    return """
        /* Main window */
        QWidget {
            background-color: #ffffff;
            color: #000000;
        }

        /* Search input */
        QLineEdit {
            background-color: #f5f5f5;
            border: 1px solid #e0e0e0;
            border-radius: 4px;
            padding: 10px 12px;
            font-size: 11pt;
            margin: 8px;
            selection-background-color: #0078d4;
        }

        QLineEdit:focus {
            border: 2px solid #0078d4;
            margin: 7px;
            outline: none;
        }

        /* Results list */
        QListWidget {
            background-color: #ffffff;
            border: none;
            outline: none;
        }

        QListWidget::item {
            padding: 0px;
            margin: 2px 4px;
            border-radius: 4px;
        }

        QListWidget::item:hover {
            background-color: #f5f5f5;
        }

        QListWidget::item:selected {
            background-color: #e5f3ff;
            border-left: 3px solid #0078d4;
        }

        /* Scrollbar */
        QScrollBar:vertical {
            background-color: #f5f5f5;
            width: 10px;
            border: none;
            margin: 0px;
        }

        QScrollBar::handle:vertical {
            background-color: #d0d0d0;
            border-radius: 5px;
            min-height: 20px;
            margin: 0px 2px;
        }

        QScrollBar::handle:vertical:hover {
            background-color: #999999;
        }

        QScrollBar::add-line:vertical {
            height: 0px;
        }

        QScrollBar::sub-line:vertical {
            height: 0px;
        }

        /* Labels */
        QLabel {
            color: #000000;
        }

        QLabel[type="meta"] {
            color: #888888;
            font-size: 9pt;
        }

        QLabel[type="category"] {
            color: #0078d4;
            font-size: 9pt;
            font-weight: 600;
        }

        /* Dialog */
        QDialog {
            background-color: #ffffff;
            color: #000000;
        }

        /* Buttons */
        QPushButton {
            background-color: #0078d4;
            color: #ffffff;
            border: none;
            border-radius: 4px;
            padding: 6px 12px;
            font-size: 10pt;
            font-weight: 500;
        }

        QPushButton:hover {
            background-color: #106ebe;
        }

        QPushButton:pressed {
            background-color: #005a9e;
        }

        QPushButton:disabled {
            background-color: #d0d0d0;
            color: #888888;
        }

        /* Settings */
        QGroupBox {
            color: #000000;
            border: 1px solid #e0e0e0;
            border-radius: 4px;
            padding-top: 10px;
            margin-top: 10px;
        }

        QGroupBox::title {
            subcontrol-origin: margin;
            subcontrol-position: top left;
            padding: 0px 3px;
        }

        QCheckBox {
            color: #000000;
            spacing: 5px;
        }

        QCheckBox::indicator {
            width: 16px;
            height: 16px;
        }

        QCheckBox::indicator:unchecked {
            background-color: #ffffff;
            border: 1px solid #d0d0d0;
            border-radius: 2px;
        }

        QCheckBox::indicator:checked {
            background-color: #0078d4;
            border: 1px solid #0078d4;
            border-radius: 2px;
        }

        QComboBox {
            background-color: #ffffff;
            border: 1px solid #d0d0d0;
            border-radius: 4px;
            padding: 5px 8px;
            color: #000000;
        }

        QComboBox::drop-down {
            border: none;
        }

        QComboBox::down-arrow {
            image: none;
            width: 10px;
            height: 10px;
        }

        /* Menu */
        QMenu {
            background-color: #ffffff;
            color: #000000;
            border: 1px solid #d0d0d0;
            border-radius: 4px;
            padding: 4px 0px;
        }

        QMenu::item:selected {
            background-color: #f0f0f0;
        }

        QMenu::item:pressed {
            background-color: #e0e0e0;
        }

        QMenu::separator {
            background-color: #e0e0e0;
            height: 1px;
            margin: 4px 0px;
        }
    """


def get_dark_stylesheet() -> str:
    """Get dark theme stylesheet.

    Returns:
        QSS stylesheet string for dark theme
    """
    return """
        /* Main window */
        QWidget {
            background-color: #1e1e1e;
            color: #ffffff;
        }

        /* Search input */
        QLineEdit {
            background-color: #2d2d2d;
            border: 1px solid #3d3d3d;
            border-radius: 4px;
            padding: 10px 12px;
            font-size: 11pt;
            margin: 8px;
            color: #ffffff;
            selection-background-color: #0078d4;
        }

        QLineEdit:focus {
            border: 2px solid #0078d4;
            margin: 7px;
            outline: none;
        }

        /* Results list */
        QListWidget {
            background-color: #1e1e1e;
            border: none;
            outline: none;
        }

        QListWidget::item {
            padding: 0px;
            margin: 2px 4px;
            border-radius: 4px;
        }

        QListWidget::item:hover {
            background-color: #2d2d2d;
        }

        QListWidget::item:selected {
            background-color: #1a3a52;
            border-left: 3px solid #0078d4;
        }

        /* Scrollbar */
        QScrollBar:vertical {
            background-color: #2d2d2d;
            width: 10px;
            border: none;
        }

        QScrollBar::handle:vertical {
            background-color: #555555;
            border-radius: 5px;
            min-height: 20px;
            margin: 0px 2px;
        }

        QScrollBar::handle:vertical:hover {
            background-color: #777777;
        }
    """
