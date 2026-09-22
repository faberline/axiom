"""Input validation and one shared tokenizer layout for training and inference."""
import json
from dataclasses import dataclass


@dataclass(frozen=True)
class Candidate:
    id: str
    text: str


@dataclass(frozen=True)
class DecisionRequest:
    state: str
    question: str
    candidates: tuple[Candidate, ...]

    @classmethod
    def from_dict(cls, value):
        if not isinstance(value, dict):
            raise ValueError("Input must be a JSON object.")
        for name in ("state", "question"):
            if not isinstance(value.get(name), str) or not value[name].strip():
                raise ValueError(f"{name} must be a nonempty string.")
        raw = value.get("candidates")
        if not isinstance(raw, list) or not 2 <= len(raw) <= 8:
            raise ValueError("Provide 2–8 candidates.")
        candidates = []
        for item in raw:
            if not isinstance(item, dict):
                raise ValueError("Each candidate must have id and text fields.")
            if any(not isinstance(item.get(k), str) or not item[k].strip() for k in ("id", "text")):
                raise ValueError("Candidate id and text must be nonempty strings.")
            candidates.append(Candidate(item["id"], item["text"]))
        if len({c.id for c in candidates}) != len(candidates):
            raise ValueError("Candidate IDs must be unique.")
        return cls(value["state"], value["question"], tuple(candidates))


@dataclass(frozen=True)
class EncodedRequest:
    tokens: list[int]
    candidate_positions: list[int]
    decision_position: int


def encode(request, tokenizer, max_tokens=512):
    request = DecisionRequest.from_dict(request) if isinstance(request, dict) else request
    # Encode chunks separately so marker offsets cannot drift at BPE boundaries.
    chunks = [
        "Choose the candidate that best answers the question about the state.\n",
        f"STATE: {json.dumps(request.state, ensure_ascii=False)}\n",
        f"QUESTION: {json.dumps(request.question, ensure_ascii=False)}\n",
    ]
    tokens = []
    for chunk in chunks:
        tokens.extend(tokenizer.encode(chunk, add_special_tokens=False))
    positions = []
    for i, candidate in enumerate(request.candidates):
        chunk = f"CANDIDATE {chr(65 + i)}: {json.dumps(candidate.text, ensure_ascii=False)}\nEND CANDIDATE\n"
        tokens.extend(tokenizer.encode(chunk, add_special_tokens=False))
        positions.append(len(tokens) - 1)
    tokens.extend(tokenizer.encode("DECIDE:\n", add_special_tokens=False))
    if len(tokens) > max_tokens:
        raise ValueError(f"Input has {len(tokens)} tokens; maximum is {max_tokens}. No text was truncated.")
    return EncodedRequest(tokens, positions, len(tokens) - 1)
