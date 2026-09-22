"""V5 objective: learn absolute validity as well as relative patch ranking."""
import mlx.core as mx

LOSS_CONFIG = {"binary_weight": 1., "hard_negative_weight": .25, "negative_margin": 1.}


def score_loss(scores, gold):
    """The last class is review, with fixed score zero. No test-time score shift."""
    n = scores.shape[0]
    logits = mx.concatenate([scores, mx.zeros((1,))])
    ce = mx.logsumexp(logits) - logits[gold]
    negative = [i for i in range(n) if i != gold]
    no_loss = mx.mean(mx.logaddexp(mx.zeros((len(negative),)), scores[negative]))
    if gold < n:
        yes_loss = mx.logaddexp(mx.array(0.), -scores[gold])
        binary = .5 * (yes_loss + no_loss)
    else:
        binary = no_loss
    hardest = mx.maximum(mx.max(scores[negative]) + LOSS_CONFIG["negative_margin"], 0.)
    return ce + LOSS_CONFIG["binary_weight"] * binary + LOSS_CONFIG["hard_negative_weight"] * hardest


def loss(model, features):
    n = int(features["count"].item())
    scores = mx.stack([model.score_features(features[f"hidden{i}"], features["labels"])[0]
                       @ mx.array([1., -1.]) for i in range(n)])
    return score_loss(scores, int(features["gold"].item()))
