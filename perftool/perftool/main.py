import tempfile
from pathlib import Path
import typer
import json

from utils import (
    bench_all_values,
    build_release,
    create_repo_from_commmit,
    plot_both,
    write_into_csv,
)

app = typer.Typer()


def load_from_config():
    config_file = Path.cwd() / "perftool" / "config.json"
    config = json.load(Path.open(config_file, "r"))
    commit_1 = config["commit_1"]
    commit_2 = config["commit_2"]
    return commit_1, commit_2


def prepare(commit_1: str, commit_2: str, tmpfolder):
    tmpfolder = Path(tmpfolder)
    commit_1_folder = tmpfolder / commit_1[:7]
    commit_2_folder = tmpfolder / commit_2[:7]
    cli_repo_1 = commit_1_folder / "cli"
    cli_repo_2 = commit_2_folder / "cli"
    commit_1_folder.mkdir()
    commit_2_folder.mkdir()
    create_repo_from_commmit(commit_1, str(commit_1_folder))
    create_repo_from_commmit(commit_2, str(commit_2_folder))
    build_release(cli_repo_1)
    build_release(cli_repo_2)
    return cli_repo_1, cli_repo_2


@app.command()
def bench(bench_function: str, num_samples: int, min_value: int, max_value: int):
    commit_1, commit_2 = load_from_config()
    with tempfile.TemporaryDirectory() as tmpfolder:
        cli_repo_1, cli_repo_2 = prepare(commit_1, commit_2, tmpfolder)
        data_1 = bench_all_values(
            cli_repo_1, bench_function, num_samples, min_value, max_value
        )
        data_2 = bench_all_values(
            cli_repo_2, bench_function, num_samples, min_value, max_value
        )
        data_1_csv_file = Path("data_1.csv")
        data_2_csv_file = Path("data_2.csv")
        write_into_csv(data_1, data_1_csv_file)
        write_into_csv(data_2, data_2_csv_file)
        plot_both(data_1_csv_file, data_2_csv_file, bench_function)


if __name__ == "__main__":
    app()
