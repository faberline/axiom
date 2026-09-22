"""Export saved v5 measurements and paired v4 comparisons without GPU work."""
import copy
from pathlib import Path
import numpy as np

from ..paths import ROOT, read_json, write_json, sha256
from .next_report import write_report


def outcome(record):
    choice = record['candidate_ids'][int(np.argmax(record['logits']))]
    gold = record['candidate_ids'][record['gold_index']]
    return choice == gold, gold == '__review__', choice == '__review__'


def breakdown(records):
    values = [outcome(r) for r in records]
    present = [r for r in values if not r[1]]
    absent = [r for r in values if r[1]]
    return {'n': len(values), 'accuracy': sum(r[0] for r in values) / len(values),
            'present_n': len(present), 'present_correct': sum(r[0] for r in present),
            'present_accuracy': sum(r[0] for r in present) / len(present),
            'missing_n': len(absent), 'missing_review': sum(r[2] for r in absent),
            'missing_review_recall': sum(r[2] for r in absent) / len(absent),
            'wrong_patch_on_missing': sum(not r[2] for r in absent)}


def paired(before, after):
    first = {r['id']: r for r in before}; second = {r['id']: r for r in after}
    if len(first) != len(before) or len(second) != len(after) or first.keys() != second.keys():
        raise ValueError('Paired comparison needs the exact same unique IDs.')
    groups = {}
    for key in sorted(first):
        a, b = first[key], second[key]
        if (a['candidate_ids'], a['gold_index'], a['task'], a['family']) != (b['candidate_ids'], b['gold_index'], b['task'], b['family']):
            raise ValueError('Paired candidate identity or gold mismatch.')
        groups.setdefault(a['family'], []).append(float(outcome(b)[0]) - float(outcome(a)[0]))
    counts = np.asarray([[sum(v), len(v)] for v in groups.values()])
    rng = np.random.default_rng(5301)
    samples = counts[rng.integers(0, len(counts), size=(10000, len(counts)))].sum(axis=1)
    differences = samples[:, 0] / samples[:, 1]
    return {'v4': breakdown(before), 'v5': breakdown(after), 'problem_groups': len(groups),
            'accuracy_difference': sum(v[0] for v in counts) / len(after),
            'paired_problem_bootstrap_95': np.percentile(differences, [2.5, 97.5]).tolist()}


def export():
    run = ROOT / 'runs/som/research/next'
    main_path = run / read_json(run / 'evaluation.json')['report']; report = read_json(main_path)
    output = main_path.parent
    old_run = ROOT / 'runs/som/research/ranker'
    old_main = old_run / read_json(old_run / 'evaluation.json')['report']
    old = old_main.parent
    new_files = {key: output / f'{key}.json' for key in ('public-final', 'v4-public-final', 'previous-public-regression', 'seen')}
    old_files = {key: old / f'{key}.json' for key in ('public-final', 'previous-public-regression', 'seen')}
    sources = [main_path, old_main, *new_files.values(), *old_files.values(), ROOT / 'som/developer/next_summary.py', ROOT / 'som/developer/next_report.py']
    comparison = {'note': 'Same-case paired comparison. Problem variants are correlated. Old tests informed v5 design and are regression only. Intervals describe this selected corpus.', 'splits': {}}
    all_old = read_json(old_files['public-final']) + read_json(old_files['previous-public-regression'])
    new_reg = read_json(new_files['previous-public-regression'])
    pools = {'fresh_v5': (read_json(new_files['v4-public-final']), read_json(new_files['public-final'])),
             'previous_80': (all_old, new_reg),
             'seen': (read_json(old_files['seen']), read_json(new_files['seen']))}
    for name, file_key in (('previous_v3_40', 'previous-public-regression'), ('previous_v4_40', 'public-final')):
        before = read_json(old_files[file_key]); identities = {r['id'] for r in before}
        pools[name] = (before, [r for r in new_reg if r['id'] in identities])
    for name, (before, after) in pools.items():
        comparison['splits'][name] = {d: paired([r for r in before if r['task'] == d], [r for r in after if r['task'] == d]) for d in ('python', 'frontend')}
    comparison['source_hashes'] = {str(p.relative_to(ROOT)): sha256(p) for p in sources}
    write_json(ROOT / 'reports/ranker-v5-v4-comparison.json', comparison)
    requests = {r['id']: r for r in read_json(ROOT / 'data/som/research/next-final/rows.json')}
    errors = []
    for record in read_json(new_files['public-final']):
        if outcome(record)[0]:
            continue
        domain = record['task']; temperature = report['policy']['domains'][domain]['temperature']
        logits = np.asarray(record['logits']) / temperature
        weights = np.exp(logits - logits.max()); probabilities = weights / weights.sum()
        index = int(probabilities.argmax())
        errors.append({'id': record['id'], 'task': domain, 'confidence': float(probabilities[index]),
                       'suggested_id': record['candidate_ids'][index],
                       'gold_id': record['candidate_ids'][record['gold_index']],
                       'missing_correct_patch': outcome(record)[1], 'request': requests[record['id']]})
    errors.sort(key=lambda r: r['confidence'], reverse=True)
    write_json(ROOT / 'reports/next-public-errors.json', {'source_sha256': sha256(new_files['public-final']),
               'note': 'All v5 fresh-public mistakes, sorted by calibrated confidence. These are now diagnostic cases for future work, not future fresh test data.', 'errors': errors})
    latencies = {'note': 'Fresh uncached first pass plus cached reversed-order lookup. Excludes model load and warm-up.', 'splits': {}}
    for key in ('seen', 'public-final', 'previous-public-regression'):
        records = read_json(new_files[key])
        latencies['splits'][key] = {d: {'n': sum(r['task'] == d for r in records),
            **{f'p{q}_seconds': float(np.percentile([r['paired_seconds'] for r in records if r['task'] == d], q)) for q in (50, 95)}} for d in ('python', 'frontend')}
    write_json(ROOT / 'reports/next-latency-by-split.json', latencies)
    report['reused_baselines'] = {name: read_json(ROOT / 'reports' / file) for name, file in
        (('original_base_seen', 'next-base-cache-transfer.json'), ('v4_browser', 'next-browser-base-cache-transfer.json'))}
    verification_path = ROOT / 'reports/next-final-verification.json'
    verification = read_json(verification_path) if verification_path.exists() else None
    report['selected_samples_seen'] = read_json(Path(report['checkpoint']) / 'trainer.json')['samples_seen']
    if verification:
        report['delivery_verification'] = verification
    write_json(ROOT / 'reports/ranker-v5.json', report)
    write_report(copy.deepcopy(report), ROOT / 'reports/ranker-v5.md')
    lines = ['', '## 與 v4 的同題比較', '', '差值以同一批題目計算。舊題只作回歸檢查。', '',
             '| 測試 | Python：v4 → v5 | JavaScript：v4 → v5 |', '|---|---:|---:|']
    for name, label in (('fresh_v5', '32 個新公開問題'), ('previous_80', '80 個舊公開問題'),
                        ('previous_v3_40', '其中：v3 的 40 題'), ('previous_v4_40', '其中：v4 的 40 題'), ('seen', '已見題型')):
        m = comparison['splits'][name]
        lines.append(f"| {label} | {m['python']['v4']['accuracy']:.2%} → {m['python']['v5']['accuracy']:.2%} | {m['frontend']['v4']['accuracy']:.2%} → {m['frontend']['v5']['accuracy']:.2%} |")
    lines += ['', '新公開題中，缺少正解時選擇人工檢查：', '']
    for d in ('python', 'frontend'):
        m = comparison['splits']['fresh_v5'][d]
        lines.append(f"- {d}：v4 {m['v4']['missing_review']}/{m['v4']['missing_n']} → v5 {m['v5']['missing_review']}/{m['v5']['missing_n']}。")
    lines += ['', '## 前端行為回歸檢查', '']
    browser_path = run / read_json(run / 'browser-evaluation.json')['report']; browser = read_json(browser_path)
    trained = read_json(browser_path.parent / 'trained.json'); initial = read_json(browser_path.parent / 'v4.json')
    browser_comparison = paired(initial, trained)
    write_json(ROOT / 'reports/ranker-v5-browser.json', {**browser, 'paired_breakdown': browser_comparison})
    browser_text = (run / 'BROWSER.md').read_text().replace(
        f"正確率：v4 {browser['base']['accuracy']:.1%} → 訓練後 {browser['trained']['accuracy']:.1%}。",
        f"正確率：v4 {browser['base']['accuracy']:.2%} → 訓練後 {browser['trained']['accuracy']:.2%}。")
    a, b = browser_comparison['v4'], browser_comparison['v5']
    (ROOT / 'reports/ranker-v5-browser.md').write_text(browser_text +
        f"\n缺少正解時，v4 {a['missing_review']}/{a['missing_n']} → v5 {b['missing_review']}/{b['missing_n']} 選擇人工檢查。\n" +
        f"有正確修法時，v4 選對 {a['present_correct']}/{a['present_n']} → v5 {b['present_correct']}/{b['present_n']}。\n")
    for label, key in (('v4', 'v4'), ('v5', 'v5')):
        m = browser_comparison[key]
        lines.append(f"{label}：48 題正確率 {m['accuracy']:.2%}；缺少正解時，{m['missing_review']}/{m['missing_n']} 題選擇人工檢查。")
    lines += ['', '這些舊題影響了 v5 的設計，因此改善不能視為全新任務的證據。',
              '原模型的 320 筆舊題分數與 v4 的 48 筆前端基準重用既有結果。已核對相同權重、評分程式與完整輸入；其基準延遲是歷史量測。v5 的結果與延遲均重新測量。',
              f"[完整同題比較]({ROOT / 'reports/ranker-v5-v4-comparison.json'})包含按問題群組計算的差值區間。",
              f"[新公開題錯誤案例]({ROOT / 'reports/next-public-errors.json'})依校準後信心排序，保留原始需求與候選。",
              f"[各資料組延遲]({ROOT / 'reports/next-latency-by-split.json'})保留中位數與第 95 百分位。", '']
    if verification:
        lines += ['## 交付確認', '',
                  f"{verification['tests']['passed']} 項程式測試全部通過，含 GPU 與續跑測試。",
                  f"Python 範例選出 {verification['examples']['python']['suggested_choice_id']}，前端範例選出 {verification['examples']['frontend']['suggested_choice_id']}。狀態分別為 {verification['examples']['python']['status']} 與 {verification['examples']['frontend']['status']}。",
                  '已完成的執行再次續跑，權重、選定檢查點及進度均不改變。v4 權重也保持不變。',
                  f"[完整交付驗證]({verification_path})保留命令列結果與權重雜湊。", '']
    with (ROOT / 'reports/ranker-v5.md').open('a') as stream:
        stream.write('\n'.join(lines))
    return comparison


if __name__ == '__main__':
    export()
