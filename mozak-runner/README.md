# Self-Hosted GitHub Runner for Mozak

## Setup

Copy `.env.in` to `.env` and edit it.

``` shell
$ cp .env.in .env
```

Verify your configuration using `docker compose config`.  It should
print out your configuration.

``` shell
$ docker compose config
```

## Start

You can start the build agents using `docker compose up`.  If you want
to start it in the background pass `-d`:

``` shell
$ docker compose up -d
[+] Running 6/6
 ✔ Network mozak_default     Created
 ✔ Volume "mozak_nix-store"  Created
 ✔ Container mozak-nix-1     Healthy
 ✔ Container mozak-runner-3  Started
 ✔ Container mozak-runner-1  Started
 ✔ Container mozak-runner-2  Started
```


If you start it in the foreground, be wary that build agents are
waiting for `nix` to become `healthy`, which may take up to a minute
on the first run.  Every subsequent run should be quite fast.

## Check health

You can check the status by using `docker ps -a`

``` shell
$ docker ps -a
CONTAINER ID   IMAGE          COMMAND                  CREATED         STATUS                          PORTS     NAMES
0c323ceffd32   mozak-runner   "/entrypoint.sh ./bi…"   2 minutes ago   Exited (1) About a minute ago             mozak-runner-1
8d582c9be309   mozak-runner   "/entrypoint.sh ./bi…"   2 minutes ago   Exited (1) About a minute ago             mozak-runner-2
c9d2032b5167   mozak-runner   "/entrypoint.sh ./bi…"   2 minutes ago   Exited (1) About a minute ago             mozak-runner-3
6024270109db   mozak-nix      "/root/.nix-profile/…"   2 minutes ago   Up 2 minutes (healthy)                    mozak-nix-1
```

## Stop

You can stop the build agents using `docker compose stop`.  It will
not remove them.

``` shell
$ docker compose stop
[+] Stopping 4/4
 ✔ Container mozak-runner-3  Stopped
 ✔ Container mozak-runner-2  Stopped
 ✔ Container mozak-runner-1  Stopped
 ✔ Container mozak-nix-1     Stopped
```

## Teardown

You can teardown all the containers using `docker compose down`.

``` shell
$ docker compose down
[+] Running 5/0
 ✔ Container mozak-runner-3  Removed
 ✔ Container mozak-runner-1  Removed
 ✔ Container mozak-runner-2  Removed
 ✔ Container mozak-nix-1     Removed
 ✔ Network mozak_default     Removed
```

Please note that it will _not_ remove the `nix-store` volume.  If you
want to remove the `nix-store` volume, please pass `-v` flag

``` shell
$ docker compose down -v
[+] Running 6/6
 ✔ Container mozak-runner-3  Removed
 ✔ Container mozak-runner-2  Removed
 ✔ Container mozak-runner-1  Removed
 ✔ Container mozak-nix-1     Removed
 ✔ Volume mozak_nix-store    Removed
 ✔ Network mozak_default     Removed
```

