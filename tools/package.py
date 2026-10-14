#!/usr/bin/env python3
"""Create a userspace-only review artifact, not a legally cleared distribution."""
import argparse
import pathlib
import tarfile
ROOT = pathlib.Path(__file__).resolve().parents[1]
p = argparse.ArgumentParser()
p.add_argument('output', type=pathlib.Path)
p.add_argument('--bin-dir', type=pathlib.Path, default=ROOT / 'target/release')
a = p.parse_args()
files = [(a.bin_dir / n, 'bin/' + n) for n in ('rfv', 'rfvd', 'rfv-adapter-rtw88')]
files += [(ROOT / n, n) for n in ('LICENSE', 'LICENSING.md', 'README.md', 'IPC_SPECIFICATION.md', 'SECURITY.md', 'INSTALLATION_WIZARD.md')]
files += [(ROOT / 'docs/licensing/inventory.tsv', 'docs/licensing/inventory.tsv')]
for source, _ in files:
    if not source.is_file() or source.is_symlink():
        raise SystemExit(f'Missing or symlink input: {source}')
a.output.parent.mkdir(parents=True, exist_ok=True)
# Exclusive creation: never overwrite unrelated output.
with a.output.open('xb') as stream, tarfile.open(fileobj=stream, mode='w:gz') as archive:
    for source, name in files:
        archive.add(source, arcname='rfvision/' + name, recursive=False)
print('Created userspace-only review artifact. Legal/distribution clearance remains required.')
