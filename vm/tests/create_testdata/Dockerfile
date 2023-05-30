FROM --platform=linux/amd64 ubuntu:latest as builder-stage

# Set the working directory
WORKDIR /root
COPY generate_testdata_from_this_commit .

# Install necessary tools to build RISC-V tests
RUN apt-get update && apt-get install --yes curl git autoconf g++ build-essential

# Download and extract RISC-V GNU toolchain binaries
RUN curl --fail --silent --show-error --location \
         https://github.com/riscv-collab/riscv-gnu-toolchain/releases/download/2023.05.19/riscv32-elf-ubuntu-22.04-nightly-2023.05.19-nightly.tar.gz \
         | tar --extract --gzip --verbose

# Set the environment variables
ENV PATH="/root/riscv/bin:${PATH}"
ENV RISCV="/root"

# Clone the RISC-V tests repository
# (Get only the specific commit we want)
WORKDIR /root/riscv-tests/
RUN git init --initial-branch=main \
    && git remote add origin https://github.com/riscv/riscv-tests.git \
    && git fetch origin --depth=3 "$(cat ../generate_testdata_from_this_commit)" \
    && git reset --hard FETCH_HEAD

# Set the environment variable for the RISC-V tests
ENV RISCV_TEST="/root/riscv-tests/isa"

# Update tests
RUN git submodule update --init --recursive && \
    git rev-parse HEAD | tee .testdata_generated_from_this_commit

# Build the tests - we are interested in rv32ui and rv32um
RUN autoconf && \
    ./configure --prefix=/root/riscv-tests && \
    cd isa && \
    make rv32ui XLEN=32 && \
    make rv32um XLEN=32

FROM scratch as exporter-stage

# Copy all built tests to the host system
COPY --from=builder-stage /root/riscv-tests/isa /
COPY --from=builder-stage /root/riscv-tests/.testdata_generated_from_this_commit /
