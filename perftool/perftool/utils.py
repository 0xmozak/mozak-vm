import json
import re
import subprocess
from pathlib import Path
import random
from typing import List
import pandas as pd
from pyparsing import Any


def sample(min_value: int, max_value: int) -> int:
    return random.randrange(min_value, max_value)


def create_repo_from_commmit(commit: str, commit_folder: Path):
    subprocess.run(
        ["git", "worktree", "add", "--force", str(commit_folder), commit], check=True
    )


def build_release(cli_repo: Path):
    subprocess.run(["cargo", "build", "--release"], cwd=cli_repo, check=True)


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


def get_csv_file(commit: str, bench_function: str) -> Path:
    return Path.cwd() / "data" / bench_function / f"{commit}.csv"


def get_cli_repo(commit: str, bench_function: str) -> Path:
    commit_symlink = Path.cwd() / "build" / bench_function / commit
    return commit_symlink.resolve() / "cli"
