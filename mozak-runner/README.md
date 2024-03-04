# Self-Hosted GitHub Runner for Mozak

[Compose file](https://docs.docker.com/compose/compose-file/) for
running self-hosted GitHub Runners with
- `nix` preinstalled,
- shared `nix` daemon, and
- shared `nix` store in a separate Docker volume

using
- [docker compose](https://docs.docker.com/compose/),
- [myoung34/github-runner](https://github.com/myoung34/docker-github-actions-runner?tab=readme-ov-file), and
- [DeterminateSystems/nix-installer](https://github.com/DeterminateSystems/nix-installer).

Starting from 2020, `docker compose` ships with all installations of
Docker Desktop.  In case it is missing on your system, manual
installation instructions are available on [Docker
Documentation](https://docs.docker.com/compose/migrate/#how-do-i-switch-to-compose-v2).

## Setup

Copy `template.env` to `.env` and edit it.  Make sure to provide your
GitHub PAT token as outlined by [Enroll Sekf-Hosted CI Runner Notion
Page](https://www.notion.so/0xmozak/Enroll-Self-Hosted-CI-Runner-af6ddd3897594970b6ec4106ebde228f?pvs=4).

Since we will be putting our PAT into `.env`, we need to restrict it's
permissions.  Please ensure that it has either

- `600` (readable and writeable by the owner), or even better
- `400` (readable by the owner) after editing it.

```shell
$ cp tempalte.env .env
$ chmod 600 .env
```

After editing `.env` file, please restrict it's permissions to `400`.

```shell
$ chmod 400 .env
```

Verify your configuration using `docker compose config`.  It should
print out your configuration.  Please verify that your configuration
from `.env` has been properly loaded by inspecting:

- `replicas`,
- `ACCESS_TOKEN`,
- `RUNNER_NAME_PREFIX`,
- volume's `source:` to contain your `MOZAK_RUNNER_CACHE`.

```shell
$ docker compose config \
  grep -E 'replicas|ACCESS_TOKEN|RUNNER_NAME_PREFIX|source'

      source: nix-store
      replicas: 3
      ACCESS_TOKEN: github_pat_<rest-of-your-token>
      RUNNER_NAME_PREFIX: <your runner prefix>
        source: /var/run/docker.sock
        source: <path specified in MOZAK_RUNNER_CACHE>
        source: nix-store
```

## Building Containers

You can build containers using `docker compose build`.

```shell
$ docker compose build
[+] Building 0.1s (9/9) FINISHED                                                                              docker:orbstack
 => [nix internal] load build definition from Dockerfile                                                                 0.0s
 => => transferring dockerfile: 488B                                                                                     0.0s
 => [runner internal] load metadata for docker.io/myoung34/github-runner:latest                                          0.0s
 => [nix internal] load .dockerignore                                                                                    0.0s
 => => transferring context: 42B                                                                                         0.0s
 => [runner 1/2] FROM docker.io/myoung34/github-runner:latest                                                            0.0s
 => CACHED [runner 2/2] RUN curl --proto '=https' --tlsv1.2 --silent --show-error --fail --location https://install.det  0.0s
 => [nix] exporting to image                                                                                             0.0s
 => => exporting layers                                                                                                  0.0s
 => => writing image sha256:20c8bfe3f84458a1af3aa2c035224e88445aa1b6e2c60006252346f31eef24ff                             0.0s
 => => naming to docker.io/library/mozak-nix                                                                             0.0s
 => [runner internal] load build definition from Dockerfile                                                              0.0s
 => => transferring dockerfile: 488B                                                                                     0.0s
 => [runner internal] load .dockerignore                                                                                 0.0s
 => => transferring context: 42B                                                                                         0.0s
 => [runner] exporting to image                                                                                          0.0s
 => => exporting layers                                                                                                  0.0s
 => => writing image sha256:ca8b18a9d6d49c2865b48f21eed0067570ad1d58c634f58175a3cf2ce5a3938a                             0.0s
 => => naming to docker.io/library/mozak-runner                                                                          0.0s
```

## Start

Start the build agents using the `--wait` flag, it will wait for the
`nix` daemon to pass health checks and detach from the console.
Please note that on the first run it might take around a minute to
pass the health check, depending on your network connection.

```shell
$ docker compose up --wait
[+] Running 6/6
 ✔ Network mozak_default     Created
 ✔ Volume "mozak_nix-store"  Created
 ✔ Container mozak-nix-1     Healthy
 ✔ Container mozak-runner-3  Started
 ✔ Container mozak-runner-1  Started
 ✔ Container mozak-runner-2  Started
```

## Checking Status

You can check the status of build agents using `docker compose top`:

```shell
$ docker compose top
mozak-nix-1
PID     USER   TIME   COMMAND
22964   root   0:00   /root/.nix-profile/bin/nix daemon

mozak-runner-1
PID     USER   TIME   COMMAND
23201   root   0:00   {entrypoint.sh} /usr/bin/dumb-init /bin/bash /entrypoint.sh ./bin/Runner.Listener run --startuptype service
23233   root   0:00   /bin/bash /entrypoint.sh ./bin/Runner.Listener run --startuptype service
23455   root   0:00   ./bin/Runner.Listener run --startuptype service

mozak-runner-2
PID     USER   TIME   COMMAND
23066   root   0:00   {entrypoint.sh} /usr/bin/dumb-init /bin/bash /entrypoint.sh ./bin/Runner.Listener run --startuptype service
23096   root   0:00   /bin/bash /entrypoint.sh ./bin/Runner.Listener run --startuptype service
23442   root   0:00   ./bin/Runner.Listener run --startuptype service

mozak-runner-3
PID     USER   TIME   COMMAND
23133   root   0:00   {entrypoint.sh} /usr/bin/dumb-init /bin/bash /entrypoint.sh ./bin/Runner.Listener run --startuptype service
23165   root   0:00   /bin/bash /entrypoint.sh ./bin/Runner.Listener run --startuptype service
23467   root   0:00   ./bin/Runner.Listener run --startuptype service
```

## Check health

You can check the status by using `docker ps --all --filter "name=mozak"`

```shell
$ docker ps --all --filter "name=mozak"
CONTAINER ID   IMAGE          COMMAND                  CREATED         STATUS                          PORTS     NAMES
0c323ceffd32   mozak-runner   "/entrypoint.sh ./bi…"   2 minutes ago   Exited (1) About a minute ago             mozak-runner-1
8d582c9be309   mozak-runner   "/entrypoint.sh ./bi…"   2 minutes ago   Exited (1) About a minute ago             mozak-runner-2
c9d2032b5167   mozak-runner   "/entrypoint.sh ./bi…"   2 minutes ago   Exited (1) About a minute ago             mozak-runner-3
6024270109db   mozak-nix      "/root/.nix-profile/…"   2 minutes ago   Up 2 minutes (healthy)                    mozak-nix-1
```

## Stopping and Starting Builders

You can stop the build agents using `docker compose stop`.  It will
not remove them.

```shell
$ docker compose stop
[+] Stopping 4/4
 ✔ Container mozak-runner-3  Stopped
 ✔ Container mozak-runner-2  Stopped
 ✔ Container mozak-runner-1  Stopped
 ✔ Container mozak-nix-1     Stopped
```

You can restart the build agents using `docker compose start`

```shell
docker compose start
[+] Running 4/3
 ✔ Container mozak-nix-1     Healthy
 ✔ Container mozak-runner-2  Started
 ✔ Container mozak-runner-3  Started
 ✔ Container mozak-runner-1  Started
```

Please note that when starting build agents will wait for the nix
daemon to pass the health check.

## Teardown

You can teardown all the containers using `docker compose down`.

```shell
$ docker compose down
[+] Running 5/0
 ✔ Container mozak-runner-3  Removed
 ✔ Container mozak-runner-1  Removed
 ✔ Container mozak-runner-2  Removed
 ✔ Container mozak-nix-1     Removed
 ✔ Network mozak_default     Removed
```

Please note that it will _not_ remove the `nix-store` volume.  If you
want to remove the `nix-store` volume, please pass `--volumes` flag

```shell
$ docker compose down --volumes
[+] Running 6/6
 ✔ Container mozak-runner-3  Removed
 ✔ Container mozak-runner-2  Removed
 ✔ Container mozak-runner-1  Removed
 ✔ Container mozak-nix-1     Removed
 ✔ Volume mozak_nix-store    Removed
 ✔ Network mozak_default     Removed
```

