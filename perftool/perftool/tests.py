import os
from utils import *
import tempfile
from tqdm import tqdm

commit = "80526ef9f23b239bae6fbd96a2bc237dabd30fdd"


def test_sample_bench():
    with tempfile.TemporaryDirectory() as tmpfolder:
        # commit with cli bench functions
        create_repo_from_commmit(commit, tmpfolder)
        cli_repo = os.path.join(tmpfolder, "cli")
        build_release(cli_repo)
        time_taken = bench("sample_bench", 12, cli_repo)
        print(f"time taken is {time_taken} ")


def test_in_tmp(rebuild: bool):
    bench_function = "sample_bench"
    tmpfolder = "tmp"
    cli_repo = os.path.join(tmpfolder, "cli")
    if rebuild:
        os.mkdir(tmpfolder)
        create_repo_from_commmit(commit, tmpfolder)
        build_release(cli_repo)
    data = {"values": [], "time_taken (in s)": []}
    for value in tqdm(sample(num_samples=30, min_value=100, max_value=1000, mean=200)):
        time_taken = bench(bench_function, value, cli_repo)
        data["values"].append(value)
        data["time_taken (in s)"].append(time_taken)
    write_into_csv(data, "data.csv")
    plot_csv_data("data.csv", bench_function)


# test_sample_bench()
test_in_tmp(rebuild=False)
