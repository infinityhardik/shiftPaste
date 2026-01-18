"""Tests for Shift Paste database operations."""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent))

from src.data.database import Database
from datetime import datetime
import tempfile
import os


def test_database_operations():
    """Test database operations."""
    # Use temporary database for testing
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = os.path.join(tmpdir, "test.db")
        db = Database(db_path)

        print("Testing Database Operations\n" + "="*50)

        # Test 1: Add clipboard items
        print("\nTest 1: Adding clipboard items")
        id1 = db.add_clipboard_item("First clipboard item")
        id2 = db.add_clipboard_item("Second clipboard item")
        id3 = db.add_clipboard_item("MARLEX A Grade 100% Door")
        id_dup = db.add_clipboard_item("Second clipboard item")  # Duplicate

        print(f"Added items: {id1}, {id2}, {id3}")
        print(f"Duplicate result (should be -1): {id_dup}")

        # Test 2: Get recent items
        print("\nTest 2: Getting recent items")
        recent = db.get_recent_items(limit=10)
        print(f"Recent items count: {len(recent)}")
        for item in recent:
            print(f"  - {item['content'][:40]}... (timestamp: {item['timestamp']})")

        # Test 3: Add master items
        print("\nTest 3: Adding master items")
        master_id1 = db.add_master_item(
            "Python tutorial link",
            "Work",
            "Python best practices"
        )
        master_id2 = db.add_master_item(
            "Email template",
            "Work",
            "Standard business email"
        )
        master_id3 = db.add_master_item(
            "Home address template",
            "Personal",
            "Address for forms"
        )

        print(f"Added master items: {master_id1}, {master_id2}, {master_id3}")

        # Test 4: Get all master items
        print("\nTest 4: Getting all master items")
        masters = db.get_all_master_items()
        print(f"Master items count: {len(masters)}")
        for item in masters:
            print(f"  - [{item['category']}] {item['content'][:30]}...")

        # Test 5: Search items (FTS5)
        print("\nTest 5: Searching items (FTS5)")
        results = db.search_all_items("python", max_results=10)
        print(f"Search 'python' found {len(results)} items")
        for item in results:
            print(f"  - [{item['source_table']}] {item['content'][:40]}...")

        results = db.search_all_items("door", max_results=10)
        print(f"Search 'door' found {len(results)} items")
        for item in results:
            print(f"  - [{item['source_table']}] {item['content'][:40]}...")

        results = db.search_all_items("template", max_results=10)
        print(f"Search 'template' found {len(results)} items")
        for item in results:
            print(f"  - [{item['source_table']}] {item['content'][:40]}...")

        # Test 6: Delete clipboard item
        print("\nTest 6: Deleting clipboard item")
        print(f"Items before deletion: {len(db.get_recent_items())}")
        db.delete_clipboard_item(id1)
        print(f"Items after deletion: {len(db.get_recent_items())}")

        # Test 7: Clear clipboard history
        print("\nTest 7: Clearing clipboard history")
        print(f"Clipboard items before clear: {len(db.get_recent_items())}")
        print(f"Master items before clear: {len(db.get_all_master_items())}")
        db.clear_clipboard_history()
        print(f"Clipboard items after clear: {len(db.get_recent_items())}")
        print(f"Master items after clear (should be unchanged): {len(db.get_all_master_items())}")

        # Test 8: Sync master from Excel
        print("\nTest 8: Syncing master items from Excel")
        excel_items = [
            {
                'content': 'New Excel item 1',
                'timestamp': int(datetime.now().timestamp()),
                'notes': 'From Excel'
            },
            {
                'content': 'New Excel item 2',
                'timestamp': int(datetime.now().timestamp()),
                'notes': 'Also from Excel'
            }
        ]
        db.sync_master_from_excel('Excel', excel_items)
        excel_masters = db.get_all_master_items()
        print(f"Master items after sync: {len(excel_masters)}")
        for item in excel_masters:
            if item['category'] == 'Excel':
                print(f"  - {item['content']}")

        db.close()
        print("\n" + "="*50)
        print("Database tests completed!")


if __name__ == "__main__":
    test_database_operations()
