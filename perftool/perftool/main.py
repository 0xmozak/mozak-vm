import random
import tempfile
from pathlib import Path
import typer
import shutil

from utils import (
    build_release,
    create_repo_from_commmit,
    init_csv,
    load_bench_function_data,
    sample_and_bench,
    write_into_csv,
)

app = typer.Typer()


def load_commits_from_config(bench_function: str) -> dict:
    bench_function_data = load_bench_function_data(bench_function)
    commits = bench_function_data["commits"]
    return commits


def build_repo(commit: str, tmpfolder: Path):
    commit_folder = tmpfolder / commit
    try:
        commit_folder.mkdir()
    except FileExistsError as e:
        print(f"{e}")
        print(f"Skipping build for {commit}...")
        return
    create_repo_from_commmit(commit, str(commit_folder))
    cli_repo = commit_folder / "cli"
    build_release(cli_repo)


def create_symlink_for_repo(commit: str, tmpfolder: Path, bench_function: str):
    commit_folder = tmpfolder / commit
    build_folder = Path.cwd() / "build"
    build_folder.mkdir(exist_ok=True)
    bench_folder = build_folder / bench_function
    bench_folder.mkdir(exist_ok=True)
    commit_link = bench_folder / commit
    try:
        commit_link.symlink_to(commit_folder)
    except FileExistsError as e:
        print(f"{e}")
        print(f"Skipping symlink for {commit}...")
        return


@app.command()
def bench(bench_function: str, min_value: int, max_value: int):
    commits = load_commits_from_config(bench_function)
    commit_1, commit_2 = list(commits.values())[:2]  # change later

    build_folder = Path.cwd() / "build"
    build_folder.mkdir(exist_ok=True)
    bench_folder = build_folder / bench_function
    commit_1_symlink = bench_folder / commit_1
    commit_2_symlink = bench_folder / commit_2
    commit_1_folder, commit_2_folder = (
        commit_1_symlink.resolve(),
        commit_2_symlink.resolve(),
    )
    cli_repo_1, cli_repo_2 = commit_1_folder / "cli", commit_2_folder / "cli"

    data_folder = Path.cwd() / "data"
    data_folder.mkdir(exist_ok=True)
    bench_folder = data_folder / bench_function
    bench_folder.mkdir(exist_ok=True)
    data_1_csv_file, data_2_csv_file = (
        bench_folder / f"{commit_1}.csv",
        bench_folder / f"{commit_2}.csv",
    )

    # initialize the csv files with headers if it does not exist
    init_csv(data_1_csv_file, bench_function)
    init_csv(data_2_csv_file, bench_function)
    num_samples = 0
    while True:
        try:
            (cli_repo, data_csv_file) = random.choice(
                [(cli_repo_1, data_1_csv_file), (cli_repo_2, data_2_csv_file)]
            )
            data = sample_and_bench(cli_repo, bench_function, min_value, max_value)
            write_into_csv(data, data_csv_file)
            num_samples += 1
        except KeyboardInterrupt:
            print("Exiting...")
            break
    print(f"sampled {num_samples} number of times")


@app.command()
def build(bench_function: str):
    commits = load_commits_from_config(bench_function)
    commit_1, commit_2 = list(commits.values())[:2]  # change later
    tmpfolder = Path(tempfile.gettempdir()) / "Perftool_Repos_tmp"
    tmpfolder.mkdir(exist_ok=True)
    build_repo(commit_1, tmpfolder)
    build_repo(commit_2, tmpfolder)
    create_symlink_for_repo(commit_1, tmpfolder, bench_function)
    create_symlink_for_repo(commit_2, tmpfolder, bench_function)
    print(f"Bench {bench_function} built succesfully.")


@app.command()
def clean(bench_function: str):
    bench_commits_folder = Path.cwd() / "build" / bench_function
    for commit_symlink in bench_commits_folder.iterdir():
        commit_folder = commit_symlink.resolve()
        if commit_folder.is_dir():
            shutil.rmtree(commit_folder)
        commit_symlink.unlink()
    bench_commits_folder.rmdir()
    print("Cleaned successfully")


if __name__ == "__main__":
    app()


# TODO: Keep the entire commit string, but shorten it only for the plots: Done
# TODO: Don't delete anything when Ctrl+C in bench command: Done
# TODO: Instead of putting csv files in tmp, put it in some folder like data/benchmark_name/commit_name.csv where
#       it won't be deleted at reboot: Done
# TODO: Plotter will take input as the benchmark which will then plot csv files in the corresponding folder.:Done
# TODO (Optional): Plotter can take optional argument that will override and result in plotting only input commit(s)
# TODO: Add function to save the plot as svg file. No need to show the plot. We can then "watch" in browser.: Done
# TODO: Add more info the plots like
#       - Description of the benchmark: Done
#       - proper legend: Done
#       - proper title: Done
#       - proper x-axis label: Done
#       - proper y-axis label with unit (of time or could be memory). Avoid underscore in labels: Done
# TODO: Don't show info about slope and intercept. If needed, we can put in stdout.: Done
# TODO: use shapes with colors to differentiate between different plots for points as well as lines.: Done
# TODO: use smaller dots.: Done
# TODO: don't store the name of tmp folder into which we build. Instead create symbolic link from ./build(s) into some
#       identifiable tmp The clean command can take the symbolic link, follow it, and delete the folder in
#       tmp and link: Done
# TODO: Make the commit names in config.json to be made more descriptive by user (like "with opt level 1") which can
#       then be used in plot.:Done
# TODO: Support multiple commits.
