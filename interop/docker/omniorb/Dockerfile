# syntax=docker/dockerfile:1
#
# omniORB / omniORBpy / omniNames image for the rustorb interop tests.
#
# Build context: the interop/ directory.
#   docker build -f interop/docker/omniorb/Dockerfile -t rustorb-interop-omniorb interop
#
# omniORBpy is not packaged for Ubuntu 24.04, so both omniORB and omniORBpy
# are built from the upstream release tarballs, pinned by version and sha256.

ARG UBUNTU_VERSION=24.04

FROM ubuntu:${UBUNTU_VERSION} AS omniorb-build

ARG OMNIORB_VERSION=4.3.4
ARG OMNIORB_SHA256=79720d415d23cd8da99287a4ef4da0aa1bd34d3e4c7b1530715600adc5ed3dc3
ARG OMNIORBPY_SHA256=a709c3c77b9c6b08616e1c9e12a5a9b9d5ccc1f2dcf6f647f205018d77f819a7

RUN apt-get update \
 && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
      build-essential pkg-config python3 python3-dev bzip2 \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /src
ADD --checksum=sha256:${OMNIORB_SHA256} \
    https://downloads.sourceforge.net/project/omniorb/omniORB/omniORB-${OMNIORB_VERSION}/omniORB-${OMNIORB_VERSION}.tar.bz2 \
    omniORB.tar.bz2
ADD --checksum=sha256:${OMNIORBPY_SHA256} \
    https://downloads.sourceforge.net/project/omniorb/omniORBpy/omniORBpy-${OMNIORB_VERSION}/omniORBpy-${OMNIORB_VERSION}.tar.bz2 \
    omniORBpy.tar.bz2
RUN tar xjf omniORB.tar.bz2 && tar xjf omniORBpy.tar.bz2

RUN cd omniORB-${OMNIORB_VERSION} && mkdir build && cd build \
 && ../configure --prefix=/opt/omniorb --disable-static PYTHON=/usr/bin/python3 \
 && make -j"$(nproc)" && make install

RUN cd omniORBpy-${OMNIORB_VERSION} && mkdir build && cd build \
 && ../configure --prefix=/opt/omniorb --with-omniorb=/opt/omniorb PYTHON=/usr/bin/python3 \
 && make -j"$(nproc)" && make install

# Make the omniORBpy site-packages directory version independent.
RUN ln -s "$(dirname "$(find /opt/omniorb -name 'omniORB' -path '*-packages/*' -type d | head -n1)")" \
      /opt/omniorb/python \
 && test -f /opt/omniorb/python/omniORB/__init__.py

ENV PATH=/opt/omniorb/bin:$PATH \
    LD_LIBRARY_PATH=/opt/omniorb/lib \
    PYTHONPATH=/opt/omniorb/python

# ---------------------------------------------------------------------------
# Test implementations: stubs generated from the test IDL, the C++ server and
# client, and the Python server and client.
FROM omniorb-build AS impl-build

COPY idl /opt/interop/idl
COPY impls/omniorb-cpp /src/omniorb-cpp
COPY impls/omniorbpy /opt/interop/impls/omniorbpy

RUN make -C /src/omniorb-cpp OMNIORB=/opt/omniorb IDLDIR=/opt/interop/idl \
      OUT=/opt/interop/impls/omniorb-cpp install
RUN mkdir -p /opt/interop/impls/omniorbpy/stubs \
 && omniidl -bpython -C /opt/interop/impls/omniorbpy/stubs \
      -I/opt/interop/idl /opt/interop/idl/InteropTest.idl

# ---------------------------------------------------------------------------
FROM ubuntu:${UBUNTU_VERSION}

RUN apt-get update \
 && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
      python3 libstdc++6 tcpdump iproute2 \
 && rm -rf /var/lib/apt/lists/*

COPY --from=omniorb-build /opt/omniorb /opt/omniorb
COPY --from=impl-build /opt/interop /opt/interop
COPY scripts /opt/interop/scripts

ENV PATH=/opt/omniorb/bin:/opt/interop/scripts:$PATH \
    LD_LIBRARY_PATH=/opt/omniorb/lib \
    PYTHONPATH=/opt/omniorb/python \
    PYTHONUNBUFFERED=1

WORKDIR /opt/interop
