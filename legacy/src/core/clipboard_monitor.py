"""Clipboard monitoring thread for detecting text changes with security and platform context.

Security considerations:
- Excludes password manager content to prevent credential leakage
- Uses SHA-256 hashing for efficient change detection without storing raw content in memory
- Proper resource cleanup to prevent memory leaks
"""

import time
import hashlib
import platform
from typing import Optional, Tuple
from PySide6.QtCore import QThread, Signal

# Platform-specific imports with graceful degradation
_HAS_WIN32 = False
_HAS_PSUTIL = False

try:
    if platform.system() == "Windows":
        import win32gui
        import win32process
        import win32clipboard
        _HAS_WIN32 = True
        import psutil
        _HAS_PSUTIL = True
except ImportError as e:
    print(f"[WARN] Platform-specific dependency missing: {e}")


# Security: Known password manager processes to exclude from clipboard capture
# This prevents accidental credential leakage into clipboard history
EXCLUDED_PROCESSES = {
    'Windows': frozenset([
        'KeePass.exe', '1Password.exe', 'Bitwarden.exe', 'LastPass.exe',
        'KeePassXC.exe', 'Dashlane.exe', 'RoboForm.exe', 'Keeper.exe',
        'NordPass.exe', 'Enpass.exe'
    ]),
    'Darwin': frozenset([
        '1Password', 'KeePassXC', 'Bitwarden', 'Dashlane', 'LastPass'
    ]),
    'Linux': frozenset([
        'keepassxc', '1password', 'bitwarden', 'secret-tool'
    ])
}


class ClipboardMonitor(QThread):
    """Background thread that monitors clipboard for text changes with process awareness.
    
    Thread Safety:
    - Uses atomic flag for shutdown coordination
    - Emits signals for thread-safe communication with main thread
    
    Security:
    - Excludes password manager clipboard content
    - Uses content hashing to detect changes efficiently
    """

    # Signal: (plain_text, is_formatted, formatted_content)
    clipboard_changed = Signal(object, object, object)

    # Default poll interval in seconds - balanced for responsiveness vs CPU
    DEFAULT_POLL_INTERVAL = 0.4
    
    # Maximum content size to process (prevents memory issues with huge clipboard)
    MAX_CONTENT_SIZE = 1_000_000  # 1MB limit

    def __init__(self, poll_interval: float = DEFAULT_POLL_INTERVAL):
        """Initialize clipboard monitor.
        
        Args:
            poll_interval: Seconds between clipboard checks. Lower = more responsive
                          but higher CPU usage. Default 0.4s is a good balance.
        """
        super().__init__()
        self._running = False
        self._poll_interval = max(0.1, min(poll_interval, 5.0))  # Clamp to sane range
        self._last_hash = ""
        self._ignore_next = False
        self._system = platform.system()
        self.preserve_formatting = False
        
        # Cache the excluded process set for current platform
        self._excluded_processes = EXCLUDED_PROCESSES.get(self._system, frozenset())

    def get_active_process_name(self) -> str:
        """Get the name of the currently focused process.
        
        Returns:
            Process name or empty string if unavailable.
        """
        if not (_HAS_WIN32 and _HAS_PSUTIL) or self._system != "Windows":
            return ""
            
        try:
            hwnd = win32gui.GetForegroundWindow()
            if not hwnd:
                return ""
            _, pid = win32process.GetWindowThreadProcessId(hwnd)
            if pid <= 0:
                return ""
            process = psutil.Process(pid)
            return process.name()
        except (psutil.NoSuchProcess, psutil.AccessDenied, OSError):
            return ""
        except Exception:
            # Catch-all for unexpected platform errors
            return ""

    def is_password_manager_active(self) -> bool:
        """Check if an excluded password manager is currently focused.
        
        Security: This prevents capturing passwords copied from password managers.
        
        Returns:
            True if a password manager is the active window.
        """
        if not self._excluded_processes:
            return False
            
        active_proc = self.get_active_process_name()
        if not active_proc:
            return False
        
        active_lower = active_proc.lower()
        return any(pm.lower() in active_lower for pm in self._excluded_processes)

    def _get_clipboard_text(self) -> Optional[str]:
        """Get plain text from clipboard using multiple methods.
        
        Returns:
            Clipboard text or None if unavailable/empty.
        """
        try:
            import pyperclip
            text = pyperclip.paste()
            
            # Validate content
            if not text or not isinstance(text, str):
                return None
            
            # Security/Performance: Limit content size to prevent memory issues
            if len(text) > self.MAX_CONTENT_SIZE:
                return text[:self.MAX_CONTENT_SIZE]
                
            return text
        except Exception:
            return None

    def _get_formatted_content(self) -> Tuple[bool, Optional[str]]:
        """Get HTML formatted content from clipboard if available.
        
        Only called when preserve_formatting is enabled.
        
        Returns:
            Tuple of (is_formatted, formatted_content)
        """
        if not (_HAS_WIN32 and self._system == "Windows"):
            return False, None
            
        try:
            win32clipboard.OpenClipboard()
            try:
                html_format = win32clipboard.RegisterClipboardFormat("HTML Format")
                if win32clipboard.IsClipboardFormatAvailable(html_format):
                    data = win32clipboard.GetClipboardData(html_format)
                    if isinstance(data, bytes):
                        # Decode with error handling for malformed content
                        formatted = data.decode('utf-8', errors='replace')
                        return True, formatted
            finally:
                win32clipboard.CloseClipboard()
        except Exception:
            # Ensure clipboard is closed on error
            try:
                win32clipboard.CloseClipboard()
            except Exception:
                pass
        
        return False, None

    def get_clipboard_data(self) -> Tuple[Optional[str], bool, Optional[str]]:
        """Get plain text and optionally formatted text from clipboard.
        
        Returns:
            Tuple of (plain_text, is_formatted, formatted_content)
        """
        plain_text = self._get_clipboard_text()
        if not plain_text:
            return None, False, None
        
        is_formatted = False
        formatted_content = None
        
        if self.preserve_formatting:
            is_formatted, formatted_content = self._get_formatted_content()
        
        return plain_text, is_formatted, formatted_content

    def run(self):
        """Main monitoring loop.
        
        Thread-safe shutdown: Set _running to False to stop.
        """
        self._running = True
        
        while self._running:
            try:
                # Security: Skip password manager content
                if not self.is_password_manager_active():
                    plain_text, is_formatted, formatted_content = self.get_clipboard_data()
                    
                    if plain_text:
                        # Use SHA-256 for efficient change detection
                        current_hash = hashlib.sha256(
                            plain_text.encode('utf-8', errors='replace')
                        ).hexdigest()
                        
                        if current_hash != self._last_hash:
                            if self._ignore_next:
                                self._ignore_next = False
                            else:
                                self.clipboard_changed.emit(
                                    plain_text, is_formatted, formatted_content
                                )
                            
                            self._last_hash = current_hash
                
            except Exception as e:
                # Non-critical error - log and continue
                print(f"[WARN] Clipboard monitor error: {e}")

            # Interruptible sleep for responsive shutdown
            sleep_remaining = self._poll_interval
            while sleep_remaining > 0 and self._running:
                time.sleep(min(0.1, sleep_remaining))
                sleep_remaining -= 0.1

    def stop(self):
        """Stop the monitoring thread gracefully.
        
        Thread-safe: Can be called from any thread.
        """
        self._running = False
        # Wait for thread to finish with timeout
        if not self.wait(2000):  # 2 second timeout
            print("[WARN] Clipboard monitor thread did not stop gracefully")
            self.terminate()
            self.wait(1000)

    def ignore_next_change(self):
        """Signal to ignore the next detected change.
        
        Used when the app itself sets the clipboard to avoid
        re-adding our own paste to history.
        """
        self._ignore_next = True
