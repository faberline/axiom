"""A LangChain-style recursive character text splitter with chunk overlap."""

from __future__ import annotations

DEFAULT_SEPARATORS = ("\n\n", "\n", " ", "")


class RecursiveCharacterTextSplitter:
    """Split on the coarsest separator that yields pieces under chunk_size."""

    def __init__(
        self,
        chunk_size: int = 200,
        chunk_overlap: int = 20,
        separators: tuple[str, ...] = DEFAULT_SEPARATORS,
    ) -> None:
        if chunk_size <= 0:
            raise ValueError("chunk_size must be positive")
        if not 0 <= chunk_overlap < chunk_size:
            raise ValueError("chunk_overlap must be in [0, chunk_size)")
        self.chunk_size = chunk_size
        self.chunk_overlap = chunk_overlap
        self.separators = separators

    def split_text(self, text: str) -> list[str]:
        """Return chunks no longer than chunk_size, in order."""
        return self._split(text, self.separators)

    def _split(self, text: str, separators: tuple[str, ...]) -> list[str]:
        if len(text) <= self.chunk_size:
            return [text] if text.strip() else []
        sep, rest = separators[0], separators[1:]
        if sep and sep not in text and rest:
            return self._split(text, rest)
        pieces = text.split(sep) if sep else list(text)
        chunks: list[str] = []
        for piece in pieces:
            if len(piece) > self.chunk_size and rest:
                chunks.extend(self._split(piece, rest))
            elif piece:
                chunks.append(piece)
        return self._merge(chunks, sep)

    def _merge(self, pieces: list[str], sep: str) -> list[str]:
        merged: list[str] = []
        window: list[str] = []
        for piece in pieces:
            candidate = sep.join([*window, piece])
            if window and len(candidate) > self.chunk_size:
                merged.append(sep.join(window))
                while window and (
                    len(sep.join(window)) > self.chunk_overlap
                    or len(sep.join([*window, piece])) > self.chunk_size
                ):
                    window = window[1:]
            window.append(piece)
        if window:
            merged.append(sep.join(window))
        return [chunk for chunk in merged if chunk.strip()]
