"""Verify delivered CLI results and completed-run resume without updating weights."""
import json
import math
import re
import subprocess
import sys

from ..paths import ROOT, read_json, write_json, resolve_checkpoint, sha256
from .next_train import RUN, START, START_SHA, verify_protocol


def verify():
    verify_protocol(RUN)
    checkpoint = resolve_checkpoint(RUN)
    report_path = RUN / read_json(RUN / 'evaluation.json')['report']
    report = read_json(report_path)
    policy = read_json(checkpoint / 'developer_policy.json')
    assert report['adapter_sha256'] == policy['adapter_sha256'] == sha256(checkpoint / 'adapter.safetensors')
    assert policy == report['policy']
    tests = (ROOT / 'reports/next-full-tests.txt').read_text()
    match = re.search(r'(\d+) passed in ([\d.]+)s', tests)
    assert match and int(match[1]) >= 50
    results = {}
    for domain in ('python', 'frontend'):
        path = ROOT / f'examples/developer-{domain}.json'
        completed = subprocess.run([sys.executable, '-m', 'som', 'next-predict', '--domain', domain,
                                    '--input', str(path)], cwd=ROOT, check=True, capture_output=True, text=True)
        value = json.loads(completed.stdout)
        expected_ids = {c['id'] for c in read_json(path)['candidates']} | {'__review__'}
        assert {c['id'] for c in value['candidates']} == expected_ids
        assert all(math.isfinite(c['score']) and math.isfinite(c['probability']) and 0 <= c['probability'] <= 1 for c in value['candidates'])
        assert abs(sum(c['probability'] for c in value['candidates']) - 1) < 1e-6
        top = max(value['candidates'], key=lambda c: c['score'])['id']
        assert value['suggested_choice_id'] == (None if top == '__review__' else top)
        if not policy['acceptance_passed']:
            assert value['status'] == 'human_review' and value['choice_id'] is None
            assert 'developer_acceptance_gates_not_met' in value['review_reasons']
        write_json(ROOT / f'reports/next-{domain}-example.json', value)
        results[domain] = {k: value[k] for k in ('status', 'suggested_choice_id', 'confidence')}
    before = read_json(ROOT / 'reports/next-resume-before.json')['hashes']
    assert all(sha256(ROOT / p) == digest for p, digest in before.items())
    completed = subprocess.run([sys.executable, '-m', 'som', 'next-train', '--resume'], cwd=ROOT,
                               check=True, capture_output=True, text=True)
    resumed = json.loads(completed.stdout)
    assert resumed == {'status': 'completed', 'unchanged': True}
    after = {p: sha256(ROOT / p) for p in before}
    assert before == after and sha256(START / 'adapter.safetensors') == START_SHA
    write_json(ROOT / 'reports/next-resume-check.json', {'result': resumed, 'before': before, 'after': after})
    result = {'tests': {'passed': int(match[1]), 'seconds': float(match[2]), 'gpu_included': True,
                        'log_sha256': sha256(ROOT / 'reports/next-full-tests.txt')},
              'examples': results, 'completed_resume_unchanged': True, 'parent_v4_adapter_unchanged': True,
              'checkpoint': str(checkpoint), 'adapter_sha256': report['adapter_sha256'],
              'main_report_sha256': sha256(report_path), 'model_status': report['status']}
    write_json(ROOT / 'reports/next-final-verification.json', result)
    print(json.dumps(result), flush=True)
    return result


if __name__ == '__main__':
    verify()
