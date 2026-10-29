# syntax=docker/dockerfile:1
#
# omniORB / omniORBpy / omniNames (and JacORB) image for the rustorb interop tests.
#
# Build context: the interop/ directory.
#   docker build -f interop/docker/omniorb/Dockerfile -t rustorb-interop-omniorb interop
#
# omniORBpy is not packaged for Ubuntu 24.04, so both omniORB and omniORBpy
# are built from the upstream release tarballs, pinned by version and sha256.
# JacORB comes from its release jars on Maven Central (the jacorb-build stage).

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
      -I/opt/interop/idl /opt/interop/idl/InteropTest.idl /opt/interop/idl/PoaProbe.idl \
      /opt/interop/idl/AnyTest.idl /opt/interop/idl/AnyTestHidden.idl

# ---------------------------------------------------------------------------
# OpenRTM-aist (C++) on omniORB: the example RTCs that run-openrtm.sh connects
# with RTCs on rustorb. Pinned by commit; only the examples it uses are built.
FROM omniorb-build AS openrtm-build

ARG OPENRTM_REF=7e08ae03dcf613689aeef81ee32ceab763206a74

RUN apt-get update \
 && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
      cmake git ca-certificates uuid-dev \
 && rm -rf /var/lib/apt/lists/*

RUN git init -q /src/openrtm \
 && git -C /src/openrtm fetch -q --depth 1 https://github.com/OpenRTM/OpenRTM-aist.git "${OPENRTM_REF}" \
 && git -C /src/openrtm checkout -q FETCH_HEAD

RUN mkdir /src/openrtm/build && cd /src/openrtm/build \
 && PKG_CONFIG_PATH=/opt/omniorb/lib/pkgconfig cmake .. -DCMAKE_BUILD_TYPE=Release \
      -DFLUENTBIT_ENABLE=OFF -DSSL_ENABLE=OFF -DOBSERVER_ENABLE=OFF \
 && make -j"$(nproc)" ConsoleInComp ConsoleOutComp SeqInComp SeqOutComp \
      MyServiceProviderComp MyServiceConsumerComp \
 && mkdir -p /opt/openrtm/examples /opt/openrtm/lib \
 && find . -name 'libRTC2.so*' -exec cp -P {} /opt/openrtm/lib/ \; \
 && for c in ConsoleIn ConsoleOut SeqIn SeqOut MyServiceProvider MyServiceConsumer; do \
      cp "$(find . -name "${c}Comp" -type f -perm -u+x | head -n1)" /opt/openrtm/examples/; \
    done \
 && test -x /opt/openrtm/examples/MyServiceConsumerComp && ls /opt/openrtm/lib/libRTC2.so*

# ---------------------------------------------------------------------------
# JacORB (Java) for the jacorb client and server: the release jars from Maven
# Central, pinned by version and sha256, with stubs from JacORB's own IDL
# compiler and the Java sources shared with rustorbjava (impls/java).
# JacORB 3.9 uses javax.rmi.CORBA, which Java 11 dropped, so the javax.rmi
# API jar comes too.
FROM ubuntu:${UBUNTU_VERSION} AS jacorb-build

ARG JACORB_VERSION=3.9
ARG MAVEN=https://repo1.maven.org/maven2

RUN apt-get update \
 && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
      openjdk-17-jdk-headless \
 && rm -rf /var/lib/apt/lists/*

ADD --checksum=sha256:cb833df39c6e30caca29f2c244901c2d5566c8c6aac3ea59842024c5f5264ee2 \
    ${MAVEN}/org/jacorb/jacorb/${JACORB_VERSION}/jacorb-${JACORB_VERSION}.jar /opt/jacorb/lib/
ADD --checksum=sha256:247f565dfd4b0c1ee54b1e0a6288be84fc66ae961c34715171ca2867c6857447 \
    ${MAVEN}/org/jacorb/jacorb-omgapi/${JACORB_VERSION}/jacorb-omgapi-${JACORB_VERSION}.jar /opt/jacorb/lib/
ADD --checksum=sha256:b030a29e088dea60b07c7299d25f43cbd120502e10dcae3f382435ecd5de5ddd \
    ${MAVEN}/org/slf4j/slf4j-api/1.7.14/slf4j-api-1.7.14.jar /opt/jacorb/lib/
ADD --checksum=sha256:2983d6db3a53ab1496b24703763b9fcfea53caf6550dd2200066ab552cc9e69b \
    ${MAVEN}/org/slf4j/slf4j-jdk14/1.7.14/slf4j-jdk14-1.7.14.jar /opt/jacorb/lib/
ADD --checksum=sha256:07615653320a010a648d164e77751daddcfea5602148aa4da2f052bc6af0689e \
    ${MAVEN}/org/jboss/spec/javax/rmi/jboss-rmi-api_1.0_spec/1.0.6.Final/jboss-rmi-api_1.0_spec-1.0.6.Final.jar \
    /opt/jacorb/lib/
ADD --checksum=sha256:cc2ff5b6751eb6767a5d3f4e05c330ff6791d04c12609e09cf25459bbba829d7 \
    ${MAVEN}/org/jacorb/jacorb-idl-compiler/${JACORB_VERSION}/jacorb-idl-compiler-${JACORB_VERSION}.jar /src/jacorb-idl/
ADD --checksum=sha256:e58a2604a85ab985fb94037c889bef90b0c2a862030c03b0e830aa54a4931037 \
    ${MAVEN}/java_cup/java_cup/0.9e/java_cup-0.9e.jar /src/jacorb-idl/

COPY idl /opt/interop/idl
COPY impls/java /src/java
COPY impls/jacorb/idl /src/jacorb-idl/idl
RUN java -cp '/src/jacorb-idl/*' org.jacorb.idl.parser -d /src/jacorb-gen \
      /opt/interop/idl/InteropTest.idl \
 && java -cp '/src/jacorb-idl/*' org.jacorb.idl.parser -all -I/opt/interop/idl \
      -I/src/jacorb-idl/idl -d /src/jacorb-gen /src/jacorb-idl/idl/AnyAll.idl \
 && find /src/jacorb-gen /src/java -name '*.java' > /src/sources.txt \
 && javac --release 11 -encoding UTF-8 -nowarn -cp '/opt/jacorb/lib/*' -d /opt/interop/impls/jacorb/classes \
      @/src/sources.txt

# ---------------------------------------------------------------------------
FROM ubuntu:${UBUNTU_VERSION}

RUN apt-get update \
 && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
      python3 libstdc++6 libuuid1 tcpdump iproute2 openjdk-17-jre-headless \
 && rm -rf /var/lib/apt/lists/*

COPY --from=omniorb-build /opt/omniorb /opt/omniorb
COPY --from=impl-build /opt/interop /opt/interop
COPY --from=openrtm-build /opt/openrtm /opt/openrtm
COPY --from=jacorb-build /opt/jacorb /opt/jacorb
COPY --from=jacorb-build /opt/interop/impls/jacorb /opt/interop/impls/jacorb
COPY impls/jacorb/client impls/jacorb/server /opt/interop/impls/jacorb/
COPY scripts /opt/interop/scripts

ENV PATH=/opt/omniorb/bin:/opt/interop/scripts:$PATH \
    LD_LIBRARY_PATH=/opt/omniorb/lib:/opt/openrtm/lib \
    PYTHONPATH=/opt/omniorb/python \
    PYTHONUNBUFFERED=1

WORKDIR /opt/interop
