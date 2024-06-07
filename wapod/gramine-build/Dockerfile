FROM gramineproject/gramine:stable-jammy

RUN apt-get update
RUN apt-get install -y make gcc git rsync

ENV SGX 1

RUN gramine-sgx-gen-private-key

WORKDIR /root/

# Get Rust
RUN curl https://sh.rustup.rs -sSf | bash -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

RUN git clone https://github.com/Phala-Network/wapo.git --recursive

WORKDIR wapo/wapod/gramine-build

RUN make dist

ENTRYPOINT []
CMD [ "bash" ]
