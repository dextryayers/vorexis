"""Simple in-memory sliding-window rate limiter for sensitive endpoints."""
import time
from collections import defaultdict, deque

_requests: dict[str, deque[float]] = defaultdict(deque)
WINDOW = 60.0
MAX = 30


def rate_limited(key: str, max_per_minute: int = MAX, window: float = WINDOW) -> bool:
    """Returns True if the request should be rejected (over limit)."""
    now = time.monotonic()
    dq = _requests[key]
    while dq and now - dq[0] > window:
        dq.popleft()
    if len(dq) >= max_per_minute:
        return True
    dq.append(now)
    return False
