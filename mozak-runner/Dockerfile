FROM myoung34/github-runner:latest

# Download nix installer
RUN curl --proto '=https' \
  --tlsv1.2 \
  --silent \
  --show-error \
  --fail \
  --location https://install.determinate.systems/nix \
  | sh -s -- \
  install linux \
  --extra-conf "sandbox = false" \
  --extra-conf "filter-syscalls = false" \
  # Do nothing as we will manually start the nix daemon
  --init none \
  --no-confirm

ENV PATH="/nix/var/nix/profiles/default/bin:${PATH}"

LABEL maintainer="maintainer@mozak.com"

