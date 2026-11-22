"""Storme's rebrand, re-run after every merge from Aurora (upstream).

    python scripts/storme/rebrand.py          # apply
    python scripts/storme/rebrand.py --check  # list what is still Aurora's, change nothing

Aurora keeps adding text that names its viewer; this puts Storme's name in the
user-facing places and leaves the rest alone:
- the crates keep their `aurora-*` names (smaller merges);
- Aurora's website, team ("staff") and the media pack download stay Aurora's;
- test fixtures that name Aurora (versions in old logs, sample paths) stay.
Idempotent: running it twice changes nothing the second time.
"""

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
CRATES = ROOT / "crates"
LOCALES = CRATES / "aurora-i18n" / "locales"
REPO = "github.com/AstralaDev/Storme-Viewer"


def read(p):
    return p.read_bytes().decode("utf-8")


def write(p, s, changed):
    if s != read(p):
        changed.append(str(p.relative_to(ROOT)))
        if "--check" not in sys.argv:
            p.write_bytes(s.encode("utf-8"))


def locale_lines(s):
    """Rename in the catalogs, except Aurora's own site, team and comments."""
    nl = "\r\n" if "\r\n" in s else "\n"
    out = []
    for line in s.split(nl):
        keep = "aurora-viewer.org" in line or "staff" in line.lower() or line.lstrip().startswith("#")
        out.append(line if keep else line.replace("Aurora Viewer", "Storme Viewer"))
    return nl.join(out)


def is_test_file(p):
    return p.name in ("tests.rs",) or p.name.endswith("_tests.rs") or "tests" in p.parts


def rust(s):
    """Code outside `#[cfg(test)]` modules: names, data folders, services."""
    # only the test module itself is left alone (`#[cfg(test)]` may also mark
    # a helper above ordinary code)
    m = re.search(r"#\[cfg\(test\)\]\s*\n\s*mod ", s)
    head, tail = (s, "") if not m else (s[: m.start()], s[m.start() :])
    head = head.replace('"org", "Aurora", "AuroraViewer"', '"org", "Storme", "StormeViewer"')
    # string literals that start with the product name, and `format!` ones
    head = re.sub(r'"Aurora Viewer(?=[ \\\n"{:])', '"Storme Viewer', head)
    head = head.replace("Name=Aurora Viewer", "Name=Storme Viewer")  # the Linux link handler
    head = head.replace('"AuroraViewerCache"', '"StormeViewerCache"').replace('"AuroraViewer/"', '"StormeViewer/"')
    return head + tail


# files whose Aurora links are deliberate: the trust list (Aurora's site is
# still trusted), and the media and ffmpeg packs (hosted by Aurora, hash-checked)
KEEP_LINKS = {"link_trust.rs", "ffmpeg_pack.rs"}


def links(p, s):
    if p.name in KEEP_LINKS:
        return s
    lines = s.split("\n")
    for i, line in enumerate(lines):
        if "const DOWNLOADS" in line:
            continue
        lines[i] = line.replace("github.com/Aurora-Viewer/Aurora-Viewer", REPO).replace(
            "api.github.com/repos/Aurora-Viewer/Aurora-Viewer", "api.github.com/repos/AstralaDev/Storme-Viewer"
        )
    return "\n".join(lines)


def main():
    changed = []
    for f in sorted(LOCALES.glob("*.ftl")):
        s = locale_lines(read(f))
        if f.name == "en.ftl":
            s = s.replace("Aurora's", "Storme's")
        write(f, s, changed)
    for f in sorted(CRATES.rglob("*.rs")):
        if "target" in f.parts:
            continue
        s = read(f)
        new = links(f, s) if is_test_file(f) else links(f, rust(s))
        write(f, new, changed)
    build = CRATES / "aurora-viewer" / "build.rs"
    write(build, read(build).replace('"ProductName", "Aurora Viewer"', '"ProductName", "Storme Viewer"').replace(
        '"FileDescription", "Aurora Viewer"', '"FileDescription", "Storme Viewer"'), changed)
    verb = "would change" if "--check" in sys.argv else "changed"
    print(f"{verb} {len(changed)} files")
    for c in changed:
        print("  " + c)


if __name__ == "__main__":
    main()
