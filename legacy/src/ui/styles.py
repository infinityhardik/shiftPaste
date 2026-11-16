"""QSS stylesheets for Shift Paste UI.

Design philosophy:
- Modern, clean aesthetic inspired by Windows 11 Fluent Design
- High contrast for accessibility
- Subtle animations via hover states
- Consistent spacing using 8px grid

Color palette:
- Primary: #0078D4 (Windows blue)
- Surface: #FFFFFF / #F8F9FA
- Text: #333333 / #666666 / #999999
- Selection: #E3F2FD (light blue)
- Hover: #BBDEFB
- Master item tint: #FFFDE7 (light yellow)
"""


def get_stylesheet() -> str:
    """Get the main application stylesheet.
    
    Returns:
        Complete QSS stylesheet string for the main window
    """
    return """
        /* =============================================
           Main Window Container
           ============================================= */
        #MainWindowContainer {
            background-color: #ffffff;
            border: 1px solid #d0d0d0;
            border-radius: 12px;
        }

        /* =============================================
           Header Area
           ============================================= */
        #HeaderWidget {
            background-color: #ffffff;
            border-top-left-radius: 12px;
            border-top-right-radius: 12px;
            border-bottom: 1px solid #e8e8e8;
        }

        /* Search Input */
        QLineEdit#SearchInput {
            background-color: #f5f5f5;
            border: 2px solid transparent;
            border-radius: 8px;
            padding: 0px 12px;
            height: 40px;
            font-size: 11pt;
            font-family: 'Segoe UI', sans-serif;
            margin: 15px 10px 15px 15px;
            color: #333333;
            selection-background-color: #0078D4;
            selection-color: #ffffff;
        }

        QLineEdit#SearchInput:focus {
            border: 2px solid #0078D4;
            background-color: #ffffff;
        }

        QLineEdit#SearchInput:hover {
            background-color: #ebebeb;
        }

        /* Settings Button */
        #SettingsButton {
            font-size: 14pt;
            padding: 8px;
            border-radius: 6px;
        }

        #SettingsButton:hover {
            background-color: #e8e8e8;
        }

        /* =============================================
           Results List
           ============================================= */
        QListWidget#ResultsList {
            background-color: transparent;
            border: none;
            outline: none;
            padding: 4px 8px;
        }

        QListWidget#ResultsList::item {
            border-radius: 8px;
            margin: 3px 0px;
            padding: 2px;
        }

        QListWidget#ResultsList::item:hover {
            background-color: #EBF5FB;
        }

        QListWidget#ResultsList::item:selected {
            background-color: #D4E6F1;
            border: 1px solid #85C1E9;
        }

        QListWidget#ResultsList::item:selected:hover {
            background-color: #C9E2F5;
        }

        /* =============================================
           Footer
           ============================================= */
        #FooterWidget {
            background-color: #fafafa;
            border-bottom-left-radius: 12px;
            border-bottom-right-radius: 12px;
            border-top: 1px solid #e8e8e8;
        }

        #FooterLabel {
            color: #666666;
            font-size: 9pt;
            font-family: 'Segoe UI', sans-serif;
        }

        #ClearButton {
            color: #0078D4;
            font-size: 9pt;
            font-family: 'Segoe UI', sans-serif;
            padding: 4px 8px;
            border-radius: 4px;
        }

        #ClearButton:hover {
            background-color: #e8e8e8;
            color: #005a9e;
        }

        /* =============================================
           Scrollbar - Minimal Design
           ============================================= */
        QScrollBar:vertical {
            background-color: transparent;
            width: 8px;
            margin: 4px 2px;
        }

        QScrollBar::handle:vertical {
            background-color: #c0c0c0;
            border-radius: 4px;
            min-height: 24px;
        }

        QScrollBar::handle:vertical:hover {
            background-color: #a0a0a0;
        }

        QScrollBar::handle:vertical:pressed {
            background-color: #808080;
        }

        QScrollBar::add-line:vertical, 
        QScrollBar::sub-line:vertical {
            height: 0px;
            background: transparent;
        }

        QScrollBar::add-page:vertical, 
        QScrollBar::sub-page:vertical {
            background: transparent;
        }

        /* Horizontal scrollbar (if needed) */
        QScrollBar:horizontal {
            background-color: transparent;
            height: 8px;
            margin: 2px 4px;
        }

        QScrollBar::handle:horizontal {
            background-color: #c0c0c0;
            border-radius: 4px;
            min-width: 24px;
        }

        QScrollBar::handle:horizontal:hover {
            background-color: #a0a0a0;
        }

        QScrollBar::add-line:horizontal, 
        QScrollBar::sub-line:horizontal {
            width: 0px;
            background: transparent;
        }
    """


def get_settings_stylesheet() -> str:
    """Get the stylesheet for the settings window.
    
    Returns:
        Complete QSS stylesheet string for the settings dialog
    """
    return """
        QDialog {
            background-color: #f8f9fa;
        }

        QGroupBox {
            font-weight: bold;
            font-size: 10pt;
            border: 1px solid #e0e0e0;
            border-radius: 8px;
            margin-top: 12px;
            padding-top: 12px;
            background-color: #ffffff;
        }

        QGroupBox::title {
            subcontrol-origin: margin;
            left: 12px;
            padding: 0px 8px;
            color: #333333;
        }

        QLineEdit, QSpinBox, QComboBox {
            background-color: #ffffff;
            border: 1px solid #d0d0d0;
            border-radius: 6px;
            padding: 6px 10px;
            font-size: 10pt;
        }

        QLineEdit:focus, QSpinBox:focus, QComboBox:focus {
            border: 2px solid #0078D4;
        }

        QCheckBox {
            font-size: 10pt;
            spacing: 8px;
        }

        QCheckBox::indicator {
            width: 18px;
            height: 18px;
            border-radius: 4px;
            border: 1px solid #c0c0c0;
            background-color: #ffffff;
        }

        QCheckBox::indicator:checked {
            background-color: #0078D4;
            border-color: #0078D4;
        }

        QListWidget {
            background-color: #ffffff;
            border: 1px solid #e0e0e0;
            border-radius: 6px;
        }

        QPushButton {
            background-color: #f0f0f0;
            border: 1px solid #d0d0d0;
            border-radius: 6px;
            padding: 8px 16px;
            font-size: 10pt;
        }

        QPushButton:hover {
            background-color: #e0e0e0;
        }

        QPushButton:pressed {
            background-color: #d0d0d0;
        }

        QScrollArea {
            background-color: transparent;
            border: none;
        }
    """
