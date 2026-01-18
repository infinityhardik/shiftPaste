"""Clipboard monitoring thread for detecting text changes."""

from PySide6.QtCore import QThread, Signal
import pyperclip
import time
from typing import Optional


class ClipboardMonitor(QThread):
    """Background thread that monitors clipboard for text changes."""

    clipboard_changed = Signal(str)  # Emits new clipboard content

    def __init__(self, poll_interval: float = 0.2):
        """Initialize clipboard monitor.

        Args:
            poll_interval: Time between clipboard checks in seconds
        """
        super().__init__()
        self.running = False
        self.last_content = ""
        self.poll_interval = poll_interval

    def run(self):
        """Main monitoring loop."""
        self.running = True

        while self.running:
            try:
                current_content = pyperclip.paste()

                # Only emit if content actually changed
                if current_content and current_content != self.last_content:
                    self.last_content = current_content
                    self.clipboard_changed.emit(current_content)

            except Exception as e:
                print(f"Clipboard monitor error: {e}")

            time.sleep(self.poll_interval)

    def stop(self):
        """Stop the monitoring thread."""
        self.running = False
