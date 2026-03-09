#!/usr/bin/env python3
import os
import re
import shutil
import sys
import subprocess

SELF_PATH = os.path.abspath(__file__)
SELF_DIR = os.path.dirname(SELF_PATH)
CONFIG_PATH = os.path.join(SELF_DIR, "config.toml")
DEJJ_INSTALL_PATH = os.path.abspath(os.path.join(SELF_DIR, "../../target/release"))
DEJJ_PATH = os.path.join(DEJJ_INSTALL_PATH, "dejj")
DEJJ_REPO = "https://github.com/Pistonite/dejj"
DEJJ_BUILD_OR_INSTALL = "BUILD"
DEJJ_VERSION = "0.1.0"
PROJECT_DIR = os.path.abspath(os.path.join(SELF_DIR, "../../../botw-decomp"))
GREP_SOURCE_DIRS = [ "src", "lib" ]

def main():
    ensure_dejj()
    has_error = check_enumeratorize_config()
    args = sys.argv[1:]
    if args and args[0] == "check":
        if has_error:
            print("ERROR: config check failed")
            sys.exit(1)
        print("config check ok")
    else:
        invoke_dejj(args)

def ensure_dejj():
    if DEJJ_BUILD_OR_INSTALL == "BUILD":
        subprocess.run(["cargo", "build", "--bin", "dejj", "--release"], check=True)
        if not os.path.exists(DEJJ_PATH):
            raise Exception("did not find dejj binary after building")
        return

    if DEJJ_BUILD_OR_INSTALL == "INSTALL":
        need_install = True
        if os.path.exists(DEJJ_PATH):
            result = subprocess.run([DEJJ_PATH, "version"], check=True, capture_output=True, text=True, encoding='utf-8')
            output = result.stdout.strip()
            if output == DEJJ_VERSION:
                need_install = False
        if need_install:
            raise Exception("TODO: implement installing dejj")

def invoke_dejj(args):
    cmd = [DEJJ_PATH, "-C", CONFIG_PATH] + args
    print("running: " + " ".join(cmd))
    subprocess.run(cmd, check=True)


def check_enumeratorize_config():
    config = parse_enumeratorize_config()
    hits = grep_enum_usages()

    has_error = False

    for filepath, content in hits:
        result = extract_enum_name(content)
        if result is None:
            continue
        name, companion = result

        # Find config section whose key exactly matches the filepath
        # Normalize to forward slashes to handle Windows path separators
        section_lines = config.get(filepath.replace("\\", "/")) or []

        for needle in (f"{name}$", f"{name}::{companion}$"):
            if not any(needle in entry for entry in section_lines):
                print(f"\033[91mWARNING: enum {name} is not in the enumeratorize config (in file {filepath})\033[0m")
                has_error = True
                break
    return has_error


def parse_enumeratorize_config() -> dict:
    """Parse the enumeratorize section of config.toml.
    Returns {file_path: [raw_lines]} mapping."""
    result = {}
    in_section = False
    current_file = None
    current_lines = []

    with open(CONFIG_PATH, "r", encoding="utf-8") as f:
        for raw_line in f:
            line = raw_line.rstrip("\n")
            stripped = line.strip()

            if not in_section:
                if stripped == "enumeratorize = [":
                    in_section = True
                continue

            # End of enumeratorize block
            if stripped == "]":
                if current_file is not None:
                    result[current_file] = current_lines
                break

            # New file section
            if stripped.startswith("# file:"):
                if current_file is not None:
                    result[current_file] = current_lines
                current_file = stripped[len("# file:"):].strip()
                current_lines = []
                continue

            # Empty line ends current file's entries
            if stripped == "":
                if current_file is not None:
                    result[current_file] = current_lines
                    current_file = None
                    current_lines = []
                continue

            # Config entry line belonging to current file
            if current_file is not None:
                current_lines.append(stripped)

    return result


def grep_enum_usages() -> list:
    """Grep source dirs for enum macro usages.
    Returns list of (filepath, line_content) tuples."""
    tool = find_grep_tool()
    tool_name = os.path.basename(tool)

    # Use relative source dirs so rg/grep output paths are relative to PROJECT_DIR,
    # matching the file paths in the config.
    if tool_name == "rg":
        cmd = [tool, "ORE_ENUM|ORE_VALUED_ENUM|SEAD_ENUM",
               "--glob", "*.h", "--glob", "*.cpp"] + GREP_SOURCE_DIRS
    else:
        cmd = [tool, "-r", "-E", "ORE_ENUM|ORE_VALUED_ENUM|SEAD_ENUM",
               "--include=*.h", "--include=*.cpp"] + GREP_SOURCE_DIRS

    result = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8", cwd=PROJECT_DIR)
    hits = []
    for line in result.stdout.splitlines():
        # Expected format: filepath:content
        parts = line.split(":", 1)
        if len(parts) < 2:
            continue
        filepath, content = parts[0], parts[1]
        hits.append((filepath, content))
    return hits



def find_grep_tool() -> str:
    """Return path to rg or grep, or exit with error."""
    rg = shutil.which("rg")
    if rg:
        return rg
    grep = shutil.which("grep")
    if grep:
        return grep
    print("Error: neither 'rg' nor 'grep' found. Please install ripgrep or grep.")
    sys.exit(1)


_ENUM_MACRO_RE = re.compile(
    r"(SEAD_ENUM|SEAD_ENUM_EX|SEAD_ENUM_EX_VALUES|ORE_ENUM|ORE_VALUED_ENUM)\s*\(\s*(\w+)\s*,"
)

def extract_enum_name(line: str):
    """Extract (name, companion_suffix) from a source line, or return None.
    companion_suffix is 'ValueType' for SEAD_ENUM variants, 'Type' for ORE_ENUM variants."""
    # Remove C++ line comments
    comment_idx = line.find("//")
    if comment_idx != -1:
        line = line[:comment_idx]
    stripped = line.strip()
    if stripped.startswith("#define"):
        return None
    m = _ENUM_MACRO_RE.search(stripped)
    if m:
        macro, name = m.group(1), m.group(2)
        companion = "ValueType" if macro.startswith("SEAD_") else "Type"
        return name, companion
    return None

if __name__ == "__main__":
    main()
