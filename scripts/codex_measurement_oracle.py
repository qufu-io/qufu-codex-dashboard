import argparse
import json
import subprocess
import tempfile
from pathlib import Path

parser = argparse.ArgumentParser(description='Compile the official Codex TokenUsage methods to refresh measurement regression cases.')
parser.add_argument('source', type=Path)
parser.add_argument('--output', type=Path, default=Path('tests/fixtures/codex-measurements.json'))
args = parser.parse_args()
source = (args.source / 'codex-rs/protocol/src/protocol.rs').read_text(encoding='utf-8')

def method(name):
    start = source.index('    pub fn ' + name + '(')
    body = source.index('{', start)
    depth = 1
    end = body + 1
    while depth:
        depth += (source[end] == '{') - (source[end] == '}')
        end += 1
    return source[start:end]

methods = '\n'.join(method(n) for n in ['cached_input', 'non_cached_input', 'blended_total', 'tokens_in_context_window', 'percent_of_context_window_remaining'])
program = '''const BASELINE_TOKENS:i64=12000;
struct TokenUsage {input_tokens:i64,cached_input_tokens:i64,output_tokens:i64,total_tokens:i64}
impl TokenUsage {METHODS}
fn main(){
for capacity in [1000,12000,128000,272000,1000000] {
for used in [0,11999,12000,13000,58000,70000,127999,128000,200000,600000] {
let u=TokenUsage{input_tokens:1000,cached_input_tokens:800,output_tokens:200,total_tokens:used};
println!("{} {} {} {}",capacity,used,u.percent_of_context_window_remaining(capacity),u.blended_total());
}}}
'''.replace('METHODS', methods)
with tempfile.TemporaryDirectory(prefix='qufu-codex-oracle-') as temporary:
    root = Path(temporary)
    rust = root / 'oracle.rs'
    rust.write_text(program, encoding='utf-8')
    executable = root / ('oracle.exe' if __import__('os').name == 'nt' else 'oracle')
    subprocess.run(['rustc', '--edition=2021', str(rust), '-o', str(executable)], check=True)
    cases = []
    for line in subprocess.check_output([str(executable)], text=True).splitlines():
        capacity, used, remaining, cli_used = map(int, line.split())
        cases.append(dict(capacity=capacity, contextTokens=used, remainingPercent=remaining, cliUsed=cli_used))
sha = subprocess.check_output(['git', '-C', str(args.source), 'rev-parse', 'HEAD'], text=True).strip()
args.output.write_text(json.dumps({'source': 'https://github.com/openai/codex', 'commit': sha, 'method': 'compiled unmodified official TokenUsage methods', 'cases': cases}, indent=2) + '\n', encoding='utf-8')
print(f'{len(cases)} cases from official Codex {sha}')
