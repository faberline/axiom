from ..schema import DecisionRequest

MAX_TOKENS=1536


def prompt(value,tokenizer):
    r=DecisionRequest.from_dict(value)
    options="\n\n".join(f"{chr(65+i)}: {c.text}" for i,c in enumerate(r.candidates))
    user=f"STATE:\n{r.state}\n\nQUESTION:\n{r.question}\n\nCANDIDATES:\n{options}\n\nReturn only the letter of the best candidate."
    tokens=tokenizer.apply_chat_template([
        {"role":"system","content":"You are a careful Python and JavaScript code reviewer. Choose the replacement that satisfies every stated requirement. If none satisfies the requirement, choose the human review option when supplied. Return one candidate letter only."},
        {"role":"user","content":user}],tokenize=True,add_generation_prompt=True,enable_thinking=False,return_dict=False)
    if not isinstance(tokens,list) or not all(isinstance(t,int) for t in tokens):
        raise ValueError("Tokenizer must return a flat list of token IDs.")
    if len(tokens)>MAX_TOKENS:
        raise ValueError(f"Input has {len(tokens)} tokens; maximum is {MAX_TOKENS}. No text was truncated.")
    letters=[tokenizer.encode(chr(65+i),add_special_tokens=False) for i in range(len(r.candidates))]
    if any(len(x)!=1 for x in letters):
        raise ValueError("Modern selector needs single-token letter labels.")
    return tokens,[x[0] for x in letters]
