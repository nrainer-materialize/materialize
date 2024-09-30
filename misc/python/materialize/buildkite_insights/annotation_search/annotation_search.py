#!/usr/bin/env python3

# Copyright Materialize, Inc. and contributors. All rights reserved.
#
# Use of this software is governed by the Business Source License
# included in the LICENSE file at the root of this repository.
#
# As of the Change Date specified in that file, in accordance with
# the Business Source License, use of this software will be governed
# by the Apache License, Version 2.0.

import re
import time
from pathlib import Path

verbose = False
include_csv = False

misses = []

def modify_content(path_in_str: str, content: str, offset: int = 0) -> str:
    if "CREATE SOURCE" not in content:
        return content

    if offset > 0:
        print("Offset: ", offset)

    reg = re.compile(
        r"\n([#>!? ]*)CREATE SOURCE (.*?)(\s|#|IN CLUSTER [A-Za-z0-9-_.{}$]+)+FROM KAFKA CONNECTION (.*?)(\s|#)+\([^)]*TOPIC\s+'(.*?)'[^)]*\)((\s|#)+[^>]([^>]|\s)+?)(\n\n|;|\n(>|!|contains:[^\n]+))"
    )

    match = reg.search(content, pos=offset)

    reg2 = re.compile("FROM KAFKA CONNECTION")
    match2 = reg2.search(content, pos=offset)

    if match is None:
        if match2 is not None:
            miss = f"MISSED in {path_in_str} at {match2.start()}!"
            print(miss)
            misses.append(miss)

        return content

    if verbose:
        print("Match at ", match.start())

    statement = match.group(0)
    lead_in = match.group(1).lstrip()
    source_name = match.group(2)
    topic = match.group(6)
    options_with_space_prefix = match.group(7)

    if "FORMAT CSV" in statement and not include_csv:
        # skip it for now
        return modify_content(path_in_str, content, offset + len(statement))

    modified_statement = statement.replace(options_with_space_prefix, "")

    table_suffix = "tbl"
    table_name = f"{source_name}_{table_suffix}"

    added_statement = f"{lead_in}CREATE TABLE {table_name} FROM SOURCE {source_name} (REFERENCE \"{topic}\"){options_with_space_prefix}"

    if statement.endswith(";"):
        added_statement = f"\n{added_statement};"
    else:
        added_statement = f"{added_statement}\n\n"

    if not modified_statement.endswith("\n"):
        modified_statement = f"{modified_statement}\n"

    replacement = f"{modified_statement}{added_statement}"
    content = content.replace(statement, replacement)
    content = content.replace("DELETE FROM", "DELETEFROMX")
    content = content.replace("delete from", "DELETEFROMX")

    for ending in [" ", "\n", ";"]:
        content = content.replace(f"FROM {source_name}{ending}", f"FROM {table_name}{ending}")
        content = content.replace(f"JOIN {source_name}{ending}", f"JOIN {table_name}{ending}")
        content = content.replace(f"SUBSCRIBE {source_name}{ending}", f"SUBSCRIBE {table_name}{ending}")

        content = content.replace(f"from {source_name}{ending}", f"from {table_name}{ending}")
        content = content.replace(f"join {source_name}{ending}", f"join {table_name}{ending}")
        content = content.replace(f"subscribe {source_name}{ending}", f"subscribe {table_name}{ending}")

    content = content.replace("DELETEFROMX", "DELETE FROM")

    if verbose:
        print("Replacing: \n", statement)
        print("with \n", replacement)

    return modify_content(path_in_str, content, match.start() + len(replacement) - 5)


if __name__ == "__main__":
    file_ending = "*"
    pathlist = Path("/Users/nrainer/Workspaces/mz-repo").glob(f"**/*.{file_ending}")

    textchars = bytearray({7, 8, 9, 10, 12, 13, 27} | set(range(0x20, 0x100)) - {0x7F})
    is_binary_string = lambda bytes: bool(bytes.translate(None, textchars))

    time.sleep(10)

    for path in pathlist:
        if path.is_dir():
            continue

        path_in_str = str(path)

        print(f"{path_in_str} ...")

        if "python/venv" in path_in_str or "venv/lib" in path_in_str or "/target-xcompile/" in path_in_str or "/target/debug" in path_in_str or "doc/user" in path_in_str:
            continue

        if path_in_str.endswith(".log"):
            continue

        if path_in_str.endswith(".md"):
            # not interested
            continue

        if is_binary_string(open(path_in_str, "rb").read(1024)):
            continue

        file = open(path_in_str)
        original_content = file.read()

        altered_content = modify_content(path_in_str, original_content)

        if altered_content == original_content:
            print(f"NOP")
            continue
        else:
            print("Touched: ", path_in_str)

        file = open(path_in_str, "w")
        file.write(altered_content)

    info_file = open(f"/Users/nrainer/Downloads/log_{time.time()}.txt", "w")
    info_file.write("\n".join(misses))
