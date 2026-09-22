import mlx.core as mx

LOSS_CONFIG={"binary_weight":.5,"teacher_kl_weight":.05}


def loss(model,features):
    n=int(features["count"].item()); gold=int(features["gold"].item())
    scores=mx.stack([(model.score_features(features[f"hidden{i}"],features["labels"])[0] @ mx.array([1.,-1.])) for i in range(n)])
    logits=mx.concatenate([scores,mx.zeros((1,))])
    ce=mx.logsumexp(logits)-logits[gold]
    truth=mx.array([float(i==gold) for i in range(n)])
    binary=mx.mean(mx.logaddexp(mx.zeros_like(scores),scores)-truth*scores)
    prior=features["prior"]
    teacher_yes=mx.sigmoid(prior)
    lt_yes=-mx.logaddexp(mx.zeros_like(prior),-prior)
    lt_no=-mx.logaddexp(mx.zeros_like(prior),prior)
    lp_yes=-mx.logaddexp(mx.zeros_like(scores),-scores)
    lp_no=-mx.logaddexp(mx.zeros_like(scores),scores)
    kl=mx.mean(teacher_yes*(lt_yes-lp_yes)+(1-teacher_yes)*(lt_no-lp_no))
    return ce+LOSS_CONFIG["binary_weight"]*binary+LOSS_CONFIG["teacher_kl_weight"]*kl


def features_for_row(model,tokenizer,row):
    from .ranker_input import prompt,check_request
    from .data import REVIEW_ID
    request,_=check_request(row,tokenizer)
    codes=sorted([c for c in request.candidates if c.id!=REVIEW_ID],key=lambda c:(c.text,c.id))
    gold=next((i for i,c in enumerate(codes) if c.id==row["gold_candidate_id"]),len(codes))
    values={}; priors=[]
    for i,c in enumerate(codes):
        tokens,labels=prompt(request.state,request.question,c.text,tokenizer)
        hidden=model.prefix_features(mx.array([tokens]))
        z=model.score_features(hidden,labels)[0]
        mx.eval(hidden,z); values[f"hidden{i}"]=hidden; priors.append(float((z[0]-z[1]).item()))
    values.update({"prior":mx.array(priors),"labels":mx.array(labels),"count":mx.array(len(codes)),"gold":mx.array(gold)})
    return values
