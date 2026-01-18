"""Tests for Shift Paste search engine."""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent))

from src.core.search_engine import FuzzySearchEngine
from datetime import datetime, timedelta


def test_fuzzy_search():
    """Test fuzzy search matching."""
    engine = FuzzySearchEngine()

    # Test data
    items = [
        {
            'content': 'MARLEX A Grade 100% 35 mm BWP Flush Door',
            'timestamp': int(datetime.now().timestamp()),
            'source_table': 'clipboard'
        },
        {
            'content': 'Standard door specifications for office',
            'timestamp': int((datetime.now() - timedelta(hours=1)).timestamp()),
            'source_table': 'master',
            'category': 'Work'
        },
        {
            'content': 'Python programming tutorial',
            'timestamp': int((datetime.now() - timedelta(minutes=30)).timestamp()),
            'source_table': 'clipboard'
        },
        {
            'content': 'XML configuration file example',
            'timestamp': int((datetime.now() - timedelta(hours=2)).timestamp()),
            'source_table': 'clipboard'
        }
    ]

    print("Testing Fuzzy Search Engine\n" + "="*50)

    # Test 1: Search for "mrlx"
    print("\nTest 1: Search for 'mrlx'")
    results = engine.search('mrlx', items, max_results=5)
    print(f"Results: {len(results)}")
    for match in results:
        print(f"  - Score: {match.score:.2f} | {match.metadata['content'][:50]}...")

    # Test 2: Search for "door"
    print("\nTest 2: Search for 'door'")
    results = engine.search('door', items, max_results=5)
    print(f"Results: {len(results)}")
    for match in results:
        print(f"  - Score: {match.score:.2f} | {match.metadata['content'][:50]}...")

    # Test 3: Search for "grade 100"
    print("\nTest 3: Search for 'grade 100'")
    results = engine.search('grade 100', items, max_results=5)
    print(f"Results: {len(results)}")
    for match in results:
        print(f"  - Score: {match.score:.2f} | {match.metadata['content'][:50]}...")

    # Test 4: Empty search (should return all sorted by recency)
    print("\nTest 4: Empty search (all items by recency)")
    results = engine.search('', items, max_results=5)
    print(f"Results: {len(results)}")
    for match in results:
        print(f"  - Score: {match.score:.2f} | {match.metadata['content'][:50]}...")

    # Test 5: Invalid left-to-right match
    print("\nTest 5: Invalid search 'xml' (should not match)")
    results = engine.search('xml', items, max_results=5)
    print(f"Results: {len(results)}")
    for match in results:
        print(f"  - Score: {match.score:.2f} | {match.metadata['content'][:50]}...")

    print("\n" + "="*50)
    print("Tests completed!")


def test_time_formatting():
    """Test time ago formatting."""
    engine = FuzzySearchEngine()

    print("\nTesting Time Formatting\n" + "="*50)

    # Current time
    now = datetime.now()
    print(f"Now: {engine.get_time_ago_string(int(now.timestamp()))}")

    # 30 seconds ago
    thirty_sec_ago = now - timedelta(seconds=30)
    print(f"30 seconds ago: {engine.get_time_ago_string(int(thirty_sec_ago.timestamp()))}")

    # 5 minutes ago
    five_min_ago = now - timedelta(minutes=5)
    print(f"5 minutes ago: {engine.get_time_ago_string(int(five_min_ago.timestamp()))}")

    # 2 hours ago
    two_hours_ago = now - timedelta(hours=2)
    print(f"2 hours ago: {engine.get_time_ago_string(int(two_hours_ago.timestamp()))}")

    # Yesterday
    yesterday = now - timedelta(days=1)
    print(f"Yesterday: {engine.get_time_ago_string(int(yesterday.timestamp()))}")

    # 5 days ago
    five_days_ago = now - timedelta(days=5)
    print(f"5 days ago: {engine.get_time_ago_string(int(five_days_ago.timestamp()))}")

    # 3 months ago
    three_months_ago = now - timedelta(days=90)
    print(f"3 months ago: {engine.get_time_ago_string(int(three_months_ago.timestamp()))}")

    print("="*50)


if __name__ == "__main__":
    test_fuzzy_search()
    test_time_formatting()
