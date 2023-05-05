ARG ALPINE_VERSION=3.15.0

#
# for planning and caching rust deps
# https://github.com/LukeMathWalker/cargo-chef
#
FROM clux/muslrust:nightly AS chef

# install cargo chef for caching rust deps
RUN cargo install cargo-chef --locked 

WORKDIR /app

FROM chef as planner

COPY {{ path }} .

# create recipe for rust deps using cargo chef
RUN cargo chef prepare --recipe-path recipe.json

#
# Build rust
#
FROM chef as rust

# Get the recipe from planner step
COPY --from=planner /app/recipe.json recipe.json

# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --target x86_64-unknown-linux-musl --recipe-path recipe.json

# Copy over the source code
COPY {{ path }} .

# build binary
RUN cargo build --release --target x86_64-unknown-linux-musl --bin {{ bin }}

#
# Copy the binary in a clean alpine image and zip it
#
FROM alpine:${ALPINE_VERSION} as builder

RUN apk add --no-cache zip

WORKDIR /opt/app

# Copy binary
COPY --from=rust /app/target/x86_64-unknown-linux-musl/release/{{ bin }} .

# Rename binary to bootstrap
RUN mv ./{{ bin }} ./bootstrap

# Zip the binary
RUN zip -r ./bootstrap.zip ./bootstrap  

# Remove the binary
RUN rm ./bootstrap

#
# Copy the binary in a image and run it
#
FROM public.ecr.aws/lambda/provided:al2 as runner

# Copy binary
COPY --from=rust /app/target/x86_64-unknown-linux-musl/release/{{ bin }} .

# Rename binary to bootstrap
RUN mv ./{{ bin }} ./bootstrap

ADD https://github.com/aws/aws-lambda-runtime-interface-emulator/releases/latest/download/aws-lambda-rie /usr/bin/aws-lambda-rie
RUN chmod 755 /usr/bin/aws-lambda-rie

ENTRYPOINT [ "/usr/bin/aws-lambda-rie", "./bootstrap" ]



