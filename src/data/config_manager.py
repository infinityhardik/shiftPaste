"""Configuration management for Shift Paste."""

import json
from pathlib import Path
from typing import Any, Dict


DEFAULT_CONFIG = {
    "clipboard": {
        "max_items": 20,           # 10-500
        "preview_chars": 100        # 50-200
    },
    "shortcuts": {
        "windows": "ctrl+shift+v",
        "macos": "shift+cmd+v",
        "linux": "shift+super+v"
    },
    "master_file": {
        "directory": "data/Master",
        "auto_reload": True,
        "default_categories": ["Pinned", "Work", "Personal"]
    },
    "ui": {
        "theme": "system",          # system/light/dark
        "max_visible_items": 8,     # 5-20
        "window_width": 450,
        "window_height": 400
    },
    "startup": {
        "run_on_boot": False
    }
}


class ConfigManager:
    """Manages application configuration with JSON persistence."""

    def __init__(self, config_path: str = "config.json"):
        """Initialize config manager.

        Args:
            config_path: Path to configuration JSON file
        """
        self.config_path = Path(config_path)
        self.config: Dict[str, Any] = {}
        self.load()

    def load(self):
        """Load configuration from file or create default."""
        if self.config_path.exists():
            try:
                with open(self.config_path, 'r', encoding='utf-8') as f:
                    user_config = json.load(f)
                self.config = self._merge_configs(DEFAULT_CONFIG, user_config)
            except Exception as e:
                print(f"Error loading config: {e}")
                self.config = DEFAULT_CONFIG.copy()
        else:
            self.config = DEFAULT_CONFIG.copy()
            self.save()

    def save(self):
        """Save configuration to file."""
        try:
            self.config_path.parent.mkdir(parents=True, exist_ok=True)
            with open(self.config_path, 'w', encoding='utf-8') as f:
                json.dump(self.config, f, indent=2, ensure_ascii=False)
        except Exception as e:
            print(f"Error saving config: {e}")

    def get(self, key_path: str, default=None) -> Any:
        """Get config value using dot notation.

        Args:
            key_path: Dot-separated path (e.g., 'clipboard.max_items')
            default: Default value if key not found

        Returns:
            Configuration value or default
        """
        keys = key_path.split('.')
        value = self.config

        for key in keys:
            if isinstance(value, dict) and key in value:
                value = value[key]
            else:
                return default

        return value

    def set(self, key_path: str, value: Any):
        """Set config value using dot notation.

        Args:
            key_path: Dot-separated path (e.g., 'clipboard.max_items')
            value: Value to set
        """
        keys = key_path.split('.')
        config = self.config

        for key in keys[:-1]:
            if key not in config:
                config[key] = {}
            config = config[key]

        config[keys[-1]] = value
        self.save()

    def _merge_configs(self, default: Dict, user: Dict) -> Dict:
        """Recursively merge user config with defaults.

        Args:
            default: Default configuration
            user: User configuration

        Returns:
            Merged configuration
        """
        merged = default.copy()

        for key, value in user.items():
            if key in merged and isinstance(merged[key], dict) and isinstance(value, dict):
                merged[key] = self._merge_configs(merged[key], value)
            else:
                merged[key] = value

        return merged

    def get_all(self) -> Dict[str, Any]:
        """Get entire configuration dictionary.

        Returns:
            Complete configuration
        """
        return self.config.copy()
