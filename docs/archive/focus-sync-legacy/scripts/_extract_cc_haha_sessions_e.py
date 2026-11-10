"""Extract cc-haha trace jsonl files from E: drive into per-session markdown summaries.

Hard-coded E: paths. Run after verifying E: is the active primary home.
"""
import os, sys, io, datetime, json, re, shutil
from pathlib import Path

sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8', errors='replace')

TRACES = Path(r'E:\Users\Duanyi\.claude\cc-haha\traces')
OUT_DIR = Path(r'F:\soft\00selfmade\filemanager\docs\legacy-sessions-e')

OUT_DIR.mkdir(parents=True, exist_ok=True)
if OUT_DIR.exists():
    shutil.rmtree(OUT_DIR)
OUT_DIR.mkdir(parents=True, exist_ok=True)


def extract_session(fp):
    """Parse cc-haha trace jsonl, return summary dict."""
    info = {
        'sid': fp.stem,
        'size_kb': fp.stat().st_size // 1024,
        'mtime': fp.stat().st_mtime,
        'model': '',
        'provider': '',
        'started_at': '',
        'last_at': '',
        'api_calls': 0,
        'first_user_msg': '',
        'last_assistant_msg': '',
        'user_msgs': [],
        'assistant_msgs': [],
        'tool_uses': [],
    }
    last_ts = ''
    with open(fp, 'r', encoding='utf-8', errors='replace') as fh:
        for line in fh:
            line = line.strip()
            if not line:
                continue
            try:
                obj = json.loads(line)
            except Exception:
                continue
            if not isinstance(obj, dict):
                continue
            t = obj.get('type', '')
            rec = obj.get('record') if 'record' in obj else obj
            ev = obj.get('event') if 'event' in obj else None
            if t == 'call' and isinstance(rec, dict):
                info['api_calls'] += 1
                if not info['model']:
                    info['model'] = rec.get('model', '')
                if not info['provider']:
                    prov = rec.get('provider', {})
                    if isinstance(prov, dict):
                        info['provider'] = prov.get('name', '')
                ts = rec.get('startedAt') or rec.get('completedAt') or ''
                if ts and not info['started_at']:
                    info['started_at'] = ts
                if ts:
                    last_ts = ts
                sem = rec.get('request', {}).get('semantic', {})
                sem_req = sem.get('request', {}) if isinstance(sem, dict) else None
                if isinstance(sem_req, dict):
                    msgs = sem_req.get('messages', [])
                    for m in msgs:
                        if not isinstance(m, dict):
                            continue
                        role = m.get('role')
                        content = m.get('content')
                        if role == 'user' and isinstance(content, list):
                            texts = []
                            for c in content:
                                if isinstance(c, dict) and c.get('type') == 'text':
                                    texts.append(c.get('text', ''))
                            if texts:
                                joined = '\n'.join(texts)
                                cleaned = re.sub(r'<system-reminder>.*?</system-reminder>', '', joined, flags=re.DOTALL).strip()
                                if cleaned:
                                    info['user_msgs'].append(cleaned[:800])
                        elif role == 'assistant' and isinstance(content, list):
                            texts = []
                            for c in content:
                                if isinstance(c, dict) and c.get('type') == 'text':
                                    texts.append(c.get('text', ''))
                            if texts:
                                joined = '\n'.join(texts).strip()
                                if joined:
                                    info['assistant_msgs'].append(joined[:800])
                        elif role == 'assistant' and isinstance(content, str):
                            if content.strip():
                                info['assistant_msgs'].append(content[:800])
                        if isinstance(content, list):
                            for c in content:
                                if isinstance(c, dict) and c.get('type') == 'tool_use':
                                    info['tool_uses'].append(
                                        f"{c.get('name','?')}({json.dumps(c.get('input',{}), ensure_ascii=False)[:200]})"
                                    )
            elif t == 'event' and isinstance(ev, dict):
                ts = ev.get('timestamp') or ''
                if ts:
                    last_ts = ts
    if last_ts:
        info['last_at'] = last_ts
    info['first_user_msg'] = info['user_msgs'][0] if info['user_msgs'] else ''
    info['last_assistant_msg'] = info['assistant_msgs'][-1] if info['assistant_msgs'] else ''
    return info


def render_md(info):
    dt = datetime.datetime.fromtimestamp(info['mtime'])
    date_str = dt.strftime('%Y-%m-%d')
    time_str = dt.strftime('%H:%M')
    lines = [
        f"# cc-haha session {info['sid'][:8]}",
        '',
        f"- **Date**: {date_str} {time_str}",
        f"- **Size**: {info['size_kb']:,} KB",
        f"- **Model**: `{info['model']}`",
        f"- **Provider**: `{info['provider']}`",
        f"- **API calls**: {info['api_calls']}",
        f"- **Started**: {info['started_at']}",
        f"- **Last event**: {info['last_at']}",
        f"- **User messages**: {len(info['user_msgs'])}",
        f"- **Assistant messages**: {len(info['assistant_msgs'])}",
        f"- **Tool uses**: {len(info['tool_uses'])}",
        '',
        '## First user message',
        '',
        '```',
        info['first_user_msg'][:600] or '(empty)',
        '```',
        '',
        '## Last assistant response',
        '',
        '```',
        info['last_assistant_msg'][:600] or '(empty)',
        '```',
        '',
    ]
    if info['tool_uses']:
        lines += ['## Tool uses (sample, max 20)', '']
        for t in info['tool_uses'][:20]:
            lines.append(f"- `{t}`")
        lines.append('')
    return '\n'.join(lines)


def main():
    candidates = []
    for f in TRACES.iterdir():
        if not f.name.endswith('.jsonl'):
            continue
        sz = f.stat().st_size
        if sz < 100000:
            continue
        mt = f.stat().st_mtime
        d = datetime.datetime.fromtimestamp(mt).date()
        if not (datetime.date(2026, 9, 20) <= d <= datetime.date(2026, 9, 28)):
            continue
        candidates.append((mt, sz, f))
    candidates.sort()
    print(f"Processing {len(candidates)} sessions from E: ...", file=sys.stderr)
    index_entries = []
    for mt, sz, fp in candidates:
        info = extract_session(fp)
        dt = datetime.datetime.fromtimestamp(mt)
        date_str = dt.strftime('%Y-%m-%d')
        out_name = f"{date_str}-{info['sid'][:8]}.md"
        out_path = OUT_DIR / out_name
        out_path.write_text(render_md(info), encoding='utf-8')
        first_msg = info['first_user_msg'][:80].replace('\n', ' ').replace('\r', '')
        index_entries.append({
            'date': date_str,
            'time': dt.strftime('%H:%M'),
            'sid': info['sid'],
            'short': info['sid'][:8],
            'size_kb': sz // 1024,
            'model': info['model'],
            'calls': info['api_calls'],
            'first_msg': first_msg,
            'file': out_name,
        })
        print(f"  {date_str} {dt.strftime('%H:%M')} {info['sid'][:8]}  {sz//1024:>6}K  {info['api_calls']:>4} calls -> {out_name}", file=sys.stderr)

    idx_lines = [
        '# cc-haha legacy sessions index (E: drive)',
        '',
        f'Generated: {datetime.datetime.now().strftime("%Y-%m-%d %H:%M")}',
        '',
        'Sessions from `E:\\Users\\Duanyi\\.claude\\cc-haha\\traces\\*.jsonl`',
        'filtered to 2026-09-20 to 2026-09-28 (filemanager project period).',
        '',
        '> **Replaces** `docs/legacy-sessions/` which used the C: portable stale copy.',
        '> E: has 462 traces; C: portable had 224.',
        '',
        '| Date | Time | Session (short) | Size | Calls | First user msg |',
        '|---|---|---|---|---|---|',
    ]
    for e in index_entries:
        msg = e['first_msg'].replace('|', '\\|')[:60]
        idx_lines.append(
            f"| {e['date']} | {e['time']} | [{e['short']}]({e['file']}) | {e['size_kb']:,}K | {e['calls']} | {msg} |"
        )
    idx_lines += [
        '',
        f'Total: {len(index_entries)} sessions, {sum(e["size_kb"] for e in index_entries)/1024:.0f} MB',
        '',
    ]
    (OUT_DIR / 'INDEX.md').write_text('\n'.join(idx_lines), encoding='utf-8')
    print(f"\nINDEX.md written. {len(index_entries)} sessions converted.", file=sys.stderr)


if __name__ == '__main__':
    main()