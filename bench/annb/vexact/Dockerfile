FROM ann-benchmarks

RUN apt-get update && apt-get install -y curl
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain 1.97.0
ENV PATH=/root/.cargo/bin:$PATH
RUN pip install scipy 'maturin>=1.7,<2'

# rust/ and combined/ at the pinned revision, and the bindings, put here by bench/annb/install.sh
COPY ann_benchmarks/algorithms/vexact/src /home/app/vexact
RUN cd /home/app/vexact/bench/annb/vexact-py \
 && RUSTFLAGS="-C target-cpu=native" CARGO_TARGET_DIR=/tmp/target maturin build --release -o /tmp/wheels \
 && pip install /tmp/wheels/*.whl && rm -rf /tmp/target /tmp/wheels
RUN python -c 'import vexact_py'
