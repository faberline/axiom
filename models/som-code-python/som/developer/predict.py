import copy
import math

from ..metrics import probabilities
from ..model import Predictor
from ..paths import read_json, resolve_checkpoint, sha256
from ..schema import DecisionRequest, encode
from .data import REVIEW_ID, REVIEW_TEXT, QUESTION
from .train import RUN


class DeveloperPredictor:
    """Rank supplied patches. This API never executes or applies user code."""
    def __init__(self, checkpoint=RUN, base_factory=Predictor):
        self.checkpoint = resolve_checkpoint(checkpoint)
        self.policy = read_json(self.checkpoint / "developer_policy.json")
        if self.policy["adapter_sha256"] != sha256(self.checkpoint / "adapter.safetensors"):
            raise ValueError("Developer policy belongs to different model weights.")
        if type(self.policy.get("acceptance_passed")) is not bool:
            raise ValueError("Developer acceptance state must be a boolean.")
        for domain in ("python", "frontend"):
            entry = self.policy["domains"][domain]
            temperature, threshold = entry["temperature"], entry["threshold"]
            if not math.isfinite(temperature) or temperature <= 0:
                raise ValueError("Developer temperature must be finite and positive.")
            if threshold is not None and (not math.isfinite(threshold) or not 0 <= threshold <= 1):
                raise ValueError("Developer threshold must be null or between zero and one.")
        if type(self.policy.get("validated_max_tokens")) is not int or self.policy["validated_max_tokens"] <= 0:
            raise ValueError("Developer validated length must be a positive integer.")
        counts = self.policy.get("validated_candidate_counts")
        if counts is not None and (not isinstance(counts, list) or not counts or
                                  any(type(n) is not int or not 2 <= n <= 8 for n in counts)):
            raise ValueError("Developer validated candidate counts must be between 2 and 8.")
        self.base = base_factory(self.checkpoint)

    def predict(self, value, domain):
        if domain not in ("python", "frontend"):
            raise ValueError("Domain must be python or frontend.")
        if not isinstance(value, dict):
            raise ValueError("Input must be a JSON object.")
        value = copy.deepcopy(value)
        value.setdefault("question", QUESTION)
        request = DecisionRequest.from_dict(value)
        if len(request.candidates) > 7:
            raise ValueError("Developer mode accepts 2–7 patches, reserving one slot for human review.")
        if any(c.id == REVIEW_ID for c in request.candidates):
            raise ValueError("The candidate ID __review__ is reserved.")
        value["candidates"].append({"id": REVIEW_ID, "text": REVIEW_TEXT})
        input_tokens = self.base.token_count(value)
        first = self.base.predict(value)
        value["candidates"].reverse()
        second = self.base.predict(value)
        domain_policy = self.policy["domains"][domain]
        temperature = domain_policy["temperature"]
        p = probabilities([c["score"] for c in first["candidates"]], temperature)
        rp = probabilities([c["score"] for c in second["candidates"]], temperature)
        probs = {c["id"]: float(p[i]) for i,c in enumerate(first["candidates"])}
        reverse_probs = {c["id"]: float(rp[i]) for i,c in enumerate(second["candidates"])}
        choice = first["choice_id"]
        confidence = min(probs[choice], reverse_probs[choice])
        reasons = []
        if choice == REVIEW_ID:
            reasons.append("no_supplied_patch_selected")
        if choice != second["choice_id"]:
            reasons.append("choice_changes_when_order_reverses")
        threshold = domain_policy["threshold"]
        if threshold is None or confidence < threshold:
            reasons.append("insufficient_calibrated_confidence")
        if not self.policy["acceptance_passed"]:
            reasons.append("developer_acceptance_gates_not_met")
        if input_tokens > self.policy["validated_max_tokens"]:
            reasons.append("input_longer_than_validated_range")
        counts = self.policy.get("validated_candidate_counts")
        if counts is not None and len(value["candidates"]) not in counts:
            reasons.append("candidate_count_outside_validated_range")
        return {"status": "human_review" if reasons else "suggestion", "domain": domain,
                "choice_id": None if reasons else choice,
                "suggested_choice_id": None if choice == REVIEW_ID else choice,
                "confidence": confidence, "review_reasons": reasons, "input_tokens": input_tokens,
                "candidates": [{**c, "probability": probs[c["id"]]} for c in first["candidates"]],
                "scope": "Short candidate-repair assistance; inspect and test changes before use."}
