#!/usr/bin/env python3
"""Extract the release notes of a version from the AppStream metainfo as Markdown.

Usage: release_notes.py <metainfo.xml> <version>

The metainfo <releases> block is the single source of the changelog: this script
fails if the release is missing, has no date or has an empty description.
"""

import sys
import xml.etree.ElementTree as ET


def text(element: ET.Element) -> str:
    return " ".join("".join(element.itertext()).split())


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__.strip(), file=sys.stderr)
        return 2

    path, version = sys.argv[1], sys.argv[2].removeprefix("v")
    root = ET.parse(path).getroot()

    release = next(
        (r for r in root.iterfind("releases/release") if r.get("version") == version),
        None,
    )
    if release is None:
        print(
            f"error: no <release version=\"{version}\"> in {path}.\n"
            "Add the changelog of this version to the metainfo <releases> block.",
            file=sys.stderr,
        )
        return 1

    if not release.get("date"):
        print(f"error: release {version} has no date attribute", file=sys.stderr)
        return 1

    blocks = []
    description = release.find("description")
    for child in description if description is not None else []:
        if child.tag == "p":
            blocks.append(text(child))
        elif child.tag in ("ul", "ol"):
            blocks.append("\n".join(f"- {text(li)}" for li in child.iterfind("li")))

    blocks = [block for block in blocks if block]
    if not blocks:
        print(f"error: release {version} has an empty description", file=sys.stderr)
        return 1

    print("\n\n".join(blocks))
    return 0


if __name__ == "__main__":
    sys.exit(main())
