import random
from path import (
    create_folders_if_not_exist,
    create_symlink_for_repo,
    delete_folder_if_no_symlink,
    get_actual_cli_repo,
    get_actual_commit_folder,
    get_bench_folder,
    get_cli_repo,
    get_data_csv_file,
    get_data_folder,
)
import typer

from .utils import (
    build_release,
    create_repo_from_commit,
    init_csv,
    load_bench_function_data,
    maybe_build_ELF,
    sample_and_bench,
    write_into_csv,
)

TEMPFOLDERNAME: str = "Perftool_Repos_tmp"

app = typer.Typer()


def load_commits_from_config(bench_function: str) -> dict[str, str]:
    return load_bench_function_data(bench_function)["commits"]


def build_repo(commit: str):
    try:
        get_actual_commit_folder(commit).mkdir()
    except FileExistsError:
        pass
    create_repo_from_commit(commit)
    cli_repo = get_actual_cli_repo(commit)
    build_release(cli_repo)


@app.command()
def bench(bench_function: str, min_value: int, max_value: int):
    """
    Bench  `bench_function` with parameter sampled in range `(min_value, max_value)`
    It keeps sampling parameter, benches the function and updates the data csv file,
      till terminated by Ctrl+C
    """
    bench_commits = load_commits_from_config(bench_function)
    commits = list(commit for commit in bench_commits.values())

    # initialize the csv files with headers if they does not exist
    for commit in commits:
        data_csv_file = get_data_csv_file(bench_function, commit)
        init_csv(data_csv_file, bench_function)

    while True:
        try:
            commit = random.choice(commits)
            cli_repo = get_cli_repo(bench_function, commit)
            data = sample_and_bench(cli_repo, bench_function, min_value, max_value)
            data_csv_file = get_data_csv_file(bench_function, commit)
            write_into_csv(data, data_csv_file)
        except KeyboardInterrupt:
            print("Exiting...")
            break


@app.command()
def build(bench_function: str):
    """
    Build all the commits specified in `config.json` for given `bench_function`,
      in `--release` mode.
    """
    bench_commits = load_commits_from_config(bench_function)
    create_folders_if_not_exist(bench_function)
    for commit in bench_commits.values():
        build_repo(commit)
        create_symlink_for_repo(bench_function, commit)
        maybe_build_ELF(bench_function, commit)
    print(f"Bench {bench_function} built successfully.")


@app.command()
def clean(bench_function: str):
    """
    Clean all the built commits specified in `config.json` for given `bench_function`
    NOTE: This does not clean the csv data files, so that it can still be plotted
    """
    bench_commits_folder = get_bench_folder(bench_function)
    try:
        for commit_symlink in bench_commits_folder.iterdir():
            commit_folder = commit_symlink.resolve()
            commit_symlink.unlink()
            # Delete the actual commit folder if it has no more symlinks into it
            delete_folder_if_no_symlink(commit_folder)
        bench_commits_folder.rmdir()
        print("Repos cleaned successfully")
    except NotADirectoryError:
        print("No bench commits found")


@app.command()
def cleancsv(bench_function: str):
    """
    Clean all the data csv files corresponding to given `bench_function`
    """
    bench_data_folder = get_data_folder(bench_function)
    try:
        for commit_csv_file in bench_data_folder.iterdir():
            commit_csv_file.unlink()
        bench_data_folder.rmdir()
        print("Csv files cleaned successfully")
    except NotADirectoryError:
        print("No bench csv files found")


if __name__ == "__main__":
    app()
