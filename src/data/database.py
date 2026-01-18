"""SQLite database operations for clipboard and master items."""

import sqlite3
from datetime import datetime
from typing import List, Optional, Dict, Any
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
        """Initialize database with schema."""
        # Create parent directory if needed
        Path(self.db_path).parent.mkdir(parents=True, exist_ok=True)
        
        self.conn = sqlite3.connect(self.db_path, check_same_thread=False)
        self.conn.row_factory = sqlite3.Row
        
        cursor = self.conn.cursor()

        # Clipboard items table
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS clipboard_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                source_app TEXT
            )
        """)

        # Master items table
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS master_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content TEXT NOT NULL,
                category TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                notes TEXT,
                is_active BOOLEAN DEFAULT 1
            )
        """)

        # Search index with FTS5
        cursor.execute("""
            CREATE VIRTUAL TABLE IF NOT EXISTS search_index 
            USING fts5(
                content,
                source_table,
                source_id,
                tokenize = 'porter unicode61'
            )
        """)

        # Indexes
        cursor.execute("""
            CREATE INDEX IF NOT EXISTS idx_clipboard_timestamp 
            ON clipboard_items(timestamp DESC)
        """)

        cursor.execute("""
            CREATE INDEX IF NOT EXISTS idx_master_category 
            ON master_items(category, timestamp DESC)
        """)

        self.conn.commit()

    def add_clipboard_item(self, content: str, source_app: Optional[str] = None) -> int:
        """Add new clipboard item.
        
        Args:
            content: Text content to add
            source_app: Optional source application name
            
        Returns:
            Item ID or -1 if duplicate
        """
        timestamp = int(datetime.now().timestamp())
        cursor = self.conn.cursor()

        # Check for duplicates (don't add if same as last item)
        cursor.execute("""
            SELECT content FROM clipboard_items 
            ORDER BY timestamp DESC LIMIT 1
        """)
        last_row = cursor.fetchone()
        if last_row and last_row['content'] == content:
            return -1  # Skip duplicate

        cursor.execute("""
            INSERT INTO clipboard_items (content, timestamp, source_app)
            VALUES (?, ?, ?)
        """, (content, timestamp, source_app))

        item_id = cursor.lastrowid

        # Add to search index
        cursor.execute("""
            INSERT INTO search_index (content, source_table, source_id)
            VALUES (?, 'clipboard', ?)
        """, (content, item_id))

        self.conn.commit()
        return item_id

    def get_recent_items(self, limit: int = 20) -> List[Dict[str, Any]]:
        """Get recent clipboard items.
        
        Args:
            limit: Maximum number of items to return
            
        Returns:
            List of recent clipboard items
        """
        cursor = self.conn.cursor()
        cursor.execute("""
            SELECT *, 'clipboard' as source_table FROM clipboard_items 
            ORDER BY timestamp DESC 
            LIMIT ?
        """, (limit,))

        return [dict(row) for row in cursor.fetchall()]

    def get_all_master_items(self) -> List[Dict[str, Any]]:
        """Get all active master items.
        
        Returns:
            List of master items sorted by category and recency
        """
        cursor = self.conn.cursor()
        cursor.execute("""
            SELECT *, 'master' as source_table FROM master_items 
            WHERE is_active = 1 
            ORDER BY category, timestamp DESC
        """)

        return [dict(row) for row in cursor.fetchall()]

    def search_all_items(self, query: str, max_results: int = 50) -> List[Dict[str, Any]]:
        """Search both clipboard and master items using FTS5.
        
        Args:
            query: Search query string
            max_results: Maximum results to return
            
        Returns:
            List of matching items
        """
        if not query.strip():
            return []

        cursor = self.conn.cursor()

        # Prepare query for FTS5 - use OR to match any term
        terms = query.split()
        if not terms:
            return []
        
        # Build FTS5 query: term1 OR term2 OR term3
        fts_query = ' OR '.join([f'"{term}"' for term in terms if term])

        try:
            cursor.execute("""
                SELECT 
                    si.source_table,
                    si.source_id,
                    si.content,
                    CASE 
                        WHEN si.source_table = 'clipboard' THEN ci.timestamp
                        WHEN si.source_table = 'master' THEN mi.timestamp
                    END as timestamp,
                    CASE 
                        WHEN si.source_table = 'master' THEN mi.category
                        ELSE NULL
                    END as category,
                    CASE 
                        WHEN si.source_table = 'master' THEN mi.notes
                        ELSE NULL
                    END as notes,
                    CASE 
                        WHEN si.source_table = 'clipboard' THEN ci.source_app
                        ELSE NULL
                    END as source_app
                FROM search_index si
                LEFT JOIN clipboard_items ci ON si.source_table = 'clipboard' AND si.source_id = ci.id
                LEFT JOIN master_items mi ON si.source_table = 'master' AND si.source_id = mi.id
                WHERE search_index MATCH ?
                LIMIT ?
            """, (fts_query, max_results))

            return [dict(row) for row in cursor.fetchall()]
        except Exception as e:
            print(f"Search error: {e}")
            return []

    def delete_clipboard_item(self, item_id: int):
        """Delete a specific clipboard item.
        
        Args:
            item_id: ID of item to delete
        """
        cursor = self.conn.cursor()
        cursor.execute("DELETE FROM clipboard_items WHERE id = ?", (item_id,))
        cursor.execute("""
            DELETE FROM search_index 
            WHERE source_table = 'clipboard' AND source_id = ?
        """, (item_id,))
        self.conn.commit()

    def clear_clipboard_history(self):
        """Clear all clipboard history (keep master items)."""
        cursor = self.conn.cursor()
        cursor.execute("DELETE FROM clipboard_items")
        cursor.execute("DELETE FROM search_index WHERE source_table = 'clipboard'")
        self.conn.commit()

    def add_master_item(self, content: str, category: str, notes: str = "") -> int:
        """Add item to master collection.
        
        Args:
            content: Text content
            category: Category name
            notes: Optional notes
            
        Returns:
            Master item ID
        """
        timestamp = int(datetime.now().timestamp())
        cursor = self.conn.cursor()

        cursor.execute("""
            INSERT INTO master_items (content, category, timestamp, notes)
            VALUES (?, ?, ?, ?)
        """, (content, category, timestamp, notes))

        master_id = cursor.lastrowid

        # Add to search index
        cursor.execute("""
            INSERT INTO search_index (content, source_table, source_id)
            VALUES (?, 'master', ?)
        """, (content, master_id))

        self.conn.commit()
        return master_id

    def sync_master_from_excel(self, category: str, items: List[Dict[str, Any]]):
        """Sync master items from Excel file.
        
        Args:
            category: Category name
            items: List of items with 'content', 'timestamp', and optional 'notes'
        """
        cursor = self.conn.cursor()

        # Clear existing items for this category
        cursor.execute("""
            DELETE FROM search_index 
            WHERE source_table = 'master' 
            AND source_id IN (
                SELECT id FROM master_items WHERE category = ?
            )
        """, (category,))

        cursor.execute("DELETE FROM master_items WHERE category = ?", (category,))

        # Insert new items
        for item in items:
            cursor.execute("""
                INSERT INTO master_items (content, category, timestamp, notes)
                VALUES (?, ?, ?, ?)
            """, (
                item.get('content', ''),
                category,
                item.get('timestamp', int(datetime.now().timestamp())),
                item.get('notes', '')
            ))

            master_id = cursor.lastrowid

            cursor.execute("""
                INSERT INTO search_index (content, source_table, source_id)
                VALUES (?, 'master', ?)
            """, (item.get('content', ''), master_id))

        self.conn.commit()

    def close(self):
        """Close database connection."""
        if self.conn:
            self.conn.close()
