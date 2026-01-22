"""SQLite database operations for clipboard and master items.

Design decisions:
- Uses SQLite for reliability and zero-configuration deployment
- Row factory for dict-like access to query results
- Connection pooling via check_same_thread=False (safe for this use case)
- Comprehensive migration support for schema evolution
- Uses proper user data directory for portable deployments

Security:
- All queries use parameterized statements (no SQL injection)
- Clipboard content is stored as-is (no sanitization needed for storage)
"""

import sqlite3
import hashlib
from datetime import datetime
from typing import List, Optional, Dict, Any, Tuple
from pathlib import Path

# Import path utilities for proper database location resolution
try:
    from src.utils.paths import get_database_path, migrate_old_data
except ImportError:
    # Fallback for direct module testing
    from utils.paths import get_database_path, migrate_old_data


class Database:
    """SQLite database handler for clipboard and master items.
    
    Thread Safety:
    - check_same_thread=False allows multi-thread access
    - Individual operations are atomic via SQLite's locking
    - For bulk operations, use transactions explicitly
    """
    
    # Current schema version - bump when making schema changes
    SCHEMA_VERSION = 2
    
    # Use proper user data directory for database
    DEFAULT_DB_PATH = None  # Will be resolved dynamically

    def __init__(self, db_path: Optional[str] = None):
        """Initialize database connection and create schema if needed.
        
        Args:
            db_path: Path to SQLite database file. If None, uses the proper
                     user data directory location.
        """
        # Migrate old data from previous installations before setting up
        migrate_old_data()
        
        # Use provided path or resolve proper user data location
        if db_path:
            self.db_path = db_path
        else:
            self.db_path = str(get_database_path())
        
        print(f"[*] Using database: {self.db_path}")
        self.conn: Optional[sqlite3.Connection] = None
        self._init_database()

    def _init_database(self):
        """Initialize database with schema according to specifications."""
        # Create parent directory if needed
        try:
            Path(self.db_path).parent.mkdir(parents=True, exist_ok=True)
        except OSError as e:
            print(f"[WARN] Could not create database directory: {e}")
        
        try:
            self.conn = sqlite3.connect(self.db_path, check_same_thread=False)
            self.conn.row_factory = sqlite3.Row
            
            # Enable foreign keys for referential integrity
            self.conn.execute("PRAGMA foreign_keys = ON")
            
            self._create_tables()
            self._run_migrations()
            
        except sqlite3.Error as e:
            print(f"[ERROR] Database initialization failed: {e}")
            raise

    def _create_tables(self):
        """Create database tables if they don't exist."""
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
        
        # Index for hash lookups (deduplication)
        cursor.execute("""
            CREATE INDEX IF NOT EXISTS idx_clipboard_hash 
            ON clipboard_items(content_hash)
        """)
        
        # Index for recency queries
        cursor.execute("""
            CREATE INDEX IF NOT EXISTS idx_clipboard_recency 
            ON clipboard_items(last_copied_at DESC)
        """)

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
                master_file_id INTEGER NOT NULL,
                content TEXT NOT NULL,
                row_number INTEGER,
                FOREIGN KEY (master_file_id) REFERENCES master_files(id) ON DELETE CASCADE
            )
        """)
        
        # Index for master file lookups
        cursor.execute("""
            CREATE INDEX IF NOT EXISTS idx_master_items_file 
            ON master_items(master_file_id)
        """)

        # User settings key-value store
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )
        """)
        
        self.conn.commit()

    def _run_migrations(self):
        """Run any needed schema migrations."""
        cursor = self.conn.cursor()
        
        # Get current columns in clipboard_items
        cursor.execute("PRAGMA table_info(clipboard_items)")
        existing_columns = {row[1].lower() for row in cursor.fetchall()}
        
        # Define needed columns and their types
        needed_columns = {
            'content_hash': "TEXT DEFAULT ''",
            'created_at': "TIMESTAMP DEFAULT CURRENT_TIMESTAMP",
            'last_copied_at': "TIMESTAMP DEFAULT CURRENT_TIMESTAMP",
            'is_formatted': "INTEGER DEFAULT 0",
            'formatted_content': "TEXT NULL",
            'copy_count': "INTEGER DEFAULT 1"
        }
        
        # Add any missing columns
        for col_name, col_type in needed_columns.items():
            if col_name.lower() not in existing_columns:
                try:
                    print(f"[*] Migration: Adding column '{col_name}' to clipboard_items")
                    cursor.execute(f"ALTER TABLE clipboard_items ADD COLUMN {col_name} {col_type}")
                except sqlite3.OperationalError:
                    pass  # Column might already exist
        
        # Populate content_hash for any rows that don't have it
        cursor.execute("""
            SELECT id, content FROM clipboard_items 
            WHERE content_hash IS NULL OR content_hash = ''
        """)
        rows = cursor.fetchall()
        for row in rows:
            content_hash = self._get_hash(row['content'])
            try:
                cursor.execute(
                    "UPDATE clipboard_items SET content_hash = ? WHERE id = ?",
                    (content_hash, row['id'])
                )
            except sqlite3.IntegrityError:
                # Duplicate hash - delete this row
                cursor.execute("DELETE FROM clipboard_items WHERE id = ?", (row['id'],))
        
        self.conn.commit()

    def _get_hash(self, text: str) -> str:
        """Calculate SHA256 hash for content deduplication.
        
        Args:
            text: Content to hash
            
        Returns:
            Hex-encoded SHA256 hash
        """
        if not text:
            text = ""
        return hashlib.sha256(text.encode('utf-8', errors='replace')).hexdigest()

    def add_clipboard_item(
        self, 
        content: str, 
        is_formatted: bool = False, 
        formatted_content: Optional[str] = None
    ) -> int:
        """Add new clipboard item or update existing one if hash matches.
        
        Deduplication: If content hash already exists, updates last_copied_at
        and increments copy_count instead of creating a duplicate.
        
        Args:
            content: Plain text content
            is_formatted: Whether formatted content is available
            formatted_content: Optional HTML/RTF formatted content
            
        Returns:
            Item ID (new or existing)
        """
        if not content:
            return -1
            
        content_hash = self._get_hash(content)
        cursor = self.conn.cursor()
        now = datetime.now()

        try:
            # Try to insert new item
            cursor.execute("""
                INSERT INTO clipboard_items 
                (content, content_hash, is_formatted, formatted_content, created_at, last_copied_at)
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
            row = cursor.fetchone()
            item_id = row['id'] if row else -1

        self.conn.commit()
        return item_id

    def get_setting(self, key: str, default: Any = None) -> Any:
        """Get a setting from the database.
        
        Args:
            key: Setting key
            default: Default value if key not found
            
        Returns:
            Setting value or default
        """
        try:
            cursor = self.conn.cursor()
            cursor.execute("SELECT value FROM settings WHERE key = ?", (key,))
            row = cursor.fetchone()
            return row['value'] if row else default
        except sqlite3.Error:
            return default

    def set_setting(self, key: str, value: Any):
        """Set a setting in the database.
        
        Args:
            key: Setting key
            value: Setting value (will be converted to string)
        """
        try:
            cursor = self.conn.cursor()
            cursor.execute("""
                INSERT INTO settings (key, value) VALUES (?, ?)
                ON CONFLICT(key) DO UPDATE SET value = excluded.value
            """, (key, str(value)))
            self.conn.commit()
        except sqlite3.Error as e:
            print(f"[WARN] Could not save setting '{key}': {e}")

    def add_master_file(self, file_path: str) -> int:
        """Add or get a master file record.
        
        Args:
            file_path: Absolute path to the Excel file
            
        Returns:
            File ID
        """
        cursor = self.conn.cursor()
        try:
            cursor.execute("INSERT INTO master_files (file_path) VALUES (?)", (file_path,))
            self.conn.commit()
            return cursor.lastrowid
        except sqlite3.IntegrityError:
            cursor.execute("SELECT id FROM master_files WHERE file_path = ?", (file_path,))
            row = cursor.fetchone()
            return row['id'] if row else -1

    def delete_master_file(self, file_id: int):
        """Delete a master file and its items (via cascade).
        
        Args:
            file_id: ID of the master file to delete
        """
        try:
            cursor = self.conn.cursor()
            cursor.execute("DELETE FROM master_files WHERE id = ?", (file_id,))
            self.conn.commit()
        except sqlite3.Error as e:
            print(f"[WARN] Could not delete master file: {e}")

    def update_master_items(self, file_id: int, items: List[Tuple[str, int]]):
        """Rebuild items for a specific master file.
        
        Uses a transaction for atomicity - if any error occurs,
        the entire update is rolled back.
        
        Args:
            file_id: ID of the master file
            items: List of (content, row_number) tuples
        """
        try:
            # Use connection context manager for automatic transaction handling
            with self.conn:
                cursor = self.conn.cursor()
                
                # Clear existing items
                cursor.execute("DELETE FROM master_items WHERE master_file_id = ?", (file_id,))
                
                # Insert new items in batches for performance
                batch_size = 100
                for i in range(0, len(items), batch_size):
                    batch = items[i:i + batch_size]
                    cursor.executemany("""
                        INSERT INTO master_items (master_file_id, content, row_number)
                        VALUES (?, ?, ?)
                    """, [(file_id, content, row_num) for content, row_num in batch])
                
                # Update modification timestamp
                cursor.execute(
                    "UPDATE master_files SET last_modified = ?, last_error = NULL WHERE id = ?",
                    (datetime.now(), file_id)
                )
            
        except sqlite3.Error as e:
            print(f"[ERROR] Could not update master items: {e}")

    def get_recent_items(self, limit: int = 20) -> List[Dict[str, Any]]:
        """Get the most recent clipboard history items.
        
        Args:
            limit: Maximum number of items to return
            
        Returns:
            List of item dictionaries
        """
        try:
            cursor = self.conn.cursor()
            cursor.execute("""
                SELECT *, 'clipboard' as source_table 
                FROM clipboard_items 
                ORDER BY last_copied_at DESC 
                LIMIT ?
            """, (limit,))
            return [dict(row) for row in cursor.fetchall()]
        except sqlite3.Error:
            return []

    def get_all_master_items(self) -> List[Dict[str, Any]]:
        """Get all items from enabled master files.
        
        Returns:
            List of item dictionaries with file metadata
        """
        try:
            cursor = self.conn.cursor()
            cursor.execute("""
                SELECT mi.*, mf.file_path, mf.last_modified as master_modified, 'master' as source_table
                FROM master_items mi
                JOIN master_files mf ON mi.master_file_id = mf.id
                WHERE mf.is_enabled = 1
            """)
            return [dict(row) for row in cursor.fetchall()]
        except sqlite3.Error:
            return []

    def search_clipboard(self, query: str, limit: int = 50) -> List[Dict[str, Any]]:
        """Search clipboard items using pattern matching.
        
        Uses LIKE with wildcards between characters for fuzzy matching.
        The actual ranking is done by the search engine.
        
        Args:
            query: Search query
            limit: Maximum results to return
            
        Returns:
            List of matching items
        """
        try:
            cursor = self.conn.cursor()
            
            if not query.strip():
                cursor.execute("""
                    SELECT * FROM clipboard_items 
                    ORDER BY last_copied_at DESC LIMIT ?
                """, (limit,))
            else:
                # Build LIKE pattern with wildcards between each character
                clean_query = query.replace(' ', '').lower()
                pattern = "%" + "%".join(list(clean_query)) + "%"
                
                cursor.execute("""
                    SELECT * FROM clipboard_items 
                    WHERE LOWER(content) LIKE ? 
                    ORDER BY last_copied_at DESC 
                    LIMIT ?
                """, (pattern, limit * 2))  # Fetch extra for ranking
                
            return [dict(row) for row in cursor.fetchall()]
        except sqlite3.Error:
            return []

    def search_masters(self, query: str, limit: int = 50) -> List[Dict[str, Any]]:
        """Search master items using pattern matching.
        
        Args:
            query: Search query
            limit: Maximum results to return
            
        Returns:
            List of matching items with file metadata
        """
        try:
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
                clean_query = query.replace(' ', '').lower()
                pattern = "%" + "%".join(list(clean_query)) + "%"
                
                cursor.execute("""
                    SELECT mi.*, mf.file_path, mf.last_modified as master_modified 
                    FROM master_items mi
                    JOIN master_files mf ON mi.master_file_id = mf.id
                    WHERE mf.is_enabled = 1 AND LOWER(mi.content) LIKE ?
                    LIMIT ?
                """, (pattern, limit * 2))
                
            return [dict(row) for row in cursor.fetchall()]
        except sqlite3.Error:
            return []

    def delete_clipboard_item(self, item_id: int):
        """Delete a single clipboard item.
        
        Args:
            item_id: ID of the item to delete
        """
        try:
            cursor = self.conn.cursor()
            cursor.execute("DELETE FROM clipboard_items WHERE id = ?", (item_id,))
            self.conn.commit()
        except sqlite3.Error as e:
            print(f"[WARN] Could not delete clipboard item: {e}")

    def clear_clipboard_history(self):
        """Clear all clipboard history (keeps master items)."""
        try:
            cursor = self.conn.cursor()
            cursor.execute("DELETE FROM clipboard_items")
            self.conn.commit()
            
            # VACUUM must run outside of a transaction
            # Temporarily set isolation_level to None for autocommit mode
            old_isolation = self.conn.isolation_level
            self.conn.isolation_level = None
            try:
                cursor.execute("VACUUM")
            finally:
                self.conn.isolation_level = old_isolation
                
        except sqlite3.Error as e:
            print(f"[WARN] Could not clear clipboard history: {e}")

    def close(self):
        """Close database connection gracefully."""
        if self.conn:
            try:
                self.conn.close()
            except sqlite3.Error:
                pass
            finally:
                self.conn = None

    def __del__(self):
        """Destructor to ensure connection is closed."""
        self.close()
