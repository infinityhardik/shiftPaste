"""SQLite database operations for clipboard and master items."""

import sqlite3
import hashlib
from datetime import datetime
import math
from typing import List, Optional, Dict, Any, Tuple
from pathlib import Path


class Database:
    """SQLite database handler for clipboard and master items."""

    def __init__(self, db_path: str = "data/clipboard.db"):
        """Initialize database connection and create schema if needed.
        
        Args:
            db_path: Path to SQLite database file
        """
        self.db_path = db_path
        self.conn = None
        self.init_database()

    def init_database(self):
        """Initialize database with schema according to specifications."""
        # Create parent directory if needed
        Path(self.db_path).parent.mkdir(parents=True, exist_ok=True)
        
        self.conn = sqlite3.connect(self.db_path, check_same_thread=False)
        self.conn.row_factory = sqlite3.Row
        
        cursor = self.conn.cursor()

        # Clipboard history table
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS clipboard_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content TEXT NOT NULL,
                content_hash TEXT UNIQUE NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                last_copied_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                copy_count INTEGER DEFAULT 1,
                is_formatted INTEGER DEFAULT 0,
                formatted_content TEXT NULL
            )
        """)

        # Migration: Ensure all columns from current specification exist
        try:
            # Method 1: Check via description
            cursor.execute("SELECT * FROM clipboard_items LIMIT 0")
            existing_columns = [description[0].lower() for description in cursor.description]
            
            # Method 2: Fallback/Verify via PRAGMA
            cursor.execute("PRAGMA table_info(clipboard_items)")
            pragma_cols = [row[1].lower() for row in cursor.fetchall()]
            
            # Merge both just in case
            all_found_cols = set(existing_columns) | set(pragma_cols)
            
            needed_columns = {
                'content_hash': "TEXT",
                'created_at': "TIMESTAMP DEFAULT CURRENT_TIMESTAMP",
                'last_copied_at': "TIMESTAMP DEFAULT CURRENT_TIMESTAMP",
                'is_formatted': "INTEGER DEFAULT 0",
                'formatted_content': "TEXT NULL",
                'copy_count': "INTEGER DEFAULT 1"
            }
            
            for col, col_type in needed_columns.items():
                if col.lower() not in all_found_cols:
                    print(f"[*] MIGRATION: Adding missing column '{col}' to clipboard_items...")
                    cursor.execute(f"ALTER TABLE clipboard_items ADD COLUMN {col} {col_type}")
                    if col == 'content_hash':
                        cursor.execute("CREATE UNIQUE INDEX IF NOT EXISTS idx_clipboard_hash ON clipboard_items(content_hash)")
            
        except sqlite3.OperationalError as e:
            print(f"[!] Database initialization message: {e}")
            pass

        # Master files configuration
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS master_files (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_path TEXT UNIQUE NOT NULL,
                is_enabled INTEGER DEFAULT 1,
                last_modified TIMESTAMP,
                last_error TEXT NULL
            )
        """)

        # Master items cache
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS master_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                master_file_id INTEGER,
                content TEXT NOT NULL,
                row_number INTEGER,
                FOREIGN KEY (master_file_id) REFERENCES master_files(id) ON DELETE CASCADE
            )
        """)

        # User settings
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )
        """)

        cursor.execute("PRAGMA foreign_keys = ON")
        self.conn.commit()

    def _get_hash(self, text: str) -> str:
        """Calculate SHA256 hash for content deduplication."""
        return hashlib.sha256(text.encode('utf-8')).hexdigest()

    def add_clipboard_item(self, content: str, is_formatted: bool = False, formatted_content: str = None) -> int:
        """Add new clipboard item or update existing one if hash matches.
        
        Returns:
            Item ID
        """
        content_hash = self._get_hash(content)
        cursor = self.conn.cursor()
        now = datetime.now()

        try:
            # Try to insert new item
            cursor.execute("""
                INSERT INTO clipboard_items (content, content_hash, is_formatted, formatted_content, created_at, last_copied_at)
                VALUES (?, ?, ?, ?, ?, ?)
            """, (content, content_hash, 1 if is_formatted else 0, formatted_content, now, now))
            
            item_id = cursor.lastrowid
            
        except sqlite3.IntegrityError:
            # Duplicate hash - update existing record
            cursor.execute("""
                UPDATE clipboard_items 
                SET last_copied_at = ?, copy_count = copy_count + 1
                WHERE content_hash = ?
            """, (now, content_hash))
            
            cursor.execute("SELECT id FROM clipboard_items WHERE content_hash = ?", (content_hash,))
            item_id = cursor.fetchone()['id']

        self.conn.commit()
        return item_id

    def get_setting(self, key: str, default: Any = None) -> Any:
        """Get a setting from the database."""
        cursor = self.conn.cursor()
        cursor.execute("SELECT value FROM settings WHERE key = ?", (key,))
        row = cursor.fetchone()
        return row['value'] if row else default

    def set_setting(self, key: str, value: Any):
        """Set a setting in the database."""
        cursor = self.conn.cursor()
        cursor.execute("""
            INSERT INTO settings (key, value) VALUES (?, ?)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value
        """, (key, str(value)))
        self.conn.commit()

    def add_master_file(self, file_path: str) -> int:
        """Add a master file record."""
        cursor = self.conn.cursor()
        try:
            cursor.execute("INSERT INTO master_files (file_path) VALUES (?)", (file_path,))
            self.conn.commit()
            return cursor.lastrowid
        except sqlite3.IntegrityError:
            cursor.execute("SELECT id FROM master_files WHERE file_path = ?", (file_path,))
            return cursor.fetchone()['id']

    def delete_master_file(self, file_id: int):
        """Delete a master file and its items (via cascade)."""
        cursor = self.conn.cursor()
        cursor.execute("DELETE FROM master_files WHERE id = ?", (file_id,))
        self.conn.commit()

    def update_master_items(self, file_id: int, items: List[Tuple[str, int]]):
        """Rebuild items for a specific master file."""
        cursor = self.conn.cursor()
        
        # Clear from cache table
        cursor.execute("DELETE FROM master_items WHERE master_file_id = ?", (file_id,))
        
        # Insert new items
        for content, row_num in items:
            cursor.execute("""
                INSERT INTO master_items (master_file_id, content, row_number)
                VALUES (?, ?, ?)
            """, (file_id, content, row_num))

        cursor.execute("UPDATE master_files SET last_modified = ? WHERE id = ?", (datetime.now(), file_id))
        self.conn.commit()

    def get_recent_items(self, limit: int = 20) -> List[Dict[str, Any]]:
        """Get the most recent clipboard history items."""
        cursor = self.conn.cursor()
        cursor.execute("""
            SELECT *, 'clipboard' as source_table 
            FROM clipboard_items 
            ORDER BY last_copied_at DESC 
            LIMIT ?
        """, (limit,))
        return [dict(row) for row in cursor.fetchall()]

    def get_all_master_items(self) -> List[Dict[str, Any]]:
        """Get all items from enabled master files."""
        cursor = self.conn.cursor()
        cursor.execute("""
            SELECT mi.*, mf.file_path, mf.last_modified as master_modified, 'master' as source_table
            FROM master_items mi
            JOIN master_files mf ON mi.master_file_id = mf.id
            WHERE mf.is_enabled = 1
        """)
        return [dict(row) for row in cursor.fetchall()]

    def search_clipboard(self, query: str, limit: int = 50) -> List[Dict[str, Any]]:
        """Search clipboard items. Using loose search to let Python engine rank."""
        cursor = self.conn.cursor()
        
        if not query.strip():
            cursor.execute("""
                SELECT * FROM clipboard_items 
                ORDER BY last_copied_at DESC LIMIT ?
            """, (limit,))
        else:
            # Broad search: any item containing characters from query
            # We'll fetch more than requested to give the fuzzy engine more candidates
            search_query = f"%{query.replace(' ', '%')}%"
            cursor.execute("""
                SELECT * FROM clipboard_items 
                WHERE content LIKE ? 
                ORDER BY last_copied_at DESC 
                LIMIT ?
            """, (search_query, limit * 2))
            
        return [dict(row) for row in cursor.fetchall()]

    def search_masters(self, query: str, limit: int = 50) -> List[Dict[str, Any]]:
        """Search master items. Using loose search to let Python engine rank."""
        cursor = self.conn.cursor()
        
        if not query.strip():
            cursor.execute("""
                SELECT mi.*, mf.file_path, mf.last_modified as master_modified 
                FROM master_items mi
                JOIN master_files mf ON mi.master_file_id = mf.id
                WHERE mf.is_enabled = 1
                LIMIT ?
            """, (limit,))
        else:
            search_query = f"%{query.replace(' ', '%')}%"
            cursor.execute("""
                SELECT mi.*, mf.file_path, mf.last_modified as master_modified 
                FROM master_items mi
                JOIN master_files mf ON mi.master_file_id = mf.id
                WHERE mf.is_enabled = 1 AND mi.content LIKE ?
                LIMIT ?
            """, (search_query, limit * 2))
            
        return [dict(row) for row in cursor.fetchall()]

    def delete_clipboard_item(self, item_id: int):
        """Delete clipboard item."""
        cursor = self.conn.cursor()
        cursor.execute("DELETE FROM clipboard_items WHERE id = ?", (item_id,))
        self.conn.commit()

    def clear_clipboard_history(self):
        """Clear all clipboard history."""
        cursor = self.conn.cursor()
        cursor.execute("DELETE FROM clipboard_items")
        self.conn.commit()

    def close(self):
        """Close database connection."""
        if self.conn:
            self.conn.close()
