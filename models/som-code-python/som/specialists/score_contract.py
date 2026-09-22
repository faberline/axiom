"""Parameter-free score composition shared by SOM candidate paths."""


def combine_candidate_and_prompt_logits(candidate_boundary, prompt_end):
    """Average shared Yes/No logits at two deterministic prompt positions.

    Both values come from the same language-model output projection.  The
    operation creates no trainable parameters and works for MLX arrays and
    small synthetic arrays used by contract tests.
    """
    if getattr(candidate_boundary, "shape", None) != getattr(prompt_end, "shape", None):
        raise ValueError("Candidate-boundary and prompt-end logits must have equal shapes.")
    return (candidate_boundary + prompt_end) * 0.5
