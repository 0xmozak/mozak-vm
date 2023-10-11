import os
from utils import *
import tempfile

commit = "5cc4cb0aebb35c6ae97c8fbf9e946d99ba6e8178"


def test_sample_bench():
    with tempfile.TemporaryDirectory() as tmpfolder:
        # commit with cli bench functions
        create_repo_from_commmit(commit, tmpfolder)
        cli_repo = os.path.join(tmpfolder, "cli")
        build_release(cli_repo)
        time_taken = bench("sample_bench", 12, cli_repo)
        print(f"time taken is {time_taken} ")


test_sample_bench()
# tmpfolder = "tmp"
# os.mkdir(tmpfolder)
# create_repo_from_commmit(commit, tmpfolder)
# cli_repo = os.path.join("tmp", "cli")
# build_release(cli_repo)
# time_taken = bench("sample_bench", 12, cli_repo)
# print(f"time taken is {time_taken} ")
