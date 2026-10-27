import argparse
import json
import math
import os
import sqlite3
from pathlib import Path

parser = argparse.ArgumentParser(description='Read-only comparison of dashboard diagnostics with official local Codex measurement records. Run against idle sessions for a stable comparison.')
parser.add_argument('diagnostics', type=Path)
parser.add_argument('--codex-home', type=Path, default=Path(os.environ.get('CODEX_HOME', Path.home() / '.codex')))
parser.add_argument('--output', type=Path)
args = parser.parse_args()
report = json.loads(args.diagnostics.read_text(encoding='utf-8'))
databases = list(args.codex_home.glob('state_*.sqlite'))
results = []

def path_for(thread_id):
    for file in databases:
        with sqlite3.connect(file.resolve().as_uri() + '?mode=ro', uri=True) as db:
            try:
                row = db.execute('SELECT rollout_path FROM threads WHERE id=?', (thread_id,)).fetchone()
            except sqlite3.OperationalError:
                continue
            if row and Path(row[0]).is_file():
                return Path(row[0])
    return None

def compare(actual, expected):
    fields = dict(input='input_tokens', cache='cached_input_tokens', output='output_tokens', reasoning='reasoning_output_tokens')
    if actual is None:
        return False
    return all(actual.get(key) == expected[field] for key, field in fields.items() if field in expected)

def measured_lines(path, length):
    with path.open('rb') as f:
        remaining = length
        while remaining > 0:
            line = f.readline(remaining)
            if not line:
                break
            remaining -= len(line)
            yield line.decode('utf-8')

for detail in report.get('details', []):
    thread = detail['thread']['id']
    path = path_for(thread)
    if path is None:
        results.append(dict(thread=thread, skipped='rollout unavailable'))
        continue
    info = None
    turn_snapshots = {}
    model = None
    capacity = None
    used = None
    seen = set()
    before = path.stat()
    for line in measured_lines(path, detail.get('rolloutBytes', before.st_size)):
        if not any(kind in line for kind in ['"token_count"', '"token_usage_record"', '"compacted"', '"turn_context"', '"task_started"', '"thread_settings_applied"']):
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        payload = event.get('payload') or {}
        kind = event.get('type')
        if kind == 'compacted':
            used = None
        elif kind == 'turn_context':
            if model is not None and model != payload.get('model'):
                used = None
            model = payload.get('model')
        elif kind == 'event_msg':
            name = payload.get('type')
            if name == 'thread_settings_applied':
                new_model = (payload.get('thread_settings') or {}).get('model')
                if new_model is not None and new_model != model:
                    used = None
                    model = new_model
            if name in ['task_started', 'turn_started']:
                new_capacity = payload.get('model_context_window')
                if capacity != new_capacity:
                    used = None
                capacity = new_capacity
            if name == 'token_count' and isinstance(payload.get('info'), dict):
                info = payload['info']
                capacity = info.get('model_context_window')
                measured = (info.get('last_token_usage') or {}).get('total_tokens')
                if measured is not None:
                    used = measured
        elif kind == 'token_usage_record' and payload.get('thread_id') == thread:
            key = payload.get('response_id')
            if key and key in seen:
                continue
            if key:
                seen.add(key)
            if isinstance(payload.get('turn_token_usage'), dict):
                turn_snapshots[payload.get('turn_id')] = payload['turn_token_usage']
    after = path.stat()
    result = dict(thread=thread)
    if (before.st_size, before.st_mtime_ns) != (after.st_size, after.st_mtime_ns):
        result['skipped'] = 'rollout changed while reading'
        results.append(result)
        continue
    official = (info or {}).get('total_token_usage')
    if official and detail.get('cliUsage') is not None:
        result['officialSessionSnapshot'] = compare(detail['cliUsage'], official)
        if detail.get('usageSource') == 'Codex token_count.info.total_token_usage':
            result['displayedSession'] = compare(detail['usage'], official)
            result['rawTotal'] = detail.get('rawTotal') == official.get('input_tokens', 0) + official.get('output_tokens', 0)
            if 'cached_input_tokens' in official:
                result['cliUsed'] = detail.get('used') == max(0, official.get('input_tokens', 0) - max(0, official['cached_input_tokens'])) + max(0, official.get('output_tokens', 0))
    task = detail.get('turn') or {}
    if task.get('id') in turn_snapshots:
        result['officialTurnSnapshot'] = compare(detail.get('lastUsage'), turn_snapshots[task['id']])
    if used is not None and isinstance(capacity, int) and capacity > 0:
        remaining = 0 if capacity <= 12000 else math.floor(max(0, min(100, max(0, capacity - 12000 - max(used - 12000, 0)) / (capacity - 12000) * 100)) + 0.5)
        actual = detail.get('context') or {}
        result['context'] = actual.get('used') == used and actual.get('capacity') == capacity and actual.get('percent') == 100 - remaining
    results.append(result)

checks = [(r['thread'], key, value) for r in results for key, value in r.items() if isinstance(value, bool)]
output = dict(sessions=len(results), checks=len(checks), passed=sum(v for _, _, v in checks), failed=[dict(thread=t, field=k) for t, k, v in checks if not v], results=results)
if args.output:
    args.output.write_text(json.dumps(output, indent=2) + '\n', encoding='utf-8')
print(json.dumps({key: output[key] for key in ['sessions', 'checks', 'passed', 'failed']}, indent=2))
raise SystemExit(1 if output['failed'] else 0)
