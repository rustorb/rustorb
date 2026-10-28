# Builds the omniORB C++ server and client for the interop tests.
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

all: server client

$(STUB_SRC) $(STUB_HDR): $(IDLDIR)/InteropTest.idl
	mkdir -p gen
	$(OMNIIDL) -bcxx -Wbh=.hh -Wbs=SK.cc -C gen -I$(IDLDIR) $<

server: server.cc common.h $(STUB_SRC) $(STUB_HDR)
	$(CXX) $(CPPFLAGS) $(CXXFLAGS) -o $@ server.cc $(STUB_SRC) $(LDLIBS)

client: client.cc common.h $(STUB_SRC) $(STUB_HDR)
	$(CXX) $(CPPFLAGS) $(CXXFLAGS) -o $@ client.cc $(STUB_SRC) $(LDLIBS)

install: server client
	mkdir -p $(OUT)
	cp server client $(OUT)/

clean:
	rm -rf gen server client

.PHONY: all install clean
