import random
from config import get_benches_with_commit, get_output_name, get_parameter_name
from path import (
    create_folders_if_not_exist,
    create_symlink_for_repo,
    delete_folder_if_no_symlink,
    get_bench_folder,
    get_cli_repo,
    get_data_csv_file,
    get_data_folder,
)
import typer

from .utils import (
    build_repo,
    init_csv,
    maybe_build_ELF,
    sample_and_bench,
    write_into_csv,
)

app = typer.Typer()


@app.command()
def bench(bench_name: str, min_value: int, max_value: int):
    """
    Bench  `bench_name` with parameter sampled in range `(min_value, max_value)`
    It keeps sampling parameter, benches the function and updates the data csv file,
      till terminated by Ctrl+C
    """
    # TODO: make 'bench' automatically 'build', if necessary, and if not
    # explicitly forbidden.
    bench_commits_dict = get_benches_with_commit(bench_name)
    benches = list(
        bench_with_commit for bench_with_commit in bench_commits_dict.values()
    )

    parameter_name = get_parameter_name(bench_name)
    output_name = get_output_name(bench_name)

    # initialize the csv files with headers if they do not exist
    for bench in benches:
        bench_function = bench["bench_function"]
        commit = bench["commit"]
        data_csv_file = get_data_csv_file(
            bench_name, bench["bench_function"], bench["commit"]
        )
        init_csv(data_csv_file, bench_name)

    while True:
        try:
            bench = random.choice(benches)
            commit = bench["commit"]
            bench_function = bench["bench_function"]
            cli_repo = get_cli_repo(bench_name, commit)
            parameter, output = sample_and_bench(
                cli_repo, bench_function, min_value, max_value
            )
            data = {parameter_name: [parameter], output_name: [output]}
            data_csv_file = get_data_csv_file(bench_name, bench_function, commit)
            write_into_csv(data, data_csv_file)
            print(".", end="", flush=True)
        except KeyboardInterrupt:
            print("Exiting...")
            break
        except Exception as e:
            print(e)
            break


@app.command()
def build(bench_name: str):
    """
    Build all the commits specified in `config.json` for given `bench_function`,
      in `--release` mode.
    """
    bench_commits_dict = get_benches_with_commit(bench_name)
    create_folders_if_not_exist(bench_name)
    for bench_description, bench_with_commit in bench_commits_dict.items():
        commit = bench_with_commit["commit"]
        build_repo(commit)
        create_symlink_for_repo(bench_name, commit)
        maybe_build_ELF(bench_name, bench_description, commit)
    print(f"Bench {bench_name} built successfully.")


@app.command()
def clean(bench_name: str):
    """
    Clean all the built commits specified in `config.json` for given `bench_function`
    NOTE: This does not clean the csv data files, so that it can still be plotted
    """
    bench_commits_folder = get_bench_folder(bench_name)
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
def cleancsv(bench_name: str):
    """
    Clean all the data csv files corresponding to given `bench_function`
    """
    bench_data_folder = get_data_folder(bench_name)
    try:
        for commit_csv_file in bench_data_folder.iterdir():
            commit_csv_file.unlink()
        bench_data_folder.rmdir()
        print("Csv files cleaned successfully")
    except NotADirectoryError:
        print("No bench csv files found")


if __name__ == "__main__":
    app()
