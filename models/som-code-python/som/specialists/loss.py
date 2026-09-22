"""Listwise ranker loss with explicit review and validity objectives."""
import mlx.core as mx

LOSS_CONFIG = {"binary_weight": .50, "present_margin_weight": .25, "review_margin_weight": .25, "teacher_kl_weight": .05, "margin": .50}

def specialist_loss(scores, gold_index, missing_correct_patch, prior=None, candidate_positive_weight=1.0):
    """Return ranking + binary + margin + optional frozen-v4 teacher loss.

    ``scores`` excludes the synthetic review candidate.  Review has fixed logit 0.
    """
    review = mx.zeros((1,)); logits = mx.concatenate([scores, review])
    gold = len(scores) if missing_correct_patch else gold_index
    ranking = mx.logsumexp(logits) - logits[gold]
    truth = mx.array([float((not missing_correct_patch) and i == gold_index) for i in range(len(scores))])
    positive_weight = (float(candidate_positive_weight.item())
                       if hasattr(candidate_positive_weight, "item") else float(candidate_positive_weight))
    if positive_weight <= 0:
        raise ValueError("Candidate correctness positive weight must be positive.")
    binary_terms = mx.logaddexp(mx.zeros_like(scores), scores) - truth * scores
    binary_weights = mx.ones_like(scores) + truth * (positive_weight - 1.0)
    # Divide by total mass so a balanced corpus keeps BCE on its former scale.
    binary = mx.sum(binary_weights * binary_terms) / mx.sum(binary_weights)
    margin = LOSS_CONFIG["margin"]
    if missing_correct_patch:
        margin_loss = mx.mean(mx.maximum(mx.zeros_like(scores), scores + margin))
        weighted_margin = LOSS_CONFIG["review_margin_weight"] * margin_loss
    else:
        others = mx.concatenate([scores[:gold_index], scores[gold_index + 1:], review])
        margin_loss = mx.mean(mx.maximum(mx.zeros_like(others), margin - scores[gold_index] + others))
        weighted_margin = LOSS_CONFIG["present_margin_weight"] * margin_loss
    teacher = mx.array(0.)
    if prior is not None:
        target = mx.sigmoid(prior)
        log_yes = lambda value: -mx.logaddexp(mx.zeros_like(value), -value)
        log_no = lambda value: -mx.logaddexp(mx.zeros_like(value), value)
        teacher = mx.mean(target * (log_yes(prior) - log_yes(scores)) + (1-target) * (log_no(prior) - log_no(scores)))
    return ranking + LOSS_CONFIG["binary_weight"] * binary + weighted_margin + LOSS_CONFIG["teacher_kl_weight"] * teacher
