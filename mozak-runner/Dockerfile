FROM myoung34/github-runner:latest

# Download nix installer
RUN curl --proto '=https' \
  --tlsv1.2 \
  --silent \
  --show-error \
  --fail \
  --location https://install.determinate.systems/nix \
  # Pipe the installation script to shell
  | sh -s -- \
  install linux \
  # Disable sandbox
  --extra-conf "sandbox = false" \
  # Do nothing as we will be manually starting the nix daemon
  --init none \
  --no-confirm

ENV PATH="/nix/var/nix/profiles/default/bin:${PATH}"
