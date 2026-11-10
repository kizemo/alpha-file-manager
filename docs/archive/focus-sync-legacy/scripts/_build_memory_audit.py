"""Build _memory-audit.csv from file-history snapshots.

Output columns:
  hash, latest_mtime, total_versions, distinct_session_dirs,
  has_frontmatter, name, type, latest_size, latest_version,
  recommended_project, confidence, sample_preview

Usage:
  python scripts/_build_memory_audit.py > _memory-audit.csv
"""
import os, sys, io, csv
from pathlib import Path
from collections import defaultdict

sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8', errors='replace')

FH = Path.home() / '.claude' / 'file-history'

# Aggregate by hash
by_hash = defaultdict(lambda: {
    'files': [],
    'session_uuids': set(),
})
for d in FH.iterdir():
    if not d.is_dir(): continue
    for f in d.iterdir():
        if not f.is_file(): continue
        name = f.name
        if '@' not in name: continue
        h, _, rest = name.partition('@')
        ver = rest.split('-')[0]
        try:
            n = int(ver[1:])
        except:
            continue
        by_hash[h]['files'].append((f, n))
        by_hash[h]['session_uuids'].add(d.name)


def is_binary(text):
    """Crude heuristic: more than 5% non-printable chars."""
    if not text:
        return False
    sample = text[:4096]
    non_printable = sum(1 for c in sample if ord(c) < 32 and c not in '\n\r\t')
    return non_printable > len(sample) * 0.05


def parse_frontmatter(text):
    """Return (name, type) or ('', '') if no frontmatter."""
    if not text.startswith('---'):
        return '', ''
    end = text.find('---', 3)
    if end < 0:
        return '', ''
    fm = text[3:end]
    name = ''
    typ = ''
    for line in fm.split('\n'):
        if line.startswith('name:'):
            name = line.split(':', 1)[1].strip()
        elif line.startswith('type:'):
            typ = line.split(':', 1)[1].strip()
    return name, typ


def recommend_project(name, typ, latest_path):
    """Heuristic mapping based on frontmatter + filename hints."""
    h = (name + ' ' + typ + ' ' + str(latest_path)).lower()
    if 'storedata' in h or 'sales-bi' in h or 'aiec' in h:
        return 'F-resourse_study', 'medium'
    if 'evidence' in h or 'm3_smoke' in h or 'nginx' in h:
        return 'F-resourse_study', 'medium'
    if 'jsterp' in h:
        return 'F-resourse_study', 'medium'
    if 'shop-overview' in h:
        return 'F-resourse_study', 'medium'
    if 'section' in h or 'long-doc' in h or 'ad-cut' in h:
        return 'F-resourse_study', 'low'
    if 'windows-sandbox' in h or 'rsync' in h or 'ssh' in h or '凭据' in h or 'python' in h:
        return 'global', 'high'  # generic feedback
    if typ == 'user':
        return 'global', 'high'
    if typ == 'feedback':
        return 'global', 'medium'
    if typ == 'reference':
        return 'global', 'medium'
    return 'unknown', 'none'


def main():
    w = csv.writer(sys.stdout, lineterminator='\n')
    w.writerow([
        'hash', 'latest_mtime', 'total_versions', 'distinct_session_dirs',
        'has_frontmatter', 'name', 'type', 'latest_size_B',
        'latest_version', 'recommended_project', 'confidence', 'sample_preview'
    ])
    # Sort by total versions desc
    items = sorted(by_hash.items(), key=lambda kv: -len(kv[1]['files']))
    for h, info in items:
        files = info['files']
        files.sort(key=lambda x: x[1])
        latest_fp, latest_v = files[-1]
        st = latest_fp.stat()
        try:
            text = latest_fp.read_text(encoding='utf-8', errors='replace')
        except:
            text = ''
        name, typ = parse_frontmatter(text)
        proj, conf = recommend_project(name, typ, latest_fp)
        if is_binary(text):
            preview = '(binary)'
        else:
            preview = text[:120].replace('\n', ' ').replace('\r', ' ')
        w.writerow([
            h,
            int(st.st_mtime),
            len(files),
            len(info['session_uuids']),
            'Y' if name else 'N',
            name,
            typ,
            st.st_size,
            f'v{latest_v}',
            proj,
            conf,
            preview,
        ])


if __name__ == '__main__':
    main()