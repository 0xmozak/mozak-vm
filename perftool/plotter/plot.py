from itertools import cycle
from pathlib import Path
import numpy as np
import pandas as pd
import matplotlib.pyplot as plt
from path import get_config_json, get_data_csv_file, get_plot_svg_file
from scipy.stats import linregress
import typer
import json
from typing import Tuple

app = typer.Typer()
plt.style.use("seaborn-v0_8-colorblind")


def load_plot_data_from_config(bench_function: str) -> Tuple[dict, str, str, str]:
    config_file = get_config_json()
    with open(config_file) as f:
        config = json.load(f)
    bench_data = config["benches"][bench_function]

    commits = bench_data["commits"]
    description = bench_data["description"]
    x_label = bench_data["parameter"]
    y_label = bench_data["output"]

    return commits, description, x_label, y_label


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

    num_samples = 0
    for commit_description in commits:
        data_csv_data_file = get_data_csv_file(
            bench_function, commits[commit_description]
        )
        x_data, y_data, slope, intercept, predicted_y = get_data_from_csv(
            data_csv_data_file, x_label, y_label
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
        num_samples += len(x_data)
    plt.xlabel(x_label)
    plt.ylabel(y_label)
    plt.title(description + f"\n num_samples={num_samples}")
    plt.legend()


def update_plot_from_csv(bench_function: str):
    plot_all(bench_function)


@app.command()
def plot(bench_function: str):
    """
    Plot the data from the csv files corresponding to given `bench_function`
    """
    update_plot_from_csv(bench_function)
    plt.savefig(get_plot_svg_file(bench_function))
    plt.close()


if __name__ == "__main__":
    app()
