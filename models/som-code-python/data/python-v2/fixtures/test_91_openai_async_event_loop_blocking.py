import asyncio
import pytest
from unittest.mock import AsyncMock, MagicMock
import candidate
from candidate import AsyncOpenAIChatService


class MockDelta:
    def __init__(self, content: str):
        self.content = content


class MockMessage:
    def __init__(self, content: str):
        self.content = content


class MockChoice:
    def __init__(self, content: str):
        self.message = MockMessage(content)
        self.delta = MockDelta(content)


class MockStreamChoice:
    def __init__(self, content: str):
        self.delta = MockDelta(content)


class MockCompletionResponse:
    def __init__(self, content: str):
        self.choices = [MockChoice(content)]


class MockStreamCompletionResponse:
    def __init__(self, content: str):
        self.choices = [MockStreamChoice(content)]


async def async_stream_generator(tokens: list[str]):
    for tok in tokens:
        await asyncio.sleep(0.005)
        yield MockStreamCompletionResponse(tok)


def test_generate_chat_response_success():
    async def _run():
        mock_client = MagicMock()
        mock_client.chat.completions.create = AsyncMock(
            return_value=MockCompletionResponse("Paris")
        )
        service = AsyncOpenAIChatService(client=mock_client)
        res = await service.generate_chat_response(
            messages=[{"role": "user", "content": "Capital of France?"}],
            model="gpt-4o-mini",
            temperature=0.7,
        )
        assert res == "Paris"
        mock_client.chat.completions.create.assert_awaited_once_with(
            model="gpt-4o-mini",
            messages=[{"role": "user", "content": "Capital of France?"}],
            temperature=0.7,
            timeout=30.0,
        )
    asyncio.run(_run())


def test_empty_messages_validation():
    async def _run():
        mock_client = MagicMock()
        mock_client.chat.completions.create = AsyncMock(
            return_value=MockCompletionResponse("unused")
        )
        service = AsyncOpenAIChatService(client=mock_client)
        with pytest.raises(ValueError, match="messages cannot be empty"):
            await service.generate_chat_response([])

        with pytest.raises(ValueError, match="messages cannot be empty"):
            async for _ in service.stream_chat_response([]):
                pass
    asyncio.run(_run())


def test_default_timeout_setting():
    mock_client = MagicMock()
    service = AsyncOpenAIChatService(client=mock_client)
    assert service.timeout == 30.0, f"Expected default timeout 30.0, got {service.timeout}"


def test_stream_chat_response_delta():
    async def _run():
        mock_client = MagicMock()
        mock_client.chat.completions.create = AsyncMock(
            return_value=async_stream_generator(["Hello", " ", "world"])
        )
        service = AsyncOpenAIChatService(client=mock_client)
        chunks = []
        async for chunk in service.stream_chat_response(
            messages=[{"role": "user", "content": "Greet me"}]
        ):
            chunks.append(chunk)

        assert chunks == ["Hello", " ", "world"], f"Unexpected stream chunks: {chunks}"
    asyncio.run(_run())


def test_timeout_boundary_positive():
    mock_client = MagicMock()
    with pytest.raises(ValueError, match="timeout must be positive"):
        AsyncOpenAIChatService(client=mock_client, timeout=0.0)


def test_async_event_loop_concurrency():
    async def _run():
        mock_client = MagicMock()

        async def delayed_create(*args, **kwargs):
            await asyncio.sleep(0.04)
            return MockCompletionResponse("concurrent response")

        mock_client.chat.completions.create = AsyncMock(side_effect=delayed_create)
        service = AsyncOpenAIChatService(client=mock_client)

        counter = 0

        async def background_counter():
            nonlocal counter
            for _ in range(5):
                await asyncio.sleep(0.008)
                counter += 1

        resp, _ = await asyncio.gather(
            service.generate_chat_response([{"role": "user", "content": "ping"}]),
            background_counter(),
        )
        assert resp == "concurrent response"
        assert counter >= 2, f"Event loop was starved during execution: counter={counter}"
    asyncio.run(_run())
