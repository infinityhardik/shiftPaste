"""Fuzzy search engine for clipboard items."""

import re
from typing import List, Dict, Any, NamedTuple
from datetime import datetime, timedelta


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
        if not query.strip():
            # Return items sorted by recency if no query
            sorted_items = sorted(
                items,
                key=lambda x: x.get('timestamp', 0),
                reverse=True
            )
            return [
                SearchMatch(item, 1.0) for item in sorted_items[:max_results]
            ]

        matches = []

        for item in items:
            content = item.get('content', '')
            if not content:
                continue

            # Calculate match quality
            match_quality = self._calculate_match_quality(query, content)

            if match_quality >= self.min_match_score:
                # Calculate recency score
                timestamp = item.get('timestamp', 0)
                recency = self._calculate_recency(timestamp)

                # Master items get 1.1x boost
                is_master = item.get('source_table') == 'master'
                master_boost = 1.1 if is_master else 1.0

                # Final score: 60% match quality, 40% recency
                final_score = (
                    (match_quality * 0.6 + recency * 0.4) * master_boost
                )

                matches.append(SearchMatch(item, final_score))

        # Sort by score descending
        matches.sort(key=lambda x: x.score, reverse=True)

        return matches[:max_results]

    def _calculate_match_quality(self, query: str, content: str) -> float:
        """Calculate match quality score using left-to-right matching.

        Args:
            query: Search query (case-insensitive)
            content: Text to search in (case-insensitive)

        Returns:
            Score from 0.0 to 1.0
        """
        query_lower = query.lower()
        content_lower = content.lower()

        if not query_lower or not content_lower:
            return 0.0

        # Check for exact substring match
        if query_lower in content_lower:
            # Exact match is highest quality
            return 1.0

        # Find character positions for left-to-right matching
        positions = self._find_leftright_matches(query_lower, content_lower)

        if not positions:
            return 0.0

        # Calculate quality based on positions
        return self._score_positions(positions, len(query_lower), len(content_lower))

    def _find_leftright_matches(self, query: str, text: str) -> List[int]:
        """Find left-to-right character positions for fuzzy matching.

        Args:
            query: Query string (lowercase)
            text: Text to search in (lowercase)

        Returns:
            List of character positions in text, or empty if no match
        """
        positions = []
        text_idx = 0

        for query_char in query:
            # Find next occurrence of this character
            found = False
            for i in range(text_idx, len(text)):
                if text[i] == query_char:
                    positions.append(i)
                    text_idx = i + 1
                    found = True
                    break

            if not found:
                return []  # Character not found, no match

        return positions

    def _score_positions(
        self,
        positions: List[int],
        query_len: int,
        text_len: int
    ) -> float:
        """Score match quality based on character positions.

        Args:
            positions: List of character positions
            query_len: Length of query
            text_len: Length of text

        Returns:
            Score from 0.0 to 1.0
        """
        if not positions:
            return 0.0

        # Base score starts at 0.5
        score = 0.5

        # Bonus for matching at the start (position 0)
        if positions[0] == 0:
            score += 0.3
        else:
            # Penalty for not matching at start, decreases with distance
            score -= min(positions[0] / (text_len + 1) * 0.15, 0.1)

        # Bonus for consecutive/tight matches
        consecutive_count = 0
        for i in range(len(positions) - 1):
            if positions[i + 1] == positions[i] + 1:
                consecutive_count += 1

        # Consecutive bonus: up to 0.15
        consecutive_bonus = min((consecutive_count / query_len) * 0.15, 0.15)
        score += consecutive_bonus

        # Penalty for gaps between matched characters
        total_gap = 0
        for i in range(len(positions) - 1):
            gap = positions[i + 1] - positions[i] - 1
            total_gap += gap

        # Gap penalty: increases with more gaps
        gap_penalty = min((total_gap / text_len) * 0.15, 0.15)
        score -= gap_penalty

        # Bonus for matching longer queries
        query_coverage = (query_len / max(text_len, 1)) * 0.1
        score += min(query_coverage, 0.1)

        # Normalize to 0-1 range
        final_score = min(max(score, 0.0), 1.0)

        return final_score

    def _calculate_recency(self, timestamp: int) -> float:
        """Calculate recency score for an item.

        Args:
            timestamp: Unix timestamp of item

        Returns:
            Score from 0.0 to 1.0
        """
        if not timestamp:
            return 0.0

        now = datetime.now()
        item_time = datetime.fromtimestamp(timestamp)
        age = now - item_time

        # Score decreases with age, using 7 days as reference
        age_hours = age.total_seconds() / 3600
        recency = 1.0 / (1.0 + age_hours / 168)

        return recency

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
