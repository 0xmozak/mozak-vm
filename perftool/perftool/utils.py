import re
import subprocess
from pathlib import Path
import random
from typing import Tuple
from config import get_elf, get_output_name, get_parameter_name
import pandas as pd
from path import get_actual_cli_repo, get_actual_commit_folder, get_elf_path


def shuffled(iterable):
    iterable = list(iterable)
    random.shuffle(iterable)
    return iterable


def sample(min_value: int, max_value: int) -> int:
    return random.randrange(min_value, max_value)


def create_repo_from_commit(commit: str):
    commit_folder = get_actual_commit_folder(commit)
    if (commit_folder / ".git").is_file():
        print(f"Skipping git worktree for {commit}...")
        return
    subprocess.run(
        ["git", "worktree", "add", "--force", commit_folder, commit], check=True
    )


def build_release(cli_repo: Path):
    subprocess.run(["cargo", "build", "--release"], cwd=cli_repo, check=True)


def build_repo(commit: str):
    if commit == "latest":
        print("Treating the current repo as latest")
    else:
        get_actual_commit_folder(commit).mkdir(exist_ok=True)
        create_repo_from_commit(commit)
    cli_repo = get_actual_cli_repo(commit)
    build_release(cli_repo)


def maybe_build_ELF(bench_name, bench_description: str, commit: str):
    elf = get_elf(bench_name, bench_description)
    if elf is None:
        print(f"Skipping build ELF for {bench_name}...")
        return
    print(f"Building ELF for {bench_name}")
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


def init_csv(csv_file_path: Path, bench_name: str):
    """Initialise the csv file with headers if they do not exist."""
    headers = [
        get_parameter_name(bench_name),
        get_output_name(bench_name),
    ]
    try:
        existing_headers = pd.read_csv(csv_file_path, nrows=0).columns.tolist()
    except FileNotFoundError:
        df = pd.DataFrame(columns=headers)
        df.to_csv(csv_file_path, index=False)
        return
    if set(headers) != set(existing_headers):
        raise ValueError(f"Headers do not match the existing file: {existing_headers}.")


def write_into_csv(data: dict, csv_file_path: Path):
    """Write the data into the csv file."""
    df = pd.DataFrame(data)
    with open(csv_file_path, "a", encoding="utf-8") as f:
        df.to_csv(f, header=False, index=False)
