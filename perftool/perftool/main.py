import random
import tempfile
from pathlib import Path
import typer
import json
import shutil

from utils import (
    build_release,
    create_repo_from_commmit,
    sample_and_bench,
    write_into_csv,
)

app = typer.Typer()


def load_from_config():
    config_file = Path.cwd() / "config.json"
    config = json.load(Path.open(config_file, "r"))
    commit_1, commit_2, tmpfolder = (
        config["commit_1"],
        config["commit_2"],
        config["tmpfolder"],
    )
    return commit_1, commit_2, tmpfolder


def write_tmpfolder_name_into_config(tmpfolder_name: str):
    config_file = Path.cwd() / "config.json"
    config = json.load(Path.open(config_file, "r"))
    config["tmpfolder"] = tmpfolder_name
    json.dump(config, Path.open(config_file, "w"))
    return config_file


def build_repo(commit: str, tmpfolder: Path):
    commit_folder = tmpfolder / commit[:7]
    commit_folder.mkdir()
    create_repo_from_commmit(commit, str(commit_folder))
    cli_repo = commit_folder / "cli"
    build_release(cli_repo)


@app.command()
def bench(bench_function: str, min_value: int, max_value: int):
    commit_1, commit_2, tmpfolder = load_from_config()
    tmpfolder = Path(tmpfolder)
    if not tmpfolder.exists():
        print("Please run build command first")
        return
    commit_1_folder, commit_2_folder = (
        tmpfolder / commit_1[:7],
        tmpfolder / commit_2[:7],
    )
    cli_repo_1, cli_repo_2 = commit_1_folder / "cli", commit_2_folder / "cli"
    data_1_csv_file, data_2_csv_file = (
        tmpfolder / "data_1.csv",
        tmpfolder / "data_2.csv",
    )
    sample_data = {"values": [], "time_taken": []}

    # initialize the csv files with headers
    write_into_csv(sample_data, data_1_csv_file, headers=True)
    write_into_csv(sample_data, data_2_csv_file, headers=True)
    num_samples = 0
    while True:
        try:
            (cli_repo, data_csv_file) = random.choice(
                [(cli_repo_1, data_1_csv_file), (cli_repo_2, data_2_csv_file)]
            )
            data = sample_and_bench(cli_repo, bench_function, min_value, max_value)
            write_into_csv(data, data_csv_file, headers=False)
            num_samples += 1
        except KeyboardInterrupt:
            print("Press Ctrl-C again to clean files and exit")
            try:
                while True:
                    pass
            except KeyboardInterrupt:
                print("cleaning csv files")
            data_1_csv_file.unlink()
            data_2_csv_file.unlink()
            print("Exiting...")
            break
    print(f"sampled {num_samples} number of times")


@app.command()
def build():
    commit_1, commit_2, tmpfolder = load_from_config()
    tmpfolder = tempfile.mkdtemp()
    write_tmpfolder_name_into_config(tmpfolder)
    build_repo(commit_1, Path(tmpfolder))
    build_repo(commit_2, Path(tmpfolder))


@app.command()
def clean():
    _, _, tmpfolder = load_from_config()
    tmpfolder = Path(tmpfolder)
    if not tmpfolder.exists():
        print("Please run build command first")
        return
    # ensure we delete only stuff in tmp. Works only on linux (maybe)
    # Mainly intended to prevent deletion of unintended files by accident
    # TODO: Make this platform independent
    tmp = Path("/tmp")
    if tmp in tmpfolder.parents:
        shutil.rmtree(tmpfolder)
        write_tmpfolder_name_into_config("")
    print("Cleaned successfully")


if __name__ == "__main__":
    app()


# TODO: Keep the entire commit string, but shorten it only for the plots
# TODO: Don't delete anything when Ctrl+C in bench command
# TODO: Instead of putting csv files in tmp, put it in some folder like data/benchmark_name/commit_name.csv where
#       it won't be deleted at reboot
# TODO: Plotter will take input as the benchmark which will then plot csv files in the corresponding folder.
# TODO (Optional): Plotter can take optional argument that will override and result in plotting only input commit(s)
# TODO: Add function to save the plot as svg file. No need to show the plot. We can then "watch" in browser.
# TODO: Add more info the plots like
#       - Description of the benchmark
#       - proper legend
#       - proper title
#       - proper x-axis label
#       - proper y-axis label with unit (of time or could be memory). Avoid underscore in labels
# TODO: Don't show info about slope and intercept. If needed, we can put in stdout.
# TODO: use shapes with colors to differentiate between different plots for points as well as lines.
# TODO: use smaller dots.
# TODO: don't store the name of tmp folder into which we build. Instead create symbolic link from ./build(s) into some
#       identifiable tmp The clean command can take the symbolic link, follow it, and delete the folder in tmp and link.
# TODO: Make the commit names in config.json to be made more descriptive by user (like "with opt level 1") which can
#       then be used in plot.
