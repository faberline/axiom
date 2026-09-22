from som.paths import ROOT, read_json, read_rows
from som.developer.next_holdout import DATA, rows


def test_next_holdout_is_unused_and_oracle_labeled():
    values=rows(); manifest=read_json(DATA/'manifest.json')
    used=set(read_json(ROOT/'data/humanevalpack-v3/manifest.json')['problem_ids'])
    used.update(read_json(ROOT/'data/ranker-v4-final/manifest.json')['problem_ids'])
    assert len(used)==80 and len(values)==192
    assert set(manifest['excluded_v3_v4_ids'])==used
    assert len(set(manifest['problem_ids']))==32 and not used&set(manifest['problem_ids'])
    development=[r for s in ('train','validation','calibration','seen')
                 for r in read_rows(ROOT/f'data/ranker-v4/{s}.jsonl')]
    for key in ('id','state','family'):
        assert not {r[key] for r in values}&{r[key] for r in development}
    for case in read_json(DATA/'cases.json'):
        assert [o['passed'] for o in case['oracle']]==[True,False,False]
        assert all(o['valid_syntax'] for o in case['oracle'])
