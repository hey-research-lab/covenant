#!/bin/sh
# Offline installer for an already verified, unpacked Covenant Compute archive.
# Installs binaries and their examples, documentation and license notices.
set -eu

fail() { printf 'covenant-compute install: %s\n' "$*" >&2; exit 1; }
[ "$#" -eq 0 ] || fail "use COVENANT_COMPUTE_INSTALL_DIR and COVENANT_COMPUTE_DATA_DIR to select destinations"
package="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
bin_dir="${COVENANT_COMPUTE_INSTALL_DIR:-$HOME/.local/bin}"
data_dir="${COVENANT_COMPUTE_DATA_DIR:-$HOME/.local/share/covenant-compute}"
commands="covenant-compute covenant-compute-mcp covenant-compute-node covenant-compute-stake"
for command in $commands; do
  [ -f "$package/$command" ] && [ -x "$package/$command" ] || fail "missing executable: $command"
  [ ! -d "$bin_dir/$command" ] || fail "binary destination is a directory: $bin_dir/$command"
done
version="$("$package/covenant-compute" --version)"
version="${version#covenant-compute }"
case "$version" in ''|*[!0-9A-Za-z.-]*) fail "invalid buyer version";; esac
for command in $commands; do
  [ "$("$package/$command" --version)" = "$command $version" ] || fail "packaged binary version differs: $command"
done
for document in LICENSE NOTICE.md SECURITY_AUDIT_LICENSE README.md RELEASE_NOTES.md BUILD_INFO.json; do
  [ -f "$package/$document" ] || fail "missing document: $document"
done
for directory in examples docs licenses tools; do
  [ -d "$package/$directory" ] || fail "missing data directory: $directory"
done
[ "$bin_dir" != "$data_dir" ] || fail "binary and data destinations must differ"
# Reject symlink destinations rather than replace an unrelated installation.
[ ! -L "$data_dir" ] || fail "data destination cannot be a symlink"
mkdir -p "$bin_dir" "$(dirname "$data_dir")"
bin_stage=
data_stage=
trap '[ -z "$bin_stage" ] || rm -rf "$bin_stage"; [ -z "$data_stage" ] || rm -rf "$data_stage"' EXIT
trap 'exit 1' INT TERM
bin_stage="$(mktemp -d "$bin_dir/.covenant-compute.XXXXXX")"
data_stage="$(mktemp -d "$(dirname "$data_dir")/.covenant-compute-data.XXXXXX")"
for command in $commands; do
  install -m 0755 "$package/$command" "$bin_stage/$command"
done
for document in LICENSE NOTICE.md SECURITY_AUDIT_LICENSE README.md RELEASE_NOTES.md BUILD_INFO.json; do
  install -m 0644 "$package/$document" "$data_stage/$document"
done
for directory in examples docs licenses tools; do
  cp -R "$package/$directory" "$data_stage/"
done
# A versioned directory keeps existing installed data intact if copying fails.
# Runtime behavior remains embedded in each binary; these files are supporting
# examples and notices, never a mutable dependency used to execute the client.
mkdir -p "$data_dir"
[ ! -e "$data_dir/$version" ] || {
  # Reinstalling the exact same data is harmless; do not replace different data.
  diff -qr "$data_stage" "$data_dir/$version" >/dev/null ||
    fail "data for version $version already exists and differs; choose a fresh COVENANT_COMPUTE_DATA_DIR"
}
if [ ! -e "$data_dir/$version" ]; then
  mv "$data_stage" "$data_dir/$version"
  data_stage=
fi
for command in $commands; do
  mv -f "$bin_stage/$command" "$bin_dir/$command"
done
printf 'Installed %s in %s\nExamples, guides and licenses: %s/%s\n' "$version" "$bin_dir" "$data_dir" "$version"
printf 'Local Knowledge requires no coordinator or wallet. Start with:\n  %s/covenant-compute knowledge compile %s/%s/examples/knowledge\n' "$bin_dir" "$data_dir" "$version"
