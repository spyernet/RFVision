#!/usr/bin/env python3
"""Save actual read-only reports; never simulate data or reconfigure networking."""
import json
import os
from pathlib import Path
import subprocess
import sys
if len(sys.argv) != 3:
    raise SystemExit('usage: collect_report.py CORE_SOCKET NEW_OUTPUT_DIRECTORY')
root = Path(__file__).resolve().parents[2]
binary = root / 'target/release/rfv'
output = Path(sys.argv[2])
os.umask(0o077)
output.mkdir(mode=0o700, parents=False, exist_ok=False)
results = []
for name, args in [('discovery', ['discover']), ('assessment', ['wizard', 'assess', sys.argv[1]]), ('validation', ['wizard', 'validate', sys.argv[1]])]:
    p = subprocess.run([str(binary), *args], text=True, capture_output=True, timeout=360)
    result = {'command': args, 'exit_code': p.returncode, 'stdout': p.stdout, 'stderr': p.stderr,
              'evidence_type': 'actual_host_observation_not_independently_validated_RF'}
    (output / f'{name}.json').write_text(json.dumps(result, indent=2) + '\n')
    results.append(p.returncode)
print('Saved private reports. RF accuracy and connectivity require independent verification.')
raise SystemExit(0 if all(code == 0 for code in results) else 2)
