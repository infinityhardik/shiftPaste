"""Fuzzy search engine for clipboard and master items."""

import math
from datetime import datetime
from typing import List, Dict, Any, Tuple, Optional


class FuzzySearchEngine:
    """
    Fuzzy search engine with left-to-right character matching.
    
    Finds characters in sequential order across the entire text content.
    Characters can be separated by any amount of text in between.
    """

    def __init__(self):
        """Initialize search engine with default settings."""
        self.recency_weight = 0.7
        self.quality_weight = 0.3
        self.clipboard_decay_seconds = 86400  # 24 hours
        self.master_decay_seconds = 604800    # 7 days

    def fuzzy_left_to_right_match(self, search_term: str, text: str) -> Tuple[bool, float]:
        """
        Match characters in left-to-right order across entire text.
        
        Algorithm (as requested):
        1. Normalize search (lowercase, remove spaces)
        2. For each character, use text.find(char, position) to find it starting from the last position
        3. If found, move position forward and continue
        4. If not found, return no match
        """
        if not search_term:
            return True, 1.0
        
        # Step 1: Normalize search term
        search_chars = search_term.lower().replace(' ', '')
        if not search_chars:
            return True, 1.0
            
        # Step 2: Normalize text
        text_lower = text.lower()
        
        # Step 3: Find each character sequentially
        position = 0
        positions = []
        
        for char in search_chars:
            position = text_lower.find(char, position)
            if position == -1:
                return False, 0.0
            positions.append(position)
            position += 1  # Move past the found character
            
        # Step 4: Quality Score (for ranking)
        first = positions[0]
        last = positions[-1]
        span = last - first + 1
        quality = len(search_chars) / span
        
        # Exact substring bonus
        if search_chars in text_lower.replace(' ', ''):
            quality = min(1.0, quality * 1.2)
            
        return True, quality

    def _parse_datetime(self, date_val: Any) -> datetime:
        """Parse various datetime formats, return current time if invalid."""
        now = datetime.now()
        
        if not date_val:
            return now
        
        if isinstance(date_val, datetime):
            return date_val
        
        if isinstance(date_val, str):
            # Try SQLite format: "YYYY-MM-DD HH:MM:SS"
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

    def rank_search_results(self, items: List[Dict[str, Any]], search_term: str) -> List[Dict[str, Any]]:
        """
        Search and rank items by combined recency and match quality.
        
        Ranking formula:
            score = (0.7 × recency) + (0.3 × quality)
        
        Args:
            items: List of items to search (each item is a dict with 'content' key)
            search_term: Search query
            
        Returns:
            List of matching items sorted by score (highest first)
            Each item includes 'search_score', 'match_quality', 'recency_score'
        """
        now = datetime.now()
        scored_items = []
        
        for item in items:
            content = item.get('content', '')
            
            # Test if this item matches the search
            is_match, quality = self.fuzzy_left_to_right_match(search_term, content)
            
            if not is_match:
                # Skip items that don't match
                continue
            
            # Calculate recency score using exponential decay
            # Master items decay slower (7 days) than clipboard items (24 hours)
            master_modified = item.get('master_modified')
            last_copied_at = item.get('last_copied_at')
            
            if master_modified:
                # Master item
                item_time = self._parse_datetime(master_modified)
                decay_period = self.master_decay_seconds
            else:
                # Clipboard item
                item_time = self._parse_datetime(last_copied_at)
                decay_period = self.clipboard_decay_seconds
            
            # Calculate time difference
            time_diff = (now - item_time).total_seconds()
            
            # Exponential decay: e^(-t/T)
            # Recent items → score near 1.0
            # Old items → score near 0.0
            recency = math.exp(-time_diff / decay_period)
            
            # Combine scores with weighted formula
            score = (self.recency_weight * recency) + (self.quality_weight * quality)
            
            # Add scoring metadata to item
            item_with_score = item.copy()
            item_with_score['search_score'] = score
            item_with_score['match_quality'] = quality
            item_with_score['recency_score'] = recency
            
            scored_items.append((item_with_score, score))
        
        # Sort by score descending (highest scores first)
        sorted_items = sorted(scored_items, key=lambda x: x[1], reverse=True)
        
        return [item for item, score in sorted_items]

    def search(self, items: List[Dict[str, Any]], search_term: str, limit: Optional[int] = None) -> List[Dict[str, Any]]:
        """
        Search items and return ranked results with optional limit.
        
        Args:
            items: List of items to search
            search_term: Search query
            limit: Maximum number of results to return (None = all)
            
        Returns:
            Ranked list of matching items
        """
        results = self.rank_search_results(items, search_term)
        
        if limit is not None and limit > 0:
            return results[:limit]
        
        return results

    def get_time_ago_string(self, date_val: Any) -> str:
        """
        Convert timestamp to human-readable 'time ago' format.
        
        Examples:
            "Just now", "5 mins ago", "2 hours ago", 
            "Yesterday", "3 days ago", "2 weeks ago", "Jan 15, 2024"
        """
        if not date_val:
            return "Unknown"
        
        now = datetime.now()
        dt = self._parse_datetime(date_val)
        
        seconds = (now - dt).total_seconds()
        
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