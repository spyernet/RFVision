#!/usr/bin/env python3
"""Inventory supplied source without building/importing it. stdlib only."""
import argparse
import hashlib
import json
import pathlib
import re
import sys
ROOT = pathlib.Path(__file__).resolve().parents[1]
MANIFEST = ROOT / 'docs/drivers/rtw88-source-manifest.json'

def inventory():
    files = []
    for p in sorted((ROOT / 'drv/rtw88').iterdir()):
        if not p.is_file():
            continue
        data = p.read_bytes()
        spdx = re.search(r'SPDX-License-Identifier:\s*(.*?)(?:\s*\*/)?$', data.decode().splitlines()[0])
        files.append({'path': str(p.relative_to(ROOT)), 'sha256': hashlib.sha256(data).hexdigest(),
                      'license_declared': spdx.group(1) if spdx else 'UNRESOLVED'})
    return files

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--check', action='store_true')
    args = ap.parse_args()
    data = inventory()
    if args.check:
        assert json.loads(MANIFEST.read_text())['files'] == data, 'source differs from audited manifest'
        print(f'PASS: {len(data)} supplied files unchanged; declarations checked (not legal clearance)')
    else:
        manifest = {'supplied_repository_commit': 'f7c9b55',
                    'supplied_tree': 'b0ffbf222400f28c13ba9bd30e3c6399daaf28f1',
                    'upstream_repository': 'https://github.com/torvalds/linux',
                    'upstream_reference': '4056b3c9d3c92ea45955641dbafeb9b960e146ef',
                    'upstream_path': 'drivers/net/wireless/realtek/rtw88',
                    'comparison': 'All 98 Git blob IDs matched pinned upstream directory during research; upstream comparison is not repeated by --check.',
                    'files': data}
        MANIFEST.write_text(json.dumps(manifest, indent=2) + '\n')
        print(f'Wrote {MANIFEST.relative_to(ROOT)}')
if __name__ == '__main__':
    main()
