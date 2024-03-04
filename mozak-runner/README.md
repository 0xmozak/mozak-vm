# Self-Hosted GitHub Runner for Mozak

## Setup

Copy `tempalte.env` to `.env` and edit it.  Make sure to provide your
GitHub PAT token as outlined by [Enroll Sekf-Hosted CI Runner Notion
Page](https://www.notion.so/0xmozak/Enroll-Self-Hosted-CI-Runner-af6ddd3897594970b6ec4106ebde228f?pvs=4).

```shell
$ cp tempalte.env .env
```

Verify your configuration using `docker compose config`.  It should
print out your configuration.

```shell
$ docker compose config
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

