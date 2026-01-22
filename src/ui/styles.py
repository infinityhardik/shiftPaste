"""QSS stylesheets for Shift Paste UI."""

def get_stylesheet() -> str:
    """Get the main application stylesheet."""
    return """
        /* Main window container */
        #MainWindowContainer {
            background-color: #ffffff;
            border: 1px solid #d0d0d0;
            border-radius: 12px;
        }

        /* Header area */
        #HeaderWidget {
            background-color: #ffffff;
            border-top-left-radius: 12px;
            border-top-right-radius: 12px;
        }

        /* Search input */
        QLineEdit#SearchInput {
            background-color: #f8f9fa;
            border: 1px solid #e0e0e0;
            border-radius: 8px;
            padding: 10px 12px;
            font-size: 11pt;
            margin: 10px 12px;
            color: #333333;
        }

        QLineEdit#SearchInput:focus {
            border: 2px solid #0078d4;
            background-color: #ffffff;
        }

        /* Results list */
        QListWidget#ResultsList {
            background-color: transparent;
            border: none;
            outline: none;
            padding: 0px 8px;
        }

        QListWidget#ResultsList::item {
            border-radius: 8px;
            margin: 4px 0px;
        }

        QListWidget#ResultsList::item:hover {
            background-color: #BBDEFB; /* Light blue hover */
        }

        QListWidget#ResultsList::item:selected {
            background-color: #E3F2FD; /* Selected blue */
            color: #000000;
        }

        /* Footer */
        #FooterWidget {
            background-color: #f8f9fa;
            border-bottom-left-radius: 12px;
            border-bottom-right-radius: 12px;
            border-top: 1px solid #eee;
        }

        #FooterLabel {
            color: #888;
            font-size: 9pt;
        }

        /* Scrollbar */
        QScrollBar:vertical {
            background-color: transparent;
            width: 8px;
            margin: 2px 0px;
        }

        QScrollBar::handle:vertical {
            background-color: #d0d0d0;
            border-radius: 4px;
            min-height: 20px;
        }

        QScrollBar::handle:vertical:hover {
            background-color: #999999;
        }

        QScrollBar::add-line:vertical, QScrollBar::sub-line:vertical {
            height: 0px;
        }
    """
