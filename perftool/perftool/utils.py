import re
import subprocess
from typing import Tuple
from pathlib import Path
import random

import pandas as pd
import matplotlib.pyplot as plt

import numpy as np
from scipy.stats import linregress


def sample(num_samples: int, min_value: int, max_value: int, mean: int) -> list[int]:
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

    samples = []
    while len(samples) < num_samples:
        sample = distribution_sample()
        if sample > min_value and sample < max_value:
            samples.append(int(sample))
    return list(samples)


def create_repo_from_commmit(commit: str, tmpfolder: str) -> None:
    subprocess.run(["git", "worktree", "add", "-f", tmpfolder, commit], check=True)


def build_release(cli_repo: Path) -> None:
    subprocess.run(["cargo", "build", "--release"], cwd=cli_repo, check=True)


def bench(bench_function: str, parameter: int, cli_repo: Path) -> float:
    stdout = subprocess.check_output(
        args=["cargo", "run", "--release", "bench", bench_function, f"{parameter}"],
        cwd=cli_repo,
        stderr=subprocess.DEVNULL,
    )
    pattern = r"\d+\.\d+"
    time_taken = re.findall(pattern, stdout.decode())[0]
    return float(time_taken)


def write_into_csv(data: dict, csv_file_path: Path) -> None:
    df = pd.DataFrame(data)
    df.to_csv(open(csv_file_path, "w"), index=False)


def get_data(csv_file_path: Path):
    data = pd.read_csv(csv_file_path)
    columns = list(data.columns)
    x_data = data[columns[0]]
    y_data = data[columns[1]]
    slope, intercept, _, _, _ = linregress(x_data, y_data)
    predicted_y = intercept + slope * np.array(x_data)
    return x_data, y_data, slope, intercept, predicted_y


def plot(x_data, y_data, slope, intercept, predicted_y, color: str, label: str):
    plt.scatter(x=x_data, y=y_data, color=color, label=label)
    plt.plot(x_data, predicted_y, color=color, label=f"{label} line")


def plot_both(csv_file_path_1: Path, csv_file_path_2: Path, bench_function: str):
    plt.figure(figsize=(8, 6))

    x_data_1, y_data_1, slope_1, intercept_1, predicted_y_1 = get_data(csv_file_path_1)
    x_data_2, y_data_2, slope_2, intercept_2, predicted_y_2 = get_data(csv_file_path_2)
    plot(x_data_1, y_data_1, slope_1, intercept_1, predicted_y_1, "red", "commit_1")
    plot(x_data_2, y_data_2, slope_2, intercept_2, predicted_y_2, "blue", "commit_2")
    info_text = f"Slope_1: {slope_1:.6f}"
    info_text += f"\nIntercept_1: {intercept_1:.6f}"
    info_text += f"\n\n"
    info_text += f"\nSlope_2: {slope_2:.6f}"
    info_text += f"\nIntercept_2: {intercept_2:.6f}"

    plt.text(
        0.65,
        0.35,
        info_text,
        transform=plt.gca().transAxes,
        bbox=dict(facecolor="white", edgecolor="black", boxstyle="round,pad=0.5"),
        verticalalignment="top",
        fontsize=12,
    )
    plt.xlabel("values")
    plt.ylabel("time_taken")
    plt.title(f"results for {bench_function}")
    plt.legend()
    plt.show()


# test how sample distribution plot looks like.
def plot_samples(num_samples: int, min_value: int, max_value: int, mean: int):
    samples = sample(num_samples, min_value, max_value, mean)
    plt.hist(samples, bins=10)
    plt.show()


# plot_samples(1000, 100, 20000, 5000)
