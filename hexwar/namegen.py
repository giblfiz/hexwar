#!/usr/bin/env python3
"""
Utility to get the deterministic BIP-style name for a ruleset file.

Usage:
    python -m hexwar.namegen path/to/ruleset.json
    python -m hexwar.namegen path/to/champion.json
    python -m hexwar.namegen --signature "K2:A5,A5,C1,D5|K1:B3,B3,D1"
"""

import argparse
import json
import sys

from hexwar.evolution import (
    ruleset_signature,
    signature_to_name,
    genome_to_ruleset,
    board_set_to_ruleset,
)


def get_name_from_file(filepath: str) -> tuple[str, str]:
    """Load a ruleset file and return (name, signature)."""
    with open(filepath) as f:
        data = json.load(f)

    # Handle different formats
    if 'ruleset' in data:
        # Champion format
        rs = genome_to_ruleset(data['ruleset'])
    elif 'pieces' in data:
        # Board set / designer format
        rs = board_set_to_ruleset(data)
    else:
        # Raw genome format
        rs = genome_to_ruleset(data)

    sig = ruleset_signature(rs)
    name = signature_to_name(sig)
    return name, sig


def main():
    parser = argparse.ArgumentParser(
        description='Get deterministic BIP-style name for a ruleset'
    )
    parser.add_argument('file', nargs='?', help='Path to ruleset JSON file')
    parser.add_argument('--signature', '-s', help='Raw signature string to convert')
    parser.add_argument('--verbose', '-v', action='store_true', help='Show signature too')

    args = parser.parse_args()

    if args.signature:
        name = signature_to_name(args.signature)
        if args.verbose:
            print(f"{name} ({args.signature})")
        else:
            print(name)
    elif args.file:
        try:
            name, sig = get_name_from_file(args.file)
            if args.verbose:
                print(f"{name} ({sig})")
            else:
                print(name)
        except Exception as e:
            print(f"Error: {e}", file=sys.stderr)
            sys.exit(1)
    else:
        parser.print_help()
        sys.exit(1)


if __name__ == '__main__':
    main()
