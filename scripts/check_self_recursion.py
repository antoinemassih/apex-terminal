#!/usr/bin/env python3
"""Fail if `pub fn NAME` immediately calls the SAME function via its own path.

Catches the codemod hazard where a bulk rewrite of an inlined expression into
its named helper also rewrites that helper's own body. The result compiles and
stack-overflows at the first paint — `color_alpha` and all four `r_*_cr()`
helpers were broken exactly this way.

Module-aware on purpose. `dom_feed::now_ms()` delegating to
`foundation::time::now_ms()` is a legitimate re-export and must NOT trip: same
name, different module. Only a call whose path resolves to THIS file's own
module is recursion.
"""
import re, sys, glob, os

PAT = re.compile(
    r'pub fn ([a-z_][a-z_0-9]*)\s*\([^)]*\)[^{;]*\{\s*'
    r'((?:crate::)?(?:[a-z_][a-z_0-9]*::)*)\1\s*\(',
    re.S,
)

def module_path(f):
    """src-tauri/src/ui_kit/style.rs -> 'crate::ui_kit::style::'"""
    p = f.replace(chr(92), '/')
    p = p.split('src-tauri/src/', 1)[-1]
    p = p[:-3] if p.endswith('.rs') else p
    parts = [x for x in p.split('/') if x not in ('mod', 'lib', 'main')]
    return 'crate::' + '::'.join(parts) + '::' if parts else 'crate::'

bad = []
for f in glob.glob('src-tauri/src/**/*.rs', recursive=True):
    src = open(f, encoding='utf-8', errors='ignore').read()
    own = module_path(f)
    for m in PAT.finditer(src):
        name, path = m.group(1), m.group(2)
        # Bare `name(` inside `pub fn name` is unambiguous recursion.
        # A qualified path is recursion only when it names THIS module.
        if path == '' or path == own or own.endswith(path.lstrip('crate::')):
            line = src[:m.start()].count('\n') + 1
            bad.append(f'{f}:{line}: pub fn {name} calls itself ({path or "bare"})')

if bad:
    print('SELF-RECURSION — a helper delegates to itself:')
    for b in bad:
        print('   ', b)
    sys.exit(1)
print('self-recursion check OK')
sys.exit(0)
