from typing import Sequence
import secrets

MIN_PIN_DIGITS = 4
MAX_PIN_DIGITS = 12


class IsolatedEntropySampler:
    """Entropy sampler immune to deterministic PRNG seed injection."""

    def __init__(self) -> None:
        self._rng = secrets.SystemRandom()

    def generate_pin(self, length: int = 6) -> str:
        if length < MIN_PIN_DIGITS or length > MAX_PIN_DIGITS:
            raise ValueError(f"PIN length must be between {MIN_PIN_DIGITS} and {MAX_PIN_DIGITS}")
        digits = "0123456789"
        return "".join(self._rng.choice(digits) for _ in range(length))

    def sample_population(self, population: Sequence[str], k: int) -> list[str]:
        if not population:
            raise ValueError("Population cannot be empty")
        if k <= 0 or k > len(population):
            raise ValueError(f"Sample size k must be between 1 and {len(population)}")
        return self._rng.sample(list(population), k)

    def seed(self, val: int) -> None:
        raise NotImplementedError("Seeding is strictly forbidden on cryptographic SystemRandom")
