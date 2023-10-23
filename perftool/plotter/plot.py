from itertools import cycle
from pathlib import Path
import numpy as np
import pandas as pd
import matplotlib.pyplot as plt
from scipy.stats import linregress
import typer
import json
from typing import Tuple

app = typer.Typer()
plt.style.use("seaborn-v0_8-colorblind")


def load_plot_data_from_config(bench_function: str) -> Tuple[dict, str, str, str]:
    config_file = Path.cwd() / "config.json"
    with open(config_file) as f:
        config = json.load(f)
    bench_data = config["benches"][bench_function]

    commits = bench_data["commits"]
    description = bench_data["description"]
    x_label = bench_data["parameter"]
    y_label = bench_data["output"]

    return commits, description, x_label, y_label


def get_csv_file(commit: str, bench_function: str) -> Path:
    csv_file_path = Path.cwd() / "data" / bench_function / f"{commit}.csv"
    return csv_file_path


def get_data_from_csv(csv_file_path: Path, x_label: str, y_label: str) -> Tuple:
    data = pd.read_csv(csv_file_path)
    x_data = data[x_label]
    y_data = data[y_label]
    slope, intercept, _, _, _ = linregress(x_data, y_data)
    predicted_y = intercept + slope * np.array(x_data)
    return x_data, y_data, slope, intercept, predicted_y


def plot_data(
    x_data, y_data, predicted_y, label: str, color: str, marker: str, linestyle: str
):
    plt.scatter(
        x=x_data,
        y=y_data,
        color=color,
        label=label,
        marker=marker,
        s=10,
    )
    plt.plot(
        x_data,
        predicted_y,
        color=color,
        label=f"{label} line",
        linestyle=linestyle,
    )


def plot_all(bench_function: str):
    linecycler = cycle(["-", "--", "-.", ":"])
    markerscycler = cycle(["o", ",", "v", "^"])
    colorscycler = cycle(["r", "b", "c", "m"])
    commits, description, x_label, y_label = load_plot_data_from_config(bench_function)
    plt.figure(figsize=(8, 6))

    for commit_description in commits:
        csv_data_file = get_csv_file(commits[commit_description], bench_function)
        x_data, y_data, slope, intercept, predicted_y = get_data_from_csv(
            csv_data_file, x_label, y_label
        )
        color = next(colorscycler)
        marker = next(markerscycler)
        linestyle = next(linecycler)
        plot_data(
            x_data=x_data,
            y_data=y_data,
            predicted_y=predicted_y,
            label=commit_description,
            color=color,
            marker=marker,
            linestyle=linestyle,
        )
    plt.xlabel(x_label)
    plt.ylabel(y_label)
    plt.title(description)
    plt.legend()


def update_plot_from_csv(bench_function: str):
    plot_all(bench_function)


@app.command()
def plot(bench_function: str):
    plot_folder = Path.cwd() / "plots"
    plot_folder.mkdir(exist_ok=True)
    update_plot_from_csv(bench_function)
    plt.savefig(plot_folder / f"{bench_function}.svg")
    plt.close()


if __name__ == "__main__":
    app()
