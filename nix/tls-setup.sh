#!/usr/bin/env bash
set -euo pipefail

if [[ $(uname -s) != Linux ]]; then
  echo 'tls-setup currently supports Linux hosts only.' >&2
  exit 1
fi
: "${DEVENV_STATE:?Enter the devenv shell first}"
: "${MOTET_SYSTEM_CA_BUNDLE:?The tls-setup command must supply the system CA bundle}"
: "${HOME:?HOME is required for Chromium certificate trust}"

if ! command -v ip >/dev/null 2>&1 || ! command -v resolvectl >/dev/null 2>&1; then
  echo 'tls-setup needs iproute2 and systemd-resolved on the host.' >&2
  exit 1
fi
if ! resolvectl status >/dev/null 2>&1; then
  echo 'Start systemd-resolved before running tls-setup.' >&2
  exit 1
fi
if ! awk '
  $1 == "nameserver" { found = 1; if ($2 != "127.0.0.53") other = 1 }
  END { exit !(found && !other) }
' /etc/resolv.conf; then
  echo '/etc/resolv.conf must use only the systemd-resolved stub 127.0.0.53 for wildcard Motet names.' >&2
  echo 'Configure your host resolver to use /run/systemd/resolve/stub-resolv.conf, then re-run tls-setup.' >&2
  exit 1
fi
if ! ss -H -l -n -u 'sport = :53' | grep -Eq '(^|[[:space:]])127\.0\.0\.53(%[^[:space:]:]+)?:53([[:space:]]|$)'; then
  echo 'The systemd-resolved DNS stub is not listening on 127.0.0.53:53; enable DNSStubListener and re-run tls-setup.' >&2
  exit 1
fi
ip_cmd=$(command -v ip)
resolvectl_cmd=$(command -v resolvectl)

umask 077
tls_dir="$DEVENV_STATE/tls"
mkdir -p "$tls_dir"
chmod 700 "$tls_dir"
exec 9>"$tls_dir/.setup.lock"
flock 9
tmp=$(mktemp -d "$tls_dir/.setup.XXXXXX")
trap 'rm -rf "$tmp"' EXIT

ca_key="$tls_dir/rootCA.key"
ca_cert="$tls_dir/rootCA.crt"
server_key="$tls_dir/server.key"
server_cert="$tls_dir/server.crt"

# Preserve the CA identity across runs: replacing it would invalidate issued certificates.
if [[ ! -e "$ca_key" && -e "$ca_cert" ]]; then
  echo "The local CA private key is missing: $ca_key" >&2
  echo 'Remove the old CA certificate and server certificate before creating a new CA.' >&2
  exit 1
fi
if [[ ! -e "$ca_key" ]]; then
  openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:prime256v1 -out "$tmp/rootCA.key"
  mv "$tmp/rootCA.key" "$ca_key"
fi
chmod 600 "$ca_key"
if [[ ! -e "$ca_cert" ]]; then
  openssl req -new -x509 -sha256 -days 3650 -key "$ca_key" \
    -subj '/CN=Motet Local Development CA' \
    -addext 'basicConstraints=critical,CA:TRUE,pathlen:0' \
    -addext 'keyUsage=critical,keyCertSign,cRLSign' \
    -addext 'subjectKeyIdentifier=hash' -out "$tmp/rootCA.crt"
  chmod 644 "$tmp/rootCA.crt"
  mv "$tmp/rootCA.crt" "$ca_cert"
fi
chmod 644 "$ca_cert"
openssl x509 -in "$ca_cert" -noout -pubkey > "$tmp/ca-cert.pub"
openssl pkey -in "$ca_key" -pubout > "$tmp/ca-key.pub"
if ! cmp -s "$tmp/ca-cert.pub" "$tmp/ca-key.pub"; then
  echo 'The local CA certificate and private key do not match.' >&2
  exit 1
fi
if ! openssl x509 -in "$ca_cert" -noout -checkend 2592000 >/dev/null; then
  echo 'The Motet local CA expires within 30 days; replace it and re-run tls-setup.' >&2
  exit 1
fi

renew_server=true
if [[ -f "$server_cert" && -f "$server_key" ]] \
  && openssl x509 -in "$server_cert" -noout -checkend 2592000 >/dev/null 2>&1 \
  && openssl verify -CAfile "$ca_cert" "$server_cert" >/dev/null 2>&1 \
  && openssl x509 -in "$server_cert" -noout -pubkey > "$tmp/cert.pub" \
  && openssl pkey -in "$server_key" -pubout > "$tmp/key.pub" \
  && cmp -s "$tmp/cert.pub" "$tmp/key.pub" \
  && openssl x509 -in "$server_cert" -noout -ext subjectAltName > "$tmp/sans"; then
  renew_server=false
  for name in motet.test pds.motet.test tenor.motet.test plc.motet.test relay.motet.test tap.motet.test '*.motet.test' '*.pds.motet.test'; do
    if ! grep -Fq "DNS:$name" "$tmp/sans"; then
      renew_server=true
      break
    fi
  done
fi
if "$renew_server"; then
  openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:prime256v1 -out "$tmp/server.key"
  openssl req -new -sha256 -key "$tmp/server.key" -subj '/CN=motet.test' -out "$tmp/server.csr"
  cat > "$tmp/server.ext" <<'EXT'
basicConstraints=critical,CA:FALSE
keyUsage=critical,digitalSignature
extendedKeyUsage=serverAuth
subjectAltName=DNS:motet.test,DNS:pds.motet.test,DNS:tenor.motet.test,DNS:plc.motet.test,DNS:relay.motet.test,DNS:tap.motet.test,DNS:*.motet.test,DNS:*.pds.motet.test
EXT
  openssl x509 -req -sha256 -days 397 -in "$tmp/server.csr" \
    -CA "$ca_cert" -CAkey "$ca_key" -CAserial "$tmp/rootCA.srl" -CAcreateserial \
    -extfile "$tmp/server.ext" -out "$tmp/server.crt"
  chmod 600 "$tmp/server.key"
  chmod 644 "$tmp/server.crt"
  mv "$tmp/server.key" "$server_key"
  mv "$tmp/server.crt" "$server_cert"
fi
chmod 600 "$server_key"
chmod 644 "$server_cert"

# Retain public roots when clients select the bundle through SSL_CERT_FILE.
cat "$MOTET_SYSTEM_CA_BUNDLE" "$ca_cert" > "$tmp/ca-bundle.crt"
chmod 644 "$tmp/ca-bundle.crt"
if ! cmp -s "$tmp/ca-bundle.crt" "$tls_dir/ca-bundle.crt"; then
  mv "$tmp/ca-bundle.crt" "$tls_dir/ca-bundle.crt"
fi
chmod 644 "$tls_dir/ca-bundle.crt"

# Do not silently override someone else's mapping for these hostnames.
if ! awk '
  BEGIN {
    begin = "# BEGIN motet local development"
    end = "# END motet local development"
    names["motet.test"] = names["pds.motet.test"] = 1
    names["tenor.motet.test"] = names["plc.motet.test"] = 1
    names["relay.motet.test"] = names["tap.motet.test"] = 1
  }
  $0 == begin {
    if (inside || seen) { bad = 1; exit }
    inside = seen = 1
    next
  }
  $0 == end {
    if (!inside) { bad = 1; exit }
    inside = 0
    next
  }
  inside { next }
  $1 != "127.0.0.1" {
    for (i = 2; i <= NF && $i !~ /^#/; i++) {
      if (names[tolower($i)]) {
        printf "Conflicting /etc/hosts mapping for %s; resolve it before tls-setup.\n", $i > "/dev/stderr"
        bad = 1
        exit
      }
    }
  }
  { print }
  END {
    if (inside || bad) exit 1
  }
' /etc/hosts > "$tmp/hosts"; then
  echo 'Could not prepare the Motet /etc/hosts block.' >&2
  exit 1
fi
cat >> "$tmp/hosts" <<'HOSTS'
# BEGIN motet local development
127.0.0.1 motet.test pds.motet.test tenor.motet.test plc.motet.test relay.motet.test tap.motet.test
# END motet local development
HOSTS
if ! cmp -s /etc/hosts "$tmp/hosts"; then
  sudo tee /etc/hosts < "$tmp/hosts" >/dev/null
fi

# Add one host CA, rather than disabling certificate verification in clients.
if command -v update-ca-certificates >/dev/null 2>&1; then
  system_cert=/usr/local/share/ca-certificates/motet-dev-root.crt
  if ! cmp -s "$ca_cert" "$system_cert"; then
    sudo install -m 644 "$ca_cert" "$system_cert"
    sudo update-ca-certificates
  fi
elif command -v update-ca-trust >/dev/null 2>&1; then
  system_cert=/etc/pki/ca-trust/source/anchors/motet-dev-root.crt
  if ! cmp -s "$ca_cert" "$system_cert"; then
    sudo install -m 644 "$ca_cert" "$system_cert"
    sudo update-ca-trust extract
  fi
else
  echo 'Install the local CA in your Linux system trust store, then re-run tls-setup.' >&2
  echo "CA certificate: $ca_cert" >&2
  exit 1
fi

# Chromium M146+ uses the new path unless a legacy shared database exists.
if [[ -d "$HOME/.pki/nssdb" ]]; then
  nss_dir="$HOME/.pki/nssdb"
else
  nss_dir="$HOME/.local/share/pki/nssdb"
fi
mkdir -p "$nss_dir"
nss_db="sql:$nss_dir"
nickname='Motet Local Development CA'
if [[ ! -e "$nss_dir/cert9.db" ]]; then
  certutil -N -d "$nss_db" --empty-password
fi
if certutil -L -d "$nss_db" -n "$nickname" -a > "$tmp/nss-ca.crt" 2>/dev/null; then
  openssl x509 -in "$tmp/nss-ca.crt" -outform DER -out "$tmp/nss-ca.der"
  openssl x509 -in "$ca_cert" -outform DER -out "$tmp/rootCA.der"
  if ! cmp -s "$tmp/nss-ca.der" "$tmp/rootCA.der"; then
    certutil -D -d "$nss_db" -n "$nickname"
    certutil -A -d "$nss_db" -n "$nickname" -t 'C,,' -i "$ca_cert"
  fi
else
  certutil -A -d "$nss_db" -n "$nickname" -t 'C,,' -i "$ca_cert"
fi
certutil -M -d "$nss_db" -n "$nickname" -t 'C,,'

# Route only Motet's zone to a local resolver; never replace global DNS.
if ! "$ip_cmd" link show dev motetdns >/dev/null 2>&1; then
  sudo "$ip_cmd" link add motetdns type dummy
elif ! "$ip_cmd" -details link show dev motetdns | grep -qw dummy; then
  echo 'The motetdns interface already exists but is not a dummy link.' >&2
  exit 1
fi
if ! "$ip_cmd" -o -4 addr show dev motetdns | grep -Fq 'inet 169.254.253.253/32 '; then
  sudo "$ip_cmd" addr add 169.254.253.253/32 dev motetdns
fi
sudo "$ip_cmd" link set motetdns up
sudo "$resolvectl_cmd" dns motetdns 127.0.0.1:53535
sudo "$resolvectl_cmd" domain motetdns '~motet.test'
sudo "$resolvectl_cmd" default-route motetdns no

# This runtime-only setting resets at reboot and never restricts a lower value.
if (( $(sysctl -n net.ipv4.ip_unprivileged_port_start) > 443 )); then
  sudo sysctl -w net.ipv4.ip_unprivileged_port_start=443
fi
printf 'Local TLS ready. CA: %s; Chromium NSS: %s\n' "$ca_cert" "$nss_dir"
