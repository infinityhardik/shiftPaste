"""Enhanced fuzzy search engine for clipboard and master items."""

import math
from datetime import datetime
from typing import List, Dict, Any, Tuple, Optional
from dataclasses import dataclass
from enum import Enum


class ItemType(Enum):
    """Type of searchable item."""
    CLIPBOARD = "clipboard"
    MASTER = "master"


@dataclass
class SearchConfig:
    """Configuration for search behavior."""
    recency_weight: float = 0.7
    quality_weight: float = 0.3
    clipboard_decay_seconds: int = 86400  # 24 hours
    master_decay_seconds: int = 604800    # 7 days
    min_quality_threshold: float = 0.0


class FuzzySearchEngine:
    """
    Advanced fuzzy search engine with left-to-right character matching.
    
    Features:
    - Sequential character matching with position tracking
    - Quality scoring based on character density
    - Recency-based ranking with exponential decay
    - Configurable weights and decay rates
    - Support for both clipboard and master items
    """

    def __init__(self, config: Optional[SearchConfig] = None):
        """
        Initialize search engine with optional configuration.
        
        Args:
            config: SearchConfig object for customizing behavior
        """
        self.config = config or SearchConfig()

    def fuzzy_left_to_right_match(
        self, 
        search_term: str, 
        text: str
    ) -> Tuple[bool, float]:
        """
        Perform fuzzy matching with sequential character requirements.
        
        Characters in search_term must appear in text in the same order
        (left-to-right), but don't need to be adjacent. Search works across
        the ENTIRE text including all lines, words, numbers, and special chars.
        
        Args:
            search_term: The search query
            text: The text to search within
            
        Returns:
            Tuple of (is_match: bool, quality_score: float)
            
            Quality score calculation:
            - 1.0 = perfect sequential match (all chars adjacent)
            - 0.0 = no match
            - Higher score for tighter character proximity
            
        Examples:
            >>> engine = FuzzySearchEngine()
            >>> engine.fuzzy_left_to_right_match("abc", "aXbXc")
            (True, 0.6)  # Matches but spread out
            >>> engine.fuzzy_left_to_right_match("abc", "abc")
            (True, 1.0)  # Perfect match
            >>> engine.fuzzy_left_to_right_match("L p", "LL Pro 18mm")
            (True, ~0.5)  # Matches 'L' then 'p' in 'Pro'
            >>> engine.fuzzy_left_to_right_match("1884", "18 mm 8 x 4")
            (True, ~0.3)  # Matches '18', '8', '4' in sequence
            >>> engine.fuzzy_left_to_right_match("lz", "LL Pro... ZVK")
            (True, ~0.1)  # Matches 'L' from LL, then 'Z' from ZVK
        """
        # Normalize search term: lowercase, keep all characters including spaces
        # Then remove spaces for flexible matching
        search = search_term.lower().replace(' ', '')
        
        # Normalize target: lowercase, keep ALL characters
        # This preserves numbers, letters, spaces, newlines, special chars
        target = text.lower()
        
        # Empty search matches everything perfectly
        if not search:
            return True, 1.0
        
        # Track matching positions for quality calculation
        pos = 0
        matched_positions = []
        
        # Find each character in sequence across the ENTIRE text
        for char in search:
            pos = target.find(char, pos)
            if pos == -1:
                return False, 0.0
            matched_positions.append(pos)
            pos += 1  # Start next search after current match
        
        # Calculate quality based on character density
        # Density = how tightly packed the matched characters are
        span = matched_positions[-1] - matched_positions[0] + 1
        density = len(search) / span if span > 0 else 1.0
        
        # Boost score for exact substring matches
        if search in target:
            density = min(1.0, density * 1.2)
        
        # Additional boost for word boundary matches
        # (matching start of words is higher quality)
        word_boundary_boost = 1.0
        for i, char_pos in enumerate(matched_positions):
            if char_pos == 0 or target[char_pos - 1] in ' \n\t-_':
                word_boundary_boost += 0.05
        
        density = min(1.0, density * word_boundary_boost)
        
        return True, density

    def _parse_datetime(self, date_val: Any) -> datetime:
        """
        Parse various datetime formats into datetime object.
        
        Args:
            date_val: String timestamp, datetime object, or None
            
        Returns:
            Parsed datetime or current time if parsing fails
        """
        now = datetime.now()
        
        if not date_val:
            return now
            
        if isinstance(date_val, datetime):
            return date_val
            
        if isinstance(date_val, str):
            # Try common SQLite timestamp format
            try:
                return datetime.strptime(date_val, "%Y-%m-%d %H:%M:%S")
            except ValueError:
                pass
            
            # Try ISO format
            try:
                return datetime.fromisoformat(date_val)
            except ValueError:
                pass
        
        return now

    def _calculate_recency_score(
        self, 
        item: Dict[str, Any], 
        now: datetime
    ) -> float:
        """
        Calculate recency score with exponential decay.
        
        Master items decay slower than clipboard items.
        
        Args:
            item: Item dictionary with timestamp fields
            now: Current datetime for calculation
            
        Returns:
            Recency score between 0.0 and 1.0
        """
        # Determine item type and extract timestamp
        master_modified = item.get('master_modified')
        last_copied_at = item.get('last_copied_at')
        
        if master_modified:
            # Master item: use master_modified timestamp
            item_time = self._parse_datetime(master_modified)
            decay_period = self.config.master_decay_seconds
        else:
            # Clipboard item: use last_copied_at timestamp
            item_time = self._parse_datetime(last_copied_at)
            decay_period = self.config.clipboard_decay_seconds
        
        # Calculate time difference in seconds
        time_diff = (now - item_time).total_seconds()
        
        # Exponential decay: e^(-t/T) where T is decay period
        # Recent items → 1.0, old items → 0.0
        recency = math.exp(-time_diff / decay_period)
        
        return recency

    def rank_search_results(
        self, 
        items: List[Dict[str, Any]], 
        search_term: str
    ) -> List[Dict[str, Any]]:
        """
        Search and rank items by combined score.
        
        Ranking formula:
            score = (recency_weight × recency) + (quality_weight × match_quality)
        
        Args:
            items: List of item dictionaries to search
            search_term: Search query string
            
        Returns:
            List of matching items sorted by score (highest first)
            Each item includes 'search_score' field
        """
        now = datetime.now()
        scored_items = []
        
        for item in items:
            content = item.get('content', '')
            
            # Check if item matches search term
            is_match, quality = self.fuzzy_left_to_right_match(search_term, content)
            
            if not is_match:
                continue
            
            # Apply quality threshold filter
            if quality < self.config.min_quality_threshold:
                continue
            
            # Calculate recency score
            recency = self._calculate_recency_score(item, now)
            
            # Combine scores with configured weights
            score = (
                self.config.recency_weight * recency + 
                self.config.quality_weight * quality
            )
            
            # Add score to item for debugging/UI display
            item_with_score = item.copy()
            item_with_score['search_score'] = score
            item_with_score['match_quality'] = quality
            item_with_score['recency_score'] = recency
            
            scored_items.append((item_with_score, score))
        
        # Sort by score in descending order
        return [
            item for item, score in sorted(
                scored_items, 
                key=lambda x: x[1], 
                reverse=True
            )
        ]

    def get_time_ago_string(self, date_val: Any) -> str:
        """
        Convert timestamp to human-readable 'time ago' format.
        
        Args:
            date_val: Datetime string, datetime object, or None
            
        Returns:
            Human-readable time ago string
            
        Examples:
            "Just now", "5 mins ago", "2 hours ago", 
            "Yesterday", "3 days ago", "Jan 15, 2024"
        """
        if not date_val:
            return "Unknown"

        now = datetime.now()
        dt = self._parse_datetime(date_val)

        diff = now - dt
        seconds = diff.total_seconds()
        
        # Handle future dates or very recent
        if seconds < 60:
            return "Just now"
        
        # Minutes
        if seconds < 3600:
            mins = int(seconds / 60)
            return f"{mins} min{'s' if mins != 1 else ''} ago"
        
        # Hours
        if seconds < 86400:
            hours = int(seconds / 3600)
            return f"{hours} hour{'s' if hours != 1 else ''} ago"
        
        # Days
        days = int(seconds / 86400)
        
        if days == 1:
            return "Yesterday"
        elif days < 7:
            return f"{days} days ago"
        elif days < 30:
            weeks = days // 7
            return f"{weeks} week{'s' if weeks != 1 else ''} ago"
        else:
            # Show formatted date for older items
            return dt.strftime("%b %d, %Y")

    def search(
        self, 
        items: List[Dict[str, Any]], 
        search_term: str,
        limit: Optional[int] = None
    ) -> List[Dict[str, Any]]:
        """
        Convenience method for searching with optional result limit.
        
        Args:
            items: Items to search
            search_term: Query string
            limit: Maximum number of results to return (None = all)
            
        Returns:
            Ranked search results
        """
        results = self.rank_search_results(items, search_term)
        
        if limit is not None and limit > 0:
            return results[:limit]
        
        return results


# Example usage
if __name__ == "__main__":
    # Create search engine with custom config
    config = SearchConfig(
        recency_weight=0.6,
        quality_weight=0.4,
        min_quality_threshold=0.0  # Accept all matches
    )
    engine = FuzzySearchEngine(config)
    
    # Sample data - using the example text
    sample_text = """*18 mm* :
LL Pro 18 mm 8 x 4 - 3
ZVK XL 18 mm 8 x 4 - 10
*12 mm* :
ZVK 12 mm 8 x 4 - 5
*25 BB* :
MRX AG 25 mm 7 x 4 - 1
Total : *19* Pcs."""
    
    sample_items = [
        {
            "content": sample_text,
            "last_copied_at": "2024-01-22 10:00:00"
        },
        {
            "content": "Python programming guide",
            "last_copied_at": "2024-01-21 15:30:00"
        },
        {
            "content": "Important master note",
            "master_modified": "2024-01-15 09:00:00"
        }
    ]
    
    # Test various search terms from the example
    test_searches = ["L p", "14", "1884", "lz", "m4", "ls", "ZK", "ma"]
    
    print("Testing fuzzy search with example queries:\n")
    for query in test_searches:
        is_match, quality = engine.fuzzy_left_to_right_match(query, sample_text)
        status = "✓ MATCH" if is_match else "✗ NO MATCH"
        print(f"{query:6s} -> {status} (quality: {quality:.3f})")
    
    print("\n" + "="*50)
    print("Full search results for 'L p':\n")
    
    # Full search with ranking
    results = engine.search(sample_items, "L p", limit=10)
    
    for i, result in enumerate(results, 1):
        content_preview = result['content'][:50].replace('\n', ' ')
        print(f"{i}. Score: {result['search_score']:.3f} - {content_preview}...")
