"""Fail-closed public inference API for specialist adapters."""
import copy
import math
from pathlib import Path

import numpy as np

from ..metrics import probabilities
from ..paths import read_json, resolve_checkpoint, sha256
from .input import parse, scores_with_review, token_count
from .som_protocol import config_for, validate_domain

SOM_PROTOCOL = "som-v1"

class SpecialistPredictor:
    """Rank supplied patches only.  It never executes supplied code."""
    def __init__(self, checkpoint_root=None):
        """Create a SOM predictor.

        SOM exposes one public protocol.  The storage module keeps its old
        file names until the data migration replaces it, but it is not part of
        the public API.
        """
        self.protocol = SOM_PROTOCOL
        self.root=Path(checkpoint_root) if checkpoint_root is not None else None; self._loaded={}
    def _load(self, domain):
        validate_domain(domain)
        if domain in self._loaded: return self._loaded[domain]
        config=config_for()
        if self.root is None:
            run=config.run_path(domain)
        elif (self.root / "adapter.safetensors").exists() or (self.root / "selected.json").exists():
            run=self.root
        else:
            run=self.root / domain
        checkpoint=resolve_checkpoint(run)
        policy=read_json(checkpoint / "specialist_policy.json")
        digest=sha256(checkpoint / "adapter.safetensors")
        if policy.get("adapter_sha256") != digest: raise ValueError("Specialist policy belongs to different adapter weights.")
        if policy.get("protocol") != SOM_PROTOCOL:
            raise ValueError("SOM calibration policy is invalid.")
        entry=policy.get("domains", {}).get(domain)
        if not isinstance(entry, dict) or not math.isfinite(entry.get("temperature", float("nan"))) or entry["temperature"] <= 0:
            raise ValueError("Specialist calibration policy is invalid.")
        # Import MLX only when a caller actually performs inference.  This keeps
        # ``from som.specialists import SpecialistPredictor`` is safe in a
        # CPU-only data-preparation process.
        # The frontend SOM loader has a separate provenance validator.  Do not
        # route it through the retired specialist source-lock validator.
        from .som_train import load_som_model
        from .model import Scorer
        model, tokenizer=load_som_model(checkpoint)
        loaded=(policy, Scorer(model, tokenizer), tokenizer, digest); self._loaded[domain]=loaded; return loaded
    def predict(self, value, domain):
        validate_domain(domain)
        request=parse(copy.deepcopy(value)); policy, scorer, tokenizer, digest=self._load(domain); count=token_count(request, tokenizer)
        review_id=config_for().REVIEW_ID
        scores=scorer.score(request); ids=[candidate.id for candidate in request.candidates]; all_scores=scores_with_review(request.candidates, scores); all_ids=ids+[review_id]
        temperature=policy["domains"][domain]["temperature"]; p=probabilities(all_scores, temperature); index=int(np.argmax(p)); proposed=all_ids[index]
        confidence=float(p[index]); reasons=[]; threshold=policy["domains"][domain].get("threshold")
        if proposed == review_id: reasons.append("no_supplied_patch_selected")
        if threshold is None or confidence < threshold: reasons.append("insufficient_calibrated_confidence")
        if not policy.get("acceptance_passed", False): reasons.append("specialist_acceptance_gates_not_met")
        if count > policy.get("validated_max_tokens", 0): reasons.append("input_longer_than_validated_range")
        counts=policy.get("validated_candidate_counts", [])
        if counts and len(request.candidates) not in counts: reasons.append("candidate_count_outside_validated_range")
        result={"status":"human_review" if reasons else "suggestion", "choice_id":None if reasons else proposed,
                "suggested_choice_id":None if proposed == review_id else proposed, "confidence":confidence, "domain":domain,
                "adapter_sha256":digest,"review_reasons":reasons,"input_tokens":count,
                "candidates":[{"id":ident,"score":float(score),"probability":float(probability)} for ident,score,probability in zip(all_ids,all_scores,p)]}
        result["protocol"] = SOM_PROTOCOL
        return result
