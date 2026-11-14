#!/system/bin/sh
if [ -z "$POCKETTERM_FILES_DIR" ] || [ -z "$LD_PRELOAD" ]; then
    echo "gate: missing child environment" >&2
    exit 80
fi
echo "gate shebang stdout OK"
echo "gate shebang stderr OK" >&2
