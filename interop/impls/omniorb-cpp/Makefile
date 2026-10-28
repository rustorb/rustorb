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
LDLIBS += -L$(OMNIORB)/lib -lomniDynamic4 -lomniORB4 -lomnithread -lpthread

STUB_SRC = gen/InteropTestSK.cc
STUB_HDR = gen/InteropTest.hh
PROBE_SRC = gen/PoaProbeSK.cc
PROBE_HDR = gen/PoaProbe.hh
# The stage 2 IDL, with Any support (-Wba): the servers know AnyTest, the
# clients also AnyTestHidden.
ANY_SRC = gen/AnyTestSK.cc gen/AnyTestDynSK.cc
ANY_HDR = gen/AnyTest.hh
HIDDEN_SRC = gen/AnyTestHiddenSK.cc gen/AnyTestHiddenDynSK.cc
HIDDEN_HDR = gen/AnyTestHidden.hh

all: server client poa_probe_server

$(STUB_SRC) $(STUB_HDR): $(IDLDIR)/InteropTest.idl
	mkdir -p gen
	$(OMNIIDL) -bcxx -Wbh=.hh -Wbs=SK.cc -C gen -I$(IDLDIR) $<

$(PROBE_SRC) $(PROBE_HDR): $(IDLDIR)/PoaProbe.idl
	mkdir -p gen
	$(OMNIIDL) -bcxx -Wbh=.hh -Wbs=SK.cc -C gen -I$(IDLDIR) $<

$(ANY_SRC) $(ANY_HDR): $(IDLDIR)/AnyTest.idl
	mkdir -p gen
	$(OMNIIDL) -bcxx -Wba -Wbh=.hh -Wbs=SK.cc -Wbd=DynSK.cc -C gen -I$(IDLDIR) $<

$(HIDDEN_SRC) $(HIDDEN_HDR): $(IDLDIR)/AnyTestHidden.idl $(ANY_HDR)
	mkdir -p gen
	$(OMNIIDL) -bcxx -Wba -Wbh=.hh -Wbs=SK.cc -Wbd=DynSK.cc -C gen -I$(IDLDIR) $<

server: server.cc common.h anytest.h $(STUB_SRC) $(STUB_HDR) $(ANY_SRC) $(ANY_HDR)
	$(CXX) $(CPPFLAGS) $(CXXFLAGS) -o $@ server.cc $(STUB_SRC) $(ANY_SRC) $(LDLIBS)

client: client.cc common.h anytest.h $(STUB_SRC) $(STUB_HDR) $(ANY_SRC) $(ANY_HDR) $(HIDDEN_SRC) $(HIDDEN_HDR)
	$(CXX) $(CPPFLAGS) $(CXXFLAGS) -o $@ client.cc $(STUB_SRC) $(ANY_SRC) $(HIDDEN_SRC) $(LDLIBS)

poa_probe_server: poa_probe_server.cc $(PROBE_SRC) $(PROBE_HDR)
	$(CXX) $(CPPFLAGS) $(CXXFLAGS) -o $@ poa_probe_server.cc $(PROBE_SRC) $(LDLIBS)

install: server client poa_probe_server
	mkdir -p $(OUT)
	cp server client poa_probe_server $(OUT)/

clean:
	rm -rf gen server client poa_probe_server

.PHONY: all install clean
