"""Fuzzy search engine for clipboard and master items.

Search Algorithm: Left-to-Right Sequential Matching
- Finds characters in order across the entire text content
- Characters can be separated by any amount of text
- Spaces in search term are ignored for flexible matching

Ranking Formula:
    score = (0.7 × recency) + (0.3 × quality)
    
- Recency uses exponential decay (recent items score higher)
- Quality measures how compact the match is (consecutive chars score higher)
"""

import math
from datetime import datetime
from typing import List, Dict, Any, Tuple, Optional


class FuzzySearchEngine:
    """Fuzzy search engine with left-to-right character matching.
    
    Thread Safety: This class is stateless except for configuration,
    making it safe to use from multiple threads.
    """
    
    # Ranking weights
    DEFAULT_RECENCY_WEIGHT = 0.7
    DEFAULT_QUALITY_WEIGHT = 0.3
    
    # Time decay constants
    CLIPBOARD_DECAY_SECONDS = 86400   # 24 hours for clipboard items
    MASTER_DECAY_SECONDS = 604800     # 7 days for master items

    def __init__(
        self, 
        recency_weight: float = DEFAULT_RECENCY_WEIGHT,
        quality_weight: float = DEFAULT_QUALITY_WEIGHT
    ):
        """Initialize search engine with configurable weights.
        
        Args:
            recency_weight: Weight for recency score (0-1)
            quality_weight: Weight for match quality score (0-1)
        """
        # Normalize weights to sum to 1
        total = recency_weight + quality_weight
        self.recency_weight = recency_weight / total if total > 0 else 0.7
        self.quality_weight = quality_weight / total if total > 0 else 0.3

    def fuzzy_left_to_right_match(self, search_term: str, text: str) -> Tuple[bool, float]:
        """Match characters in left-to-right order across entire text.
        
        Algorithm:
        1. Normalize search term (lowercase, remove spaces)
        2. For each character, find it in text starting from last position
        3. If found, move position forward and continue
        4. If not found, return no match
        
        Args:
            search_term: The search query
            text: The text to search in
            
        Returns:
            Tuple of (is_match: bool, quality_score: float)
            - is_match: True if all characters found in order
            - quality_score: 0.0-1.0, higher = better match quality
        """
        # Empty search matches everything
        if not search_term:
            return True, 1.0
        
        # Normalize search term: lowercase and remove spaces
        search_chars = search_term.lower().replace(' ', '')
        if not search_chars:
            return True, 1.0
            
        # Normalize text for matching
        text_lower = text.lower()
        
        if not text_lower:
            return False, 0.0
        
        # Find each character sequentially
        position = 0
        positions: List[int] = []
        
        for char in search_chars:
            pos = text_lower.find(char, position)
            if pos == -1:
                # Character not found - no match
                return False, 0.0
            positions.append(pos)
            position = pos + 1  # Move past the found character
        
        # Calculate quality score based on match span
        # Consecutive matches score higher than scattered ones
        first_pos = positions[0]
        last_pos = positions[-1]
        span = last_pos - first_pos + 1
        
        # Base quality: ratio of search length to span
        # Perfect consecutive match: quality = 1.0
        quality = len(search_chars) / span
        
        # Bonus for substring matches (ignoring spaces in text)
        text_no_spaces = text_lower.replace(' ', '')
        if search_chars in text_no_spaces:
            quality = min(1.0, quality * 1.5)
        
        return True, quality

    def _parse_datetime(self, date_val: Any) -> datetime:
        """Parse various datetime formats, return current time if invalid.
        
        Supported formats:
        - datetime object
        - SQLite format: "YYYY-MM-DD HH:MM:SS"
        - ISO format: "YYYY-MM-DDTHH:MM:SS"
        
        Args:
            date_val: Datetime value in various formats
            
        Returns:
            Parsed datetime or current time if parsing fails
        """
        now = datetime.now()
        
        if not date_val:
            return now
        
        if isinstance(date_val, datetime):
            return date_val
        
        if isinstance(date_val, str):
            # Try SQLite format first (most common in our DB)
            try:
                return datetime.strptime(date_val, "%Y-%m-%d %H:%M:%S")
            except ValueError:
                pass
            
            # Try ISO format
            try:
                return datetime.fromisoformat(date_val.replace('Z', '+00:00'))
            except ValueError:
                pass
        
        return now

    def _calculate_recency_score(self, item_time: datetime, is_master: bool) -> float:
        """Calculate recency score using exponential decay.
        
        Args:
            item_time: When the item was copied/modified
            is_master: True for master items (slower decay)
            
        Returns:
            Score from 0.0 (old) to 1.0 (recent)
        """
        now = datetime.now()
        time_diff = (now - item_time).total_seconds()
        
        # Don't allow negative time differences
        if time_diff < 0:
            time_diff = 0
        
        decay_period = self.MASTER_DECAY_SECONDS if is_master else self.CLIPBOARD_DECAY_SECONDS
        
        # Exponential decay: e^(-t/T)
        return math.exp(-time_diff / decay_period)

    def rank_search_results(
        self, 
        items: List[Dict[str, Any]], 
        search_term: str
    ) -> List[Dict[str, Any]]:
        """Search and rank items by combined recency and match quality.
        
        Ranking formula:
            score = (recency_weight × recency) + (quality_weight × quality)
        
        Args:
            items: List of items to search (each has 'content' key)
            search_term: Search query
            
        Returns:
            List of matching items sorted by score (highest first)
            Each item includes 'search_score', 'match_quality', 'recency_score'
        """
        scored_items: List[Tuple[Dict[str, Any], float]] = []
        
        for item in items:
            content = item.get('content', '')
            if not content:
                continue
            
            # Test if this item matches the search
            is_match, quality = self.fuzzy_left_to_right_match(search_term, content)
            
            if not is_match:
                continue
            
            # Determine if this is a master item
            is_master = item.get('master_file_id') is not None or item.get('master_modified') is not None
            
            # Get the relevant timestamp
            if is_master:
                item_time = self._parse_datetime(item.get('master_modified'))
            else:
                item_time = self._parse_datetime(item.get('last_copied_at'))
            
            # Calculate recency score
            recency = self._calculate_recency_score(item_time, is_master)
            
            # Calculate combined score
            score = (self.recency_weight * recency) + (self.quality_weight * quality)
            
            # Add scoring metadata to item
            item_with_score = item.copy()
            item_with_score['search_score'] = score
            item_with_score['match_quality'] = quality
            item_with_score['recency_score'] = recency
            
            scored_items.append((item_with_score, score))
        
        # Sort by score descending
        scored_items.sort(key=lambda x: x[1], reverse=True)
        
        return [item for item, _ in scored_items]

    def search(
        self, 
        items: List[Dict[str, Any]], 
        search_term: str, 
        limit: Optional[int] = None
    ) -> List[Dict[str, Any]]:
        """Search items and return ranked results with optional limit.
        
        Args:
            items: List of items to search
            search_term: Search query
            limit: Maximum number of results (None = all)
            
        Returns:
            Ranked list of matching items
        """
        results = self.rank_search_results(items, search_term)
        
        if limit is not None and limit > 0:
            return results[:limit]
        
        return results

    def get_time_ago_string(self, date_val: Any) -> str:
        """Convert timestamp to human-readable 'time ago' format.
        
        Examples: "Just now", "5 mins ago", "2 hours ago", 
                  "Yesterday", "3 days ago", "2 weeks ago", "Jan 15, 2024"
        
        Args:
            date_val: Timestamp to format
            
        Returns:
            Human-readable relative time string
        """
        if not date_val:
            return "Unknown"
        
        dt = self._parse_datetime(date_val)
        now = datetime.now()
        
        seconds = (now - dt).total_seconds()
        
        # Handle future dates (clock skew)
        if seconds < 0:
            return "Just now"
        
        if seconds < 60:
            return "Just now"
        elif seconds < 3600:
            mins = int(seconds / 60)
            return f"{mins} min{'s' if mins != 1 else ''} ago"
        elif seconds < 86400:
            hours = int(seconds / 3600)
            return f"{hours} hour{'s' if hours != 1 else ''} ago"
        else:
            days = int(seconds / 86400)
            if days == 1:
                return "Yesterday"
            elif days < 7:
                return f"{days} days ago"
            elif days < 30:
                weeks = days // 7
                return f"{weeks} week{'s' if weeks != 1 else ''} ago"
            else:
                return dt.strftime("%b %d, %Y")