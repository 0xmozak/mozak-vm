from pathlib import Path
from utils import *
import tempfile
from tqdm import tqdm

commit_1 = "28a5108e5f617d93ea055564c00c08844fee5a0a"
commit_2 = "3011dd0a5600d0203af4385dde8cf383fbf7a360"


def test_sample_bench():
    with tempfile.TemporaryDirectory() as tmpfolder:
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
        # time_taken = bench("sample_bench", 12, cli_repo)
        # print(f"time taken is {time_taken} ")


def test_in_tmp(rebuild: bool, commit_1: str, commit_2: str):
    bench_function = "sample-bench"
    tmpfolder = Path.cwd() / "tmp"
    tmpfolder.mkdir()
    commit_1_folder = tmpfolder / commit_1[:7]
    commit_2_folder = tmpfolder / commit_2[:7]
    cli_repo_1 = commit_1_folder / "cli"
    cli_repo_2 = commit_2_folder / "cli"
    if rebuild:
        commit_1_folder.mkdir()
        commit_2_folder.mkdir()
        create_repo_from_commmit(commit_1, str(commit_1_folder))
        create_repo_from_commmit(commit_2, str(commit_2_folder))
        build_release(cli_repo_1)
        build_release(cli_repo_2)
    data_1 = {"values": [], "time_taken (in s)": []}
    for value in tqdm(sample(num_samples=10, min_value=10, max_value=100, mean=20)):
        time_taken = bench(bench_function, value, cli_repo_1)
        data_1["values"].append(value)
        data_1["time_taken (in s)"].append(time_taken)
    data_2 = {"values": [], "time_taken (in s)": []}
    for value in tqdm(sample(num_samples=10, min_value=10, max_value=100, mean=20)):
        time_taken = bench(bench_function, value, cli_repo_2)
        data_2["values"].append(value)
        data_2["time_taken (in s)"].append(time_taken)
    data_1_csv_file = Path("data_1.csv")
    data_2_csv_file = Path("data_2.csv")
    write_into_csv(data_1, data_1_csv_file)
    write_into_csv(data_2, data_2_csv_file)
    plot_both(data_1_csv_file, data_2_csv_file, bench_function)


# test_sample_bench()
test_in_tmp(rebuild=False, commit_1=commit_1, commit_2=commit_2)
