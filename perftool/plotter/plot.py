# TODO: fix this
# $ plot nop-registerstark
# Traceback (most recent call last):

#   File "/Users/matthias/Library/Caches/pypoetry/virtualenvs/perftool-0DeAwteW-py3.12/bin/plot", line 6, in <module>
#     sys.exit(app())
#              ^^^^^

#   File "/Users/matthias/mozak/prog/mozak-vm-2/perftool/plotter/plot.py", line 111, in plot
#     update_plot_from_csv(bench_name)

#   File "/Users/matthias/mozak/prog/mozak-vm-2/perftool/plotter/plot.py", line 103, in update_plot_from_csv
#     plot_all(bench_name)

#   File "/Users/matthias/mozak/prog/mozak-vm-2/perftool/plotter/plot.py", line 80, in plot_all
#     x_data, y_data, slope, intercept, predicted_y = get_data_from_csv(
#                                                     ^^^^^^^^^^^^^^^^^^

#   File "/Users/matthias/mozak/prog/mozak-vm-2/perftool/plotter/plot.py", line 40, in get_data_from_csv
#     slope, intercept, _, _, _ = linregress(x_data, y_data)
#                                 ^^^^^^^^^^^^^^^^^^^^^^^^^^

#   File "/Users/matthias/Library/Caches/pypoetry/virtualenvs/perftool-0DeAwteW-py3.12/lib/python3.12/site-packages/scipy/stats/_stats_mstats_common.py", line 154, in linregress
#     raise ValueError("Inputs must not be empty.")

# ValueError: Inputs must not be empty.

from itertools import cycle
from pathlib import Path
from config import (
    get_benches_with_commit,
    get_description,
    get_output_name,
    get_parameter_name,
)
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


def plot_all(bench_name: str):
    linecycler = cycle(["-", "--", "-.", ":"])
    markerscycler = cycle(["o", ",", "v", "^"])
    colorscycler = cycle(["r", "b", "c", "m"])
    x_label = get_parameter_name(bench_name)
    y_label = get_output_name(bench_name)
    description = get_description(bench_name)
    bench_with_commits_dict = get_benches_with_commit(bench_name)
    plt.figure(figsize=(8, 6))

    num_samples = 0
    for bench_description, bench in bench_with_commits_dict.items():
        data_csv_file = get_data_csv_file(
            bench_name, bench["bench_function"], bench["commit"]
        )
        x_data, y_data, slope, intercept, predicted_y = get_data_from_csv(
            data_csv_file, x_label, y_label
        )
        color = next(colorscycler)
        marker = next(markerscycler)
        linestyle = next(linecycler)
        plot_data(
            x_data=x_data,
            y_data=y_data,
            predicted_y=predicted_y,
            label=bench_description,
            color=color,
            marker=marker,
            linestyle=linestyle,
        )
        num_samples += len(x_data)
    plt.xlabel(x_label)
    plt.ylabel(y_label)
    plt.title(description + f"\n num_samples={num_samples}")
    plt.legend()


def update_plot_from_csv(bench_name: str):
    plot_all(bench_name)


@app.command()
def plot(bench_name: str):
    """
    Plot the data from the csv files corresponding to given `bench_function`
    """
    update_plot_from_csv(bench_name)
    plt.savefig(get_plot_svg_file(bench_name))
    plt.close()


if __name__ == "__main__":
    app()
