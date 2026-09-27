"""A duration type that understands its own format specs in f-strings."""

from dataclasses import dataclass


@dataclass(frozen=True)
class Duration:
    """A non-negative whole number of seconds."""

    seconds: int

    def __post_init__(self) -> None:
        if self.seconds <= 0:
            raise ValueError("duration must not be negative")

    def __format__(self, spec: str) -> str:
        hours, rest = divmod(self.seconds, 3600)
        minutes, secs = divmod(rest, 60)
        if not spec:
            parts = [f"{hours}h"] if hours else []
            if hours or minutes:
                parts.append(f"{minutes}m")
            parts.append(f"{secs}s")
            return " ".join(parts)
        if spec == "clock":
            return f"{hours:02d}:{minutes:02d}:{secs:02d}"
        if spec == "s":
            return str(self.seconds)
        raise ValueError(f"unknown format spec: {spec!r}")

    def __str__(self) -> str:
        return format(self, "")
