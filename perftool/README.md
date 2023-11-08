## Installing and setting up Poetry

- Installation instructions can be found [here](https://python-poetry.org/docs/)
- To initialize the project, execute `poetry install`
- Enter poetry shell with `poetry shell`

## Config file

- specify the benches and other data in `config.json`

## Building and cleaning

- Build the commits with `perftool build {bench_name}`, for example `perftool build sample-bench`.  This is necessary before calling benches.
  - This builds the repo inside `Perftools_Repos_tmp` within system temp folder, and creates their symlinks in `build/{bench_name}` folder.
- When we no longer need the repos, clean with `perftool clean {bench_name}`
  - This will remove the symlinks as well as the repos inside temp folder.
  - This will not remove the csv files, so that we can still plot them later.
- When we no longer need the csv data, clean with `perftool cleancsv {bench_name}`.

## How to run

Ensure the repos are built first. After that, open two terminals and run following command on each respectively

- `perftool bench {bench_name} {min_value} {max_value}` (you can try out `perftool bench sample-bench 10 100`)
  - This samples data and stores it in csv files in folder `data/{bench_name}` until terminated, eg with Control+C.
- `plot {bench_name}`
  - This command polls the data from csv files every 5 seconds, creates a plot and saves it in `plots/{bench_name}.png`
