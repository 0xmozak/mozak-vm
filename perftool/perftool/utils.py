import re
import subprocess

import pandas as pd
import matplotlib.pyplot as plt


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


def plot_csv_data(csv_file_path):
    data = pd.read_csv(csv_file_path)
    columns = list(data.columns)
    x_data = data[columns[0]]
    y_data = data[columns[1]]
    plt.figure(figsize=(10, 6))
    plt.scatter(
        x_data,
        y_data,
    )
    plt.xlabel("values")
    plt.ylabel("time_taken")
    plt.title("Bench results")
    plt.legend()
    plt.show()
