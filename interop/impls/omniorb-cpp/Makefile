# Builds the omniORB C++ server and client for the interop tests, and the
# POA probe server (run-poa-probe.sh).
#
#   make OMNIORB=/opt/omniorb IDLDIR=../../idl OUT=/opt/interop/impls/omniorb-cpp install

OMNIORB ?= /opt/omniorb
IDLDIR ?= ../../idl
OUT ?= out

OMNIIDL = $(OMNIORB)/bin/omniidl
CXXFLAGS ?= -O2 -g -Wall -Wno-unused
CPPFLAGS += -I$(OMNIORB)/include -Igen
LDLIBS += -L$(OMNIORB)/lib -lomniORB4 -lomnithread -lpthread

STUB_SRC = gen/InteropTestSK.cc
STUB_HDR = gen/InteropTest.hh
PROBE_SRC = gen/PoaProbeSK.cc
PROBE_HDR = gen/PoaProbe.hh

all: server client poa_probe_server

$(STUB_SRC) $(STUB_HDR): $(IDLDIR)/InteropTest.idl
	mkdir -p gen
	$(OMNIIDL) -bcxx -Wbh=.hh -Wbs=SK.cc -C gen -I$(IDLDIR) $<

$(PROBE_SRC) $(PROBE_HDR): $(IDLDIR)/PoaProbe.idl
	mkdir -p gen
	$(OMNIIDL) -bcxx -Wbh=.hh -Wbs=SK.cc -C gen -I$(IDLDIR) $<

server: server.cc common.h $(STUB_SRC) $(STUB_HDR)
	$(CXX) $(CPPFLAGS) $(CXXFLAGS) -o $@ server.cc $(STUB_SRC) $(LDLIBS)

client: client.cc common.h $(STUB_SRC) $(STUB_HDR)
	$(CXX) $(CPPFLAGS) $(CXXFLAGS) -o $@ client.cc $(STUB_SRC) $(LDLIBS)

poa_probe_server: poa_probe_server.cc $(PROBE_SRC) $(PROBE_HDR)
	$(CXX) $(CPPFLAGS) $(CXXFLAGS) -o $@ poa_probe_server.cc $(PROBE_SRC) $(LDLIBS)

install: server client poa_probe_server
	mkdir -p $(OUT)
	cp server client poa_probe_server $(OUT)/

clean:
	rm -rf gen server client poa_probe_server

.PHONY: all install clean
