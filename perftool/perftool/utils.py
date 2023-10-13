import re
import subprocess
from pathlib import Path
import random
import pandas as pd
import numpy as np


def sample(min_value: int, max_value: int, mean: int = 0) -> int:
    def distribution_sample(use_uniform: bool = True) -> float | int:
        if use_uniform:
            return random.randrange(min_value, max_value)
        else:
            # lognormal can be chosen if we want to
            # keep samples as uniform as possible while
            # at the same time don't want to generate
            # too many large values which can slow down
            # the benches
            sigma = 0.7
            return np.random.lognormal(mean=np.log(mean) + sigma**2, sigma=sigma)

    value = None
    while value is None:
        value = int(distribution_sample())
        if value >= min_value and value <= max_value:
            break
    return value


def create_repo_from_commmit(commit: str, tmpfolder: str) -> None:
    subprocess.run(["git", "worktree", "add", "-f", tmpfolder, commit], check=True)


def build_release(cli_repo: Path) -> None:
    subprocess.run(["cargo", "build", "--release"], cwd=cli_repo, check=True)
    return


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


# def bench_all_values(
#     cli_repo: Path,
#     bench_function: str,
#     num_samples: int,
#     min_value: int,
#     max_value: int,
#     mean: int = 0,
# ) -> dict:
#     data = {"values": [], "time_taken (in s)": []}
#     for value in tqdm(sample(num_samples, min_value, max_value, mean)):
#         time_taken = bench(bench_function, value, cli_repo)
#         data["values"].append(value)
#         data["time_taken (in s)"].append(time_taken)
#     return data


def sample_and_bench(
    cli_repo: Path,
    bench_function: str,
    min_value: int,
    max_value: int,
) -> dict:
    parameter = sample(min_value, max_value)
    time_taken = bench(bench_function, parameter, cli_repo)
    return {"value": [parameter], "time_taken": [time_taken]}


def write_into_csv(data: dict, csv_file_path: Path, headers: bool) -> None:
    df = pd.DataFrame(data)
    df.to_csv(open(csv_file_path, "a"), header=headers, index=False)
