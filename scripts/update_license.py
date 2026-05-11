#!/usr/bin/env python3
# ///script
# requires-python = ">=3.10"
# ///
# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 Knitli Inc.
# NOTE: SPDX identifiers at the top of a file apply only to that file.
"""
Propagate changes to the Marque License across the repo. By default, it reads the new license text from LICENSE.md in the repo root, but you can specify a different file or read from standard input.
"""
# TODO: Update hashes in deny.toml and deny.wasm-safe.toml

import argparse

from pathlib import Path


def process_args() -> argparse.Namespace:
    """Process command-line arguments for the license update script."""
    parser = argparse.ArgumentParser(
        description="Propagate changes to the Marque License across the repo."
    )
    parser.add_argument(
        "--stdin",
        action="store_true",
        help="Read the new license text from standard input instead of a file.",
    )
    parser.add_argument(
        "license_file",
        nargs="?",
        help="Path to the new license text file. Ignored if --stdin is used.",
        type=Path,
        default=Path(__file__).parent.parent / "LICENSE.md",
    )
    return parser.parse_args()


def get_licenses(license_file: Path | None = None) -> list[Path]:
    """Get a list of all license files to update, excluding certain directories and the input license file if specified."""
    root_dir = Path(__file__).parent.parent
    licenses = [
        f
        for f in root_dir.glob("**/LICENSE.md")
        if f.is_file()
        and "fonts" not in f.parts
        and "OCR-B" not in f.parts
        and "IBM-Plex-Sans" not in f.parts
        and "Fira-Code" not in f.parts
    ]
    licenses.append(root_dir / "LICENSES" / "LicenseRef-MarqueLicense-1.0.md")
    if license_file is not None and any(
        f for f in licenses if f.samefile(license_file)
    ):
        licenses.remove(license_file)

    return licenses


def get_license_text(args: argparse.Namespace) -> str:
    """Get the new license text from either standard input or a file."""
    if args.stdin:
        all_input = list(iter(input, ""))
        all_input[0] = (
            all_input[0].lstrip("\ufeff").lstrip("- ").strip()
        )  # Remove BOM and arg separator if present
        return f"{all_input[0]}\n{'\n'.join(all_input[1:]).rstrip()}\n"
    else:
        return f"{args.license_file.read_text(encoding='utf-8').strip()}\n"


def write_license(license_text: str, license_file: Path) -> None:
    license_file.write_text(license_text, encoding="utf-8")
    print(f"Updated {license_file}")


def main() -> None:
    """Main function to update license files."""
    args = process_args()
    license_text = get_license_text(args)
    licenses = get_licenses(args.license_file if not args.stdin else None)

    for license_file in licenses:
        write_license(license_text, license_file)


if __name__ == "__main__":
    main()
