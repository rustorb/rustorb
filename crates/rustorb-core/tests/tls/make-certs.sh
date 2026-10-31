#!/bin/sh
# Makes the certificates of the TLS tests (tests/tls.rs and the interop
# SSLIOP suite): two CAs, and from the first a server, a client and a
# server whose certificate names another host. Valid for 100 years; run
# again (with OpenSSL) to replace them.
set -eu
cd "$(dirname "$0")"
days=36500

key() { openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out "$1" 2>/dev/null; }

ca() { # name subject
  key "$1.key"
  openssl req -x509 -new -key "$1.key" -sha256 -days $days -subj "/CN=$2" \
    -addext "basicConstraints=critical,CA:TRUE" \
    -addext "keyUsage=critical,keyCertSign,cRLSign" -out "$1.pem"
}

leaf() { # name ca subject san
  key "$1.key"
  openssl req -new -key "$1.key" -subj "/CN=$3" -out "$1.csr"
  printf '%s\n' "basicConstraints=critical,CA:FALSE" \
    "keyUsage=critical,digitalSignature" \
    "extendedKeyUsage=serverAuth,clientAuth" \
    "subjectAltName=$4" > "$1.ext"
  openssl x509 -req -in "$1.csr" -CA "$2.pem" -CAkey "$2.key" -CAcreateserial \
    -sha256 -days $days -extfile "$1.ext" -out "$1.pem" 2>/dev/null
  rm -f "$1.csr" "$1.ext"
}

ca ca "rustorb test CA"
ca other-ca "rustorb other test CA"
# The interop containers reach each other by service name.
leaf server ca "rustorb test server" \
  "DNS:localhost,IP:127.0.0.1,IP:::1,DNS:server,DNS:naming,DNS:client"
leaf client ca "rustorb test client" "DNS:client,DNS:localhost"
leaf wrong-name ca "rustorb test elsewhere" "DNS:elsewhere.invalid"
leaf other-server other-ca "rustorb other test server" "DNS:localhost,IP:127.0.0.1"
# One file with certificate and key, as omniORB's key file takes them.
for n in server client; do cat "$n.pem" "$n.key" > "$n-bundle.pem"; done
rm -f ./*.srl ca.key other-ca.key
