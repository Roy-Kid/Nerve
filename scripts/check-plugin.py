#!/usr/bin/env python3
"""Reject files that cannot be imported in the Codex desktop plugin package."""
import pathlib
import sys
sys.dont_write_bytecode = True
root = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else pathlib.Path(__file__).resolve().parents[1] / 'plugins/nerve'
forbidden_dirs = {'__pycache__', '.pytest_cache', 'node_modules', '.git', '.DS_Store'}
forbidden_suffixes = {'.pyc', '.pyo', '.so', '.dylib', '.dll', '.exe', '.o'}
errors = [str(p.relative_to(root)) for p in root.rglob('*') if p.name in forbidden_dirs or p.suffix in forbidden_suffixes]
for entry in ('.codex-plugin/plugin.json', 'hooks/codex.json', 'hooks/nerve.py'):
    if not (root / entry).is_file(): errors.append('missing ' + entry)
if errors:
    print('Plugin package rejected:\n' + '\n'.join(errors), file=sys.stderr)
    sys.exit(1)
print('Plugin package OK: no forbidden files')
