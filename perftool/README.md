## Config file
- specify the commits in `config.json`
- before building, ensure tmpfolder is set to empty string

## Building and cleaning
- Build the commits with `poetry run perftool/main.py build`. This is necessary before calling benches
- When you no longer need the repos, clean with `poetry run perftool/main.py clean`

## How to run

Ensure the repos are built first. After that, Make two terminals and run following command on each respectively
- `poetry run python perftool/main.py bench sample-bench {min_value} {max_value}` (you can try out min_value = 10, max_value = 100 to start with)
- `poetry run python plotter/plot.py sample-bench`

first command samples data points until terminated with Ctrl + C, while second command polls the data every 5 seconds, and plots it live unless terminated with Ctrl + C
