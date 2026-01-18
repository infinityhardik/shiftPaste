"""Fuzzy search engine for clipboard items."""

from typing import List, Dict, Any, NamedTuple
from datetime import datetime


class SearchMatch(NamedTuple):
    """Represents a search match result."""
    metadata: Dict[str, Any]
    score: float


class FuzzySearchEngine:
    """Fuzzy search engine with left-to-right character matching and ranking."""

    def __init__(self):
        """Initialize search engine."""
        self.min_match_score = 0.1  # Minimum score to include result

    def search(
        self,
        query: str,
        items: List[Dict[str, Any]],
        max_results: int = 20
    ) -> List[SearchMatch]:
        """Search items using fuzzy matching with ranking.

        Args:
            query: Search query string
            items: List of items with 'content', 'timestamp', etc.
            max_results: Maximum results to return

        Returns:
            Ranked list of search matches
        """
        # Simple subsequence search: return items whose `content` contains
        # the query characters in order (case-insensitive). No weights,
        # recency or other scoring — matched items get score 1.0.
        q = query or ""

        matches: List[SearchMatch] = []

        if not q:
            # If query is empty, return items in original order with score 1.0
            return [SearchMatch(item, 1.0) for item in items[:max_results]]

        for item in items:
            content = item.get('content', '')
            if not content:
                continue

            if self._contains_in_order(content, q):
                matches.append(SearchMatch(item, 1.0))

        return matches[:max_results]

    def _contains_in_order(self, text: str, query: str) -> bool:
        """Return True if `query` characters appear in `text` in order.

        This implements a simple, efficient subsequence check (case-insensitive).
        It performs a character-by-character left-to-right match without
        altering the query (no whitespace stripping).
        """
        if query is None:
            return True

        text_lower = text.lower()
        query_lower = query.lower()

        ti = 0
        qi = 0
        while ti < len(text_lower) and qi < len(query_lower):
            if text_lower[ti] == query_lower[qi]:
                qi += 1
            ti += 1

        return qi == len(query_lower)


    def get_time_ago_string(self, timestamp: int) -> str:
        """Convert timestamp to human-readable 'time ago' string.

        Args:
            timestamp: Unix timestamp

        Returns:
            Human-readable time string
        """
        if not timestamp:
            return "Unknown"

        now = datetime.now()
        item_time = datetime.fromtimestamp(timestamp)
        diff = now - item_time

        seconds = diff.total_seconds()
        if seconds < 60:
            return "Just now"
        elif seconds < 3600:
            mins = int(seconds / 60)
            return f"{mins} min{'s' if mins > 1 else ''} ago"
        elif seconds < 86400:
            hours = int(seconds / 3600)
            return f"{hours} hour{'s' if hours > 1 else ''} ago"
        else:
            days = int(seconds / 86400)
            if days == 1:
                return "Yesterday"
            elif days < 7:
                return f"{days} days ago"
            else:
                return item_time.strftime("%b %d, %Y")
