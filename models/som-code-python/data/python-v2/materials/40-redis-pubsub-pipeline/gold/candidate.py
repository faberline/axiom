"""Redis atomic transaction pipeline with optimistic locking and pubsub manager."""
from typing import Any, Callable, Dict, List, Optional
import redis
from redis.exceptions import WatchError


class RedisTransactionPipeline:
    """Executes atomic operations inside a Redis MULTI/EXEC transaction with WATCH retries."""

    def __init__(self, client: Any, max_retries: int = 3) -> None:
        if max_retries < 0:
            raise ValueError("max_retries must be non-negative")
        self.client = client
        self.max_retries = max_retries

    def execute_transaction(
        self,
        watch_keys: List[str],
        callback: Callable[[Any], None],
    ) -> List[Any]:
        """Execute callback within an atomic pipeline, retrying on WatchError."""
        if not watch_keys:
            raise ValueError("watch_keys must not be empty")

        for attempt in range(self.max_retries + 1):
            pipe = self.client.pipeline(transaction=True)
            try:
                pipe.watch(*watch_keys)
                pipe.multi()
                callback(pipe)
                results = pipe.execute()
                return results
            except WatchError:
                if attempt >= self.max_retries:
                    raise
                continue
            finally:
                pipe.reset()


class RedisPubSubManager:
    """Manages channel subscriptions, publishing, and callback dispatching for Redis PubSub."""

    def __init__(self, client: Any) -> None:
        self.client = client
        self.pubsub = self.client.pubsub()
        self._handlers: Dict[str, List[Callable[[str, Any], None]]] = {}

    def subscribe(self, channel: str, handler: Callable[[str, Any], None]) -> None:
        """Subscribe handler to a named channel."""
        if not channel or not isinstance(channel, str) or not channel.strip():
            raise ValueError("channel must be a non-empty string")
        clean_channel = channel.strip()
        if clean_channel not in self._handlers:
            self._handlers[clean_channel] = []
            self.pubsub.subscribe(clean_channel)
        self._handlers[clean_channel].append(handler)

    def publish(self, channel: str, message: Any) -> int:
        """Publish message to a channel and return receiver count."""
        if not channel or not isinstance(channel, str) or not channel.strip():
            raise ValueError("channel must be a non-empty string")
        return self.client.publish(channel.strip(), message)

    def dispatch_message(self, message_dict: Dict[str, Any]) -> int:
        """Dispatch an incoming pubsub message dictionary to registered callbacks."""
        if not isinstance(message_dict, dict) or message_dict.get("type") != "message":
            return 0
        channel = message_dict.get("channel")
        data = message_dict.get("data")
        handlers = self._handlers.get(channel, [])
        for handler in handlers:
            handler(channel, data)
        return len(handlers)

    def unsubscribe(self, channel: str) -> None:
        """Unsubscribe all handlers from a channel."""
        clean_channel = channel.strip() if isinstance(channel, str) else ""
        if clean_channel in self._handlers:
            del self._handlers[clean_channel]
            self.pubsub.unsubscribe(clean_channel)

    def close(self) -> None:
        """Close pubsub connection and clear all channel subscriptions."""
        self._handlers.clear()
        self.pubsub.close()
