'''
Python script to dump programs_map.json
'''

import json
import shlex
import subprocess
from typing import Dict, List


def get_self_prog_id(example: str) -> tuple[str, str]:
    elf = f"examples/{example}/mozakvm/target/riscv32im-mozak-mozakvm-elf/mozak-release/{example}-mozakvm"
    command = f"""cargo run --bin dump-self-prog-id -- {elf}"""
    out = subprocess.run(
        args=shlex.split(command),
        cwd=f"../",
        capture_output=True,
        check=True,
    )
    return (out.stdout.decode('utf-8').strip(), elf)

def get_program_map(examples: List[str]) -> List[Dict[str, str]]:
    map = []
    for example in examples:
        (id, elf) = get_self_prog_id(example)
        map.append({
            "name": id,
            "path": elf
        })
    return map

def dump_programs_map_json(examples: List[str]):
    with open('programs_map.json', 'w') as file:
        map = get_program_map(examples)
        json.dump(map, file, indent=4)

if __name__ == "__main__":
    examples = ["token", "wallet"]
    dump_programs_map_json(examples)
