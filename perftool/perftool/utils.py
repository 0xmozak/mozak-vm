import json
import re
import subprocess
from pathlib import Path
import random
from typing import List
import pandas as pd
from path import get_actual_commit_folder, get_elf_path
from pyparsing import Any


def sample(min_value: int, max_value: int) -> int:
    return random.randrange(min_value, max_value)


def create_repo_from_commit(commit: str):
    commit_folder = get_actual_commit_folder(commit)
    if (commit_folder / ".git").is_file():
        print(f"Skipping build for {commit}...")
        return
    subprocess.run(
        ["git", "worktree", "add", "--force", commit_folder, commit], check=True
    )


def build_release(cli_repo: Path):
    subprocess.run(["cargo", "build", "--release"], cwd=cli_repo, check=True)


def maybe_build_ELF(bench_function: str, commit: str):
    data = load_bench_function_data(bench_function)
    elf = data.get("elf")
    if elf is None:
        print(f"Skipping build ELF for {bench_function}...")
        return
    print(f"Building ELF for {bench_function}")
    elf_path = get_elf_path(elf, commit)
    subprocess.run(["cargo", "build", "--release"], cwd=elf_path, check=True)


def bench(bench_function: str, parameter: int, cli_repo: Path) -> float:
    stdout = subprocess.check_output(
        args=[
            "cargo",
            "run",
            "--release",
            "bench",
            bench_function,
            f"{parameter}",
        ],
        cwd=cli_repo,
        stderr=subprocess.DEVNULL,
    )
    pattern = r"\d+\.\d+"
    time_taken = re.findall(pattern, stdout.decode())[0]
    return float(time_taken)


def sample_and_bench(
    cli_repo: Path,
    bench_function: str,
    min_value: int,
    max_value: int,
) -> dict[str, List[int | float]]:
    parameter = sample(min_value, max_value)
    output = bench(bench_function, parameter, cli_repo)
    bench_data = load_bench_function_data(bench_function)
    return {bench_data["parameter"]: [parameter], bench_data["output"]: [output]}


def load_bench_function_data(bench_function: str) -> dict[str, Any]:
    config_file_path = Path.cwd() / "config.json"
    with open(config_file_path, "r") as f:
        config = json.load(f)
        return config["benches"][bench_function]


def init_csv(csv_file_path: Path, bench_function: str):
    bench_function_data = load_bench_function_data(bench_function)
    headers = [bench_function_data["parameter"], bench_function_data["output"]]
    try:
        existing_headers = pd.read_csv(csv_file_path, nrows=0).columns.tolist()
    except FileNotFoundError:
        df = pd.DataFrame(columns=headers)
        df.to_csv(csv_file_path, index=False)
        return
    if set(headers) != set(existing_headers):
        raise ValueError(f"Headers do not match the existing file: {existing_headers}.")


def write_into_csv(data: dict, csv_file_path: Path):
    df = pd.DataFrame(data)
    with open(csv_file_path, "a") as f:
        df.to_csv(f, header=False, index=False)
