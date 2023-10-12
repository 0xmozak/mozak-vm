import re
import subprocess

import pandas as pd
import matplotlib.pyplot as plt

import numpy as np
from scipy.stats import linregress


def sample(num_samples: int, min_value: int, max_value: int, mean: int) -> list[int]:
    def distribution_sample(use_uniform: bool = True) -> float:
        if use_uniform:
            return np.random.uniform(min_value, max_value)
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


def create_repo_from_commmit(commit: str, tmpfolder) -> None:
    subprocess.run(
        ["git", "worktree", "add", "-f", f"{tmpfolder}", f"{commit}"], check=True
    )
    return


def build_release(cli_repo: str) -> None:
    subprocess.run(["cargo", "build", "--release"], cwd=cli_repo, check=True)


def bench(bench_function: str, parameter: int, cli_repo: str) -> float:
    stdout = subprocess.check_output(
        args=["cargo", "run", "--release", "bench", bench_function, f"{parameter}"],
        cwd=cli_repo,
        stderr=subprocess.DEVNULL,
    )
    pattern = r"\d+\.\d+"
    time_taken = re.findall(pattern, stdout.decode())[0]
    return float(time_taken)


def write_into_csv(data: dict, csv_file_path) -> None:
    df = pd.DataFrame(data)
    csv_file_path = "data.csv"
    df.to_csv(csv_file_path, index=False)


def plot_csv_data(csv_file_path, bench_function: str):
    data = pd.read_csv(csv_file_path)
    columns = list(data.columns)
    x_data = data[columns[0]]
    y_data = data[columns[1]]
    slope, intercept, _, _, _ = linregress(x_data, y_data)
    predicted_y = intercept + slope * np.array(x_data)
    plt.figure(figsize=(8, 6))
    plt.scatter(
        x_data,
        y_data,
    )
    plt.plot(x_data, predicted_y, color="r", label="Linear Regression Line")
    plt.xlabel("values")
    plt.ylabel("time_taken")
    plt.title(f"results for {bench_function}")
    plt.legend()
    info_text = f"Slope: {slope:.6f}\nIntercept: {intercept:.6f}"
    plt.text(
        0.05,
        0.75,
        info_text,
        transform=plt.gca().transAxes,
        bbox=dict(facecolor="white", edgecolor="black", boxstyle="round,pad=0.5"),
        verticalalignment="top",
        fontsize=12,
    )
    plt.show()


# test how sample distribution plot looks like.
def plot_samples(num_samples: int, min_value: int, max_value: int, mean: int):
    samples = sample(num_samples, min_value, max_value, mean)
    plt.hist(samples, bins=10)
    plt.show()


# plot_samples(1000, 100, 20000, 5000)
