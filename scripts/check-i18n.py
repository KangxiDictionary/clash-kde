#!/usr/bin/env python3
"""Fail when a user-visible Rust string bypasses the ServiceError protocol,
or when a ServiceError code/template is missing from the shared catalogs."""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CN = re.compile(r'[\u4e00-\u9fff]')
CODE = re.compile(r'pub const [A-Z0-9_]+: &str = "([^"]+)"')
CPP_CODE = re.compile(r'code == QLatin1String\("([^"]+)"\)')
PO_MSGID = re.compile(r'^msgid "((?:\\.|[^"\\])*)"', re.M)
CPP_TEXT = re.compile(r'text\("((?:\\.|[^"\\])*)"\)')
TRACING_SEVERE = re.compile(r'tracing::(warn|error)!\s*\((.*?)\)\s*;', re.S)
CODE_FIELD = re.compile(r'code\s*=\s*(?:(?:crate::error::)?codes::([A-Z0-9_]+)|%[A-Za-z_][\w.]*\.code)')


def rust_strings() -> list[tuple[str, int, str]]:
    bad: list[tuple[str, int, str]] = []
    for path in sorted((ROOT / 'src').glob('*.rs')):
        text = path.read_text().split('#[cfg(test)]', 1)[0]
        for number, line in enumerate(text.splitlines(), 1):
            stripped = line.strip()
            if not CN.search(line) or '"' not in line:
                continue
            if stripped.startswith('//') or stripped.startswith('#['):
                continue
            if 'tracing::' in line or 'allow(' in line or 'POLKIT' in line:
                continue
            if 'OVERRIDE_TEMPLATE' in line or 'const ' in line:
                continue
            bad.append((str(path.relative_to(ROOT)), number, stripped))
    return bad


def tracing_texts() -> list[tuple[str, int, str]]:
    bad: list[tuple[str, int, str]] = []
    for path in sorted((ROOT / 'src').glob('*.rs')):
        for number, line in enumerate(path.read_text().splitlines(), 1):
            if 'tracing::' in line and CN.search(line) and not line.strip().startswith('//'):
                bad.append((str(path.relative_to(ROOT)), number, line.strip()))
    return bad


def tracing_without_codes() -> list[tuple[str, int, str]]:
    bad: list[tuple[str, int, str]] = []
    for path in sorted((ROOT / 'src').glob('*.rs')):
        text = path.read_text()
        for match in TRACING_SEVERE.finditer(text):
            body = match.group(2)
            if CODE_FIELD.search(body):
                continue
            line = text[: match.start()].count('\n') + 1
            bad.append((str(path.relative_to(ROOT)), line, match.group(0).splitlines()[0]))
    return bad


def main() -> int:
    codes = set(CODE.findall((ROOT / 'src/error.rs').read_text()))
    cpp = (ROOT / 'kcm/servicetranslations.cpp').read_text()
    mapped = set(CPP_CODE.findall(cpp))
    templates = set(CPP_TEXT.findall(cpp))
    po = (ROOT / 'kcm/po/zh_CN/kcm_clashkde.po').read_text()
    msgids = set(PO_MSGID.findall(po))

    failed = False
    missing_codes = sorted(codes - mapped)
    if missing_codes:
        failed = True
        print('ServiceError codes without a C++ translation:', file=sys.stderr)
        for code in missing_codes:
            print(f'  {code}', file=sys.stderr)

    missing_po = sorted(templates - msgids)
    if missing_po:
        failed = True
        print('C++ templates without a zh_CN PO entry:', file=sys.stderr)
        for text in missing_po:
            print(f'  {text}', file=sys.stderr)

    bad = rust_strings()
    if bad:
        failed = True
        print('Rust user-visible strings with raw non-ASCII text:', file=sys.stderr)
        for path, number, line in bad:
            print(f'  {path}:{number}: {line}', file=sys.stderr)

    trace_codes = tracing_without_codes()
    if trace_codes:
        failed = True
        print('tracing warn/error records must carry a code:', file=sys.stderr)
        for path, number, line in trace_codes:
            print(f'  {path}:{number}: {line}', file=sys.stderr)

    logs = tracing_texts()
    if logs:
        failed = True
        print('tracing logs must stay English:', file=sys.stderr)
        for path, number, line in logs:
            print(f'  {path}:{number}: {line}', file=sys.stderr)

    if failed:
        return 1
    print(f'i18n audit passed: {len(codes)} codes, {len(templates)} templates')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
