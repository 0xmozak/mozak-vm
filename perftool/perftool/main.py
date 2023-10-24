import random
import tempfile
from pathlib import Path
import typer
import shutil

from .utils import (
    build_release,
    create_repo_from_commit,
    get_cli_repo,
    get_csv_file,
    init_csv,
    load_bench_function_data,
    sample_and_bench,
    write_into_csv,
)

TEMPFOLDERNAME: str = "Perftool_Repos_tmp"

app = typer.Typer()


def load_commits_from_config(bench_function: str) -> dict[str, str]:
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
    create_repo_from_commmit(commit, commit_folder)
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
    bench_commits = load_commits_from_config(bench_function)
    # create build folder if it doesn't exist
    build_folder = Path.cwd() / "build"
    build_folder.mkdir(exist_ok=True)
    # create bench folder if it doesn't exist
    bench_folder = build_folder / bench_function
    bench_folder.mkdir(exist_ok=True)
    # create data folder if it doesn't exist
    data_folder = Path.cwd() / "data"
    data_folder.mkdir(exist_ok=True)
    data_bench_folder = data_folder / bench_function
    data_bench_folder.mkdir(exist_ok=True)
    commits = list(commit for (_commit_description, commit) in bench_commits.items())
    # initialize the csv files with headers if they does not exist
    for commit in commits:
        data_csv_file = data_bench_folder / f"{commit}.csv"
        init_csv(data_csv_file, bench_function)

    num_samples = 0
    while True:
        try:
            commit = random.choice(commits)
            cli_repo = get_cli_repo(commit, bench_function)
            data = sample_and_bench(cli_repo, bench_function, min_value, max_value)
            data_csv_file = get_csv_file(commit, bench_function)
            write_into_csv(data, data_csv_file)
            num_samples += 1
        except KeyboardInterrupt:
            print("Exiting...")
            break
    print(f"sampled {num_samples} number of times")


@app.command()
def build(bench_function: str):
    bench_commits = load_commits_from_config(bench_function)
    tmpfolder = Path(tempfile.gettempdir()) / TEMPFOLDERNAME
    tmpfolder.mkdir(exist_ok=True)
    for commit_description, commit in bench_commits.items():
        build_repo(commit, tmpfolder)
        create_symlink_for_repo(commit, tmpfolder, bench_function)
    print(f"Bench {bench_function} built succesfully.")


@app.command()
def clean(bench_function: str):
    bench_commits_folder = Path.cwd() / "build" / bench_function
    if not bench_commits_folder.exists():
        print("No bench commits found")
        return
    for commit_symlink in bench_commits_folder.iterdir():
        commit_folder = commit_symlink.resolve()
        if commit_folder.is_dir():
            shutil.rmtree(commit_folder)
        commit_symlink.unlink()
    bench_commits_folder.rmdir()
    print("Repos cleaned successfully")


@app.command()
def cleancsv(bench_function: str):
    bench_commits_folder = Path.cwd() / "data" / bench_function
    if not bench_commits_folder.exists():
        print("No bench csv files found")
        return
    for commit_csv_file in bench_commits_folder.iterdir():
        commit_csv_file.unlink()
    bench_commits_folder.rmdir()
    print("Csv files cleaned successfully")


if __name__ == "__main__":
    app()
