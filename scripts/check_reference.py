#!/usr/bin/env python3
"""Check the local reference assembler against independently specified bytes."""

from pathlib import Path
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[1]
ASSEMBLER = ROOT / "vasm" / "vasmm68k_mot"
EXPECTED = bytes.fromhex("702a 5280 6002 4e71 4e75 1234 89abcdef")


def main():
    if not ASSEMBLER.is_file():
        raise SystemExit("Build first: make -C vasm -f Makefile.macOS CPU=m68k SYNTAX=mot")
    with tempfile.TemporaryDirectory(prefix="vasm-reference-") as directory:
        output = Path(directory) / "encoding.bin"
        command = [str(ASSEMBLER), "-quiet", "-m68000", "-Fbin", "-o", str(output)]
        source = ROOT / "tests" / "reference" / "encoding.s"
        subprocess.run(command + [str(source)], check=True)
        actual = output.read_bytes()
        if actual != EXPECTED:
            raise SystemExit(f"Encoding mismatch: expected {EXPECTED.hex()}, got {actual.hex()}")
        subprocess.run(command + [str(source)], check=True)
        if output.read_bytes() != actual:
            raise SystemExit("Repeated assembly produced different bytes")
        invalid = Path(directory) / "requires_68020.s"
        invalid.write_text("        extb.l d0\n")
        result = subprocess.run(command + [str(invalid)], capture_output=True, text=True)
        if result.returncode == 0:
            raise SystemExit("68020-only instruction was accepted with -m68000")
    print("PASS: 16 exact bytes, repeatable output, and rejection of a 68020-only instruction")


if __name__ == "__main__":
    main()
