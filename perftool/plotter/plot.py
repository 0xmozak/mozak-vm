import json
from pathlib import Path
import numpy as np
import pandas as pd
import matplotlib.pyplot as plt
from scipy.stats import linregress
import typer

app = typer.Typer()


def get_data(csv_file_path: Path):
    data = pd.read_csv(csv_file_path)
    columns = list(data.columns)
    x_data = data[columns[0]]
    y_data = data[columns[1]]
    slope, intercept, _, _, _ = linregress(x_data, y_data)
    predicted_y = intercept + slope * np.array(x_data)
    return x_data, y_data, slope, intercept, predicted_y


def plot_data(x_data, y_data, slope, intercept, predicted_y, color: str, label: str):
    plt.scatter(x=x_data, y=y_data, color=color, label=label)
    plt.plot(x_data, predicted_y, color=color, label=f"{label} line")


def plot_both(csv_file_path_1: Path, csv_file_path_2: Path, bench_function: str):
    plt.figure(figsize=(8, 6))

    x_data_1, y_data_1, slope_1, intercept_1, predicted_y_1 = get_data(csv_file_path_1)
    x_data_2, y_data_2, slope_2, intercept_2, predicted_y_2 = get_data(csv_file_path_2)
    plot_data(
        x_data_1, y_data_1, slope_1, intercept_1, predicted_y_1, "red", "commit_1"
    )
    plot_data(
        x_data_2, y_data_2, slope_2, intercept_2, predicted_y_2, "blue", "commit_2"
    )
    info_text = f"Slope_1: {slope_1:.6f}"
    info_text += f"\nIntercept_1: {intercept_1:.6f}"
    info_text += "\n\n"
    info_text += f"\nSlope_2: {slope_2:.6f}"
    info_text += f"\nIntercept_2: {intercept_2:.6f}"

    plt.text(
        0.35,
        0.95,
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

    # TODO: save the plt file as svg file


def update_plot_from_csv(
    bench_function: str, data_1_csv_file: Path, data_2_csv_file: Path
):
    plot_both(data_1_csv_file, data_2_csv_file, bench_function)


@app.command()
def plot(bench_function: str):
    config_file = Path.cwd() / "config.json"
    tmpfolder = Path(json.load(Path.open(config_file, "r"))["tmpfolder"])
    data_1_csv_file = tmpfolder / "data_1.csv"
    data_2_csv_file = tmpfolder / "data_2.csv"
    while True:
        try:
            update_plot_from_csv(bench_function, data_1_csv_file, data_2_csv_file)
            plt.pause(5)
            plt.close()
        except KeyboardInterrupt:
            plt.close()
            print("Plotting stopped.")
            break


if __name__ == "__main__":
    app()
