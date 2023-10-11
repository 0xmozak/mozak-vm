import os
from utils import build_release, create_repo_from_commmit, run_bench
import tempfile


def test_sample_bench():
    with tempfile.TemporaryDirectory() as tmpfolder:
        # commit with cli bench functions
        create_repo_from_commmit("c1de304b57c0e2df70a07d4ebea33e7b70aa79f9", tmpfolder)
        cli_repo = os.path.join(tmpfolder, "cli")
        build_release(cli_repo)
        run_bench("sample_bench", 12, cli_repo)
