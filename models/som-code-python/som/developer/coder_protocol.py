from ..paths import ROOT, read_json, sha256, write_json
from .final_cases import FINAL

CRITERIA={"seen_accuracy":(.85,"min"),"fresh_family_accuracy":(.80,"min"),
          "accepted_fresh_error_upper":(.10,"max"),"accepted_fresh_count":(35,"min"),
          "accepted_fresh_coverage":(.25,"min"),"fresh_order_consistency":(.95,"min"),
          "fresh_missing_fix_review_recall":(.85,"min")}


def freeze(run):
    value={"criteria":{k:list(v) for k,v in CRITERIA.items()},"fresh_manifest_sha256":sha256(FINAL/"manifest.json"),
           "selection":"Validation only; final cases and acceptance scores do not select or train weights.",
           "claim_limit":"A bounded pilot on authored function-level repairs, not general repository readiness."}
    path=run/"acceptance-protocol.json"
    if path.exists() and read_json(path)!=value:
        raise ValueError("Frozen coder acceptance protocol changed.")
    write_json(path,value)


def verify(run):
    value=read_json(run/"acceptance-protocol.json")
    if value["criteria"]!={k:list(v) for k,v in CRITERIA.items()} or value["fresh_manifest_sha256"]!=sha256(FINAL/"manifest.json"):
        raise ValueError("Coder acceptance protocol or final data changed.")
