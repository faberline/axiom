"""Database settings that keep the password out of reprs and log records."""

from typing import Any
from urllib.parse import quote

from pydantic import BaseModel, ConfigDict, Field, SecretStr


class DatabaseConfig(BaseModel):
    """Connection settings for a PostgreSQL database."""

    model_config = ConfigDict(frozen=True)

    host: str = Field(min_length=1)
    port: int = Field(default=5432, ge=1, lt=65535)
    user: str = Field(min_length=1)
    password: SecretStr
    database: str = Field(min_length=1)

    def url(self) -> str:
        """The full connection URL, with the password percent-encoded."""
        password = quote(self.password.get_secret_value(), safe="")
        return (
            f"postgresql://{self.user}:{password}"
            f"@{self.host}:{self.port}/{self.database}"
        )

    def redacted_url(self) -> str:
        """The connection URL with the password replaced, safe for logs."""
        return f"postgresql://{self.user}:***@{self.host}:{self.port}/{self.database}"


def log_fields(config: DatabaseConfig) -> dict[str, Any]:
    """JSON-ready settings for structured logs; the password stays masked."""
    return config.model_dump(mode="json")
