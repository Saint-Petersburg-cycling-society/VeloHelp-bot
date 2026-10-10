#!/bin/sh
set -eu

DB="${DATABASE_PATH:-$HOME/.local/share/bike-aid/bike-aid.sqlite}"
OUT_DIR="${BACKUP_DIR:-$HOME/.local/share/bike-aid-backups}"
KEEP_DAYS="${KEEP_DAYS:-14}"

mkdir -p "$OUT_DIR"
umask 077

stamp="$(date -u +%Y%m%dT%H%M%SZ)"
out="$OUT_DIR/bike-aid-$stamp.sqlite"

# sqlite3 is a HOST maintenance tool. It is intentionally absent from the
# production container.
sqlite3 "$DB" ".backup '$out'"
sqlite3 "$out" 'PRAGMA integrity_check;' | grep -qx 'ok'

find "$OUT_DIR" -type f -name 'bike-aid-*.sqlite' -mtime "+$KEEP_DAYS" -delete
printf '%s\n' "$out"
