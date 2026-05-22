#!/usr/bin/env bash
# Patch Tauri/linuxdeploy AppImage output so WebKitGTK starts with the host
# graphics stack first. This avoids EGL/GBM crashes on rolling Linux systems.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/lib/common.sh
source "$SCRIPT_DIR/lib/common.sh"

APPIMAGE_DIR="${1:-"$PROJECT_ROOT/src-tauri/target/release/bundle/appimage"}"

if [[ "$(uname -s)" != "Linux" ]]; then
    info "非 Linux 平台，跳过 AppImage 后处理"
    exit 0
fi

if [[ ! -d "$APPIMAGE_DIR" ]]; then
    warn "未找到 AppImage 输出目录，跳过: $APPIMAGE_DIR"
    exit 0
fi
APPIMAGE_DIR="$(cd "$APPIMAGE_DIR" && pwd)"

extract_appimages_when_appdir_missing() {
    if [[ -n "$(find "$APPIMAGE_DIR" -maxdepth 1 -type d -name '*.AppDir' -print -quit)" ]]; then
        return 0
    fi

    local appimage appdir
    while IFS= read -r -d '' appimage; do
        appdir="$APPIMAGE_DIR/$(basename "${appimage%.AppImage}").AppDir"
        rm -rf "$appdir"
        info "解包 AppImage 以便后处理: $(basename "$appimage")"
        (
            cd "$APPIMAGE_DIR"
            rm -rf squashfs-root
            APPIMAGE_EXTRACT_AND_RUN=1 "$appimage" --appimage-extract >/dev/null
            mv squashfs-root "$appdir"
        )
    done < <(find "$APPIMAGE_DIR" -maxdepth 1 -type f -name '*.AppImage' -print0)
}

find_appimagetool_plugin() {
    local candidates=(
        "$HOME/.cache/tauri/linuxdeploy-plugin-appimage.AppImage"
        "$HOME/.cache/tauri/linuxdeploy-plugin-appimage-x86_64.AppImage"
    )
    local candidate
    for candidate in "${candidates[@]}"; do
        if [[ -x "$candidate" ]]; then
            printf '%s\n' "$candidate"
            return 0
        fi
    done
    return 1
}

resolve_app_binary() {
    local appdir="$1"
    if [[ -x "$appdir/usr/bin/sprite-anime" ]]; then
        printf '%s\n' "sprite-anime"
        return 0
    fi

    local binary
    binary="$(find "$appdir/usr/bin" -maxdepth 1 -type f -perm -111 -printf '%f\n' | sort | head -n 1 || true)"
    if [[ -z "$binary" ]]; then
        return 1
    fi
    printf '%s\n' "$binary"
}

write_portable_apprun() {
    local appdir="$1"
    local app_binary="$2"

    cat > "$appdir/AppRun" <<'APPRUN'
#!/usr/bin/env bash
set -eo pipefail

HERE="$(dirname "$(readlink -f "$0")")"
cd "$HERE"
export APPDIR="$HERE"

APP_BINARY="__SPRITE_ANIMTE_APP_BINARY__"
SYSTEM_LIB_PATHS="/usr/lib:/usr/lib64:/usr/lib/x86_64-linux-gnu"
APPDIR_LIB_PATHS="$APPDIR/usr/lib:$APPDIR/usr/lib/x86_64-linux-gnu:$APPDIR/usr/lib64:$APPDIR/lib:$APPDIR/lib/x86_64-linux-gnu:$APPDIR/lib64"

if [[ "${SPRITE_ANIMTE_FORCE_BUNDLED_LIBS:-0}" == "1" ]]; then
  export LD_LIBRARY_PATH="$APPDIR_LIB_PATHS:${LD_LIBRARY_PATH:-}:$SYSTEM_LIB_PATHS"
else
  # Prefer the host WebKit/GTK/GL stack. Mixing Ubuntu-bundled WebKit with
  # newer host EGL/GBM libraries can abort with "Could not create surfaceless
  # EGL display" before the application window is usable.
  export LD_LIBRARY_PATH="$SYSTEM_LIB_PATHS:${LD_LIBRARY_PATH:-}:$APPDIR_LIB_PATHS"
fi

if [[ -d "$HERE/apprun-hooks" ]]; then
  while IFS= read -r -d '' hook; do
    # shellcheck disable=SC1090
    source "$hook"
  done < <(find "$HERE/apprun-hooks" -maxdepth 1 -type f -print0 | sort -z)
fi

if [[ "${SPRITE_ANIMTE_FORCE_GPU:-0}" != "1" ]]; then
  if [[ -n "${DISPLAY:-}" ]]; then
    unset WAYLAND_DISPLAY
    export GDK_BACKEND="${GDK_BACKEND:-x11}"
    export EGL_PLATFORM="${EGL_PLATFORM:-x11}"
  fi
  export WEBKIT_DISABLE_COMPOSITING_MODE="${WEBKIT_DISABLE_COMPOSITING_MODE:-1}"
  export WEBKIT_DISABLE_DMABUF_RENDERER="${WEBKIT_DISABLE_DMABUF_RENDERER:-1}"
  export WEBKIT_DMABUF_RENDERER_DISABLE_GBM="${WEBKIT_DMABUF_RENDERER_DISABLE_GBM:-1}"
  export LIBGL_ALWAYS_SOFTWARE="${LIBGL_ALWAYS_SOFTWARE:-1}"
  export GSK_RENDERER="${GSK_RENDERER:-cairo}"
  export NO_AT_BRIDGE="${NO_AT_BRIDGE:-1}"
fi

WEBKIT_BASE=""
if [[ "${SPRITE_ANIMTE_FORCE_BUNDLED_WEBKIT:-0}" == "1" ]]; then
  WEBKIT_CANDIDATES=(
    "$APPDIR/lib/x86_64-linux-gnu/webkit2gtk-4.1"
    "$APPDIR/usr/lib/x86_64-linux-gnu/webkit2gtk-4.1"
    "$APPDIR/usr/lib/webkit2gtk-4.1"
    "$APPDIR/usr/lib64/webkit2gtk-4.1"
    "/usr/lib/webkit2gtk-4.1"
    "/usr/lib/x86_64-linux-gnu/webkit2gtk-4.1"
    "/usr/lib64/webkit2gtk-4.1"
  )
else
  WEBKIT_CANDIDATES=(
    "/usr/lib/webkit2gtk-4.1"
    "/usr/lib/x86_64-linux-gnu/webkit2gtk-4.1"
    "/usr/lib64/webkit2gtk-4.1"
    "$APPDIR/lib/x86_64-linux-gnu/webkit2gtk-4.1"
    "$APPDIR/usr/lib/x86_64-linux-gnu/webkit2gtk-4.1"
    "$APPDIR/usr/lib/webkit2gtk-4.1"
    "$APPDIR/usr/lib64/webkit2gtk-4.1"
  )
fi
for candidate in "${WEBKIT_CANDIDATES[@]}"; do
  if [[ -x "$candidate/WebKitNetworkProcess" ]]; then
    WEBKIT_BASE="$candidate"
    break
  fi
done
if [[ -n "$WEBKIT_BASE" ]]; then
  export WEBKIT_EXEC_PATH="$WEBKIT_BASE"
  export WEBKIT_INJECTED_BUNDLE_PATH="$WEBKIT_BASE/injected-bundle"
fi

if [[ "${SPRITE_ANIMTE_DEBUG_APPRUN:-0}" == "1" ]]; then
  echo "[AppRun] APPDIR=$APPDIR" >&2
  echo "[AppRun] LD_LIBRARY_PATH=$LD_LIBRARY_PATH" >&2
  echo "[AppRun] WEBKIT_EXEC_PATH=${WEBKIT_EXEC_PATH:-}" >&2
  echo "[AppRun] GDK_BACKEND=${GDK_BACKEND:-}" >&2
  echo "[AppRun] EGL_PLATFORM=${EGL_PLATFORM:-}" >&2
fi

exec "$HERE/usr/bin/$APP_BINARY" "$@"
APPRUN

    sed -i "s|__SPRITE_ANIMTE_APP_BINARY__|$app_binary|g" "$appdir/AppRun"
    chmod +x "$appdir/AppRun"
}

prepare_webkit_fallback_paths() {
    local appdir="$1"

    if [[ -d "$appdir/usr/lib/x86_64-linux-gnu" ]]; then
        mkdir -p "$appdir/lib"
        ln -sfn ../usr/lib/x86_64-linux-gnu "$appdir/lib/x86_64-linux-gnu"
    fi

    if [[ -d "$appdir/usr/lib/x86_64-linux-gnu/webkit2gtk-4.1" ]]; then
        mkdir -p "$appdir/lib"
        ln -sfn x86_64-linux-gnu/webkit2gtk-4.1 "$appdir/lib/webkit2gtk-4.1"
    fi
}

repack_appimage() {
    local appdir="$1"
    local output="$2"
    local plugin="$3"
    local arch
    arch="$(uname -m)"
    case "$arch" in
        amd64) arch="x86_64" ;;
        arm64) arch="aarch64" ;;
    esac

    local runtime_file tmp_dir tmp_output offset
    runtime_file="$(mktemp /tmp/sprite_animte_runtime.XXXXXX)"
    tmp_dir="$(mktemp -d /tmp/sprite_animte_appimagetool.XXXXXX)"
    tmp_output="${output}.tmp"
    rm -f "$tmp_output"

    offset="$(APPIMAGE_EXTRACT_AND_RUN=1 "$plugin" --appimage-offset)"
    dd if="$plugin" of="$runtime_file" bs=1 count="$offset" status=none
    chmod +x "$runtime_file"

    cp "$plugin" "$tmp_dir/plugin.AppImage"
    (
        cd "$tmp_dir"
        APPIMAGE_EXTRACT_AND_RUN=1 ./plugin.AppImage --appimage-extract >/dev/null
        ARCH="$arch" "$tmp_dir/squashfs-root/appimagetool-prefix/AppRun" \
            --runtime-file "$runtime_file" \
            "$appdir" "$tmp_output"
    )

    chmod +x "$tmp_output"
    mv -f "$tmp_output" "$output"
    rm -f "$runtime_file"
    rm -rf "$tmp_dir"
}

extract_appimages_when_appdir_missing

if [[ -z "$(find "$APPIMAGE_DIR" -maxdepth 1 -type d -name '*.AppDir' -print -quit)" ]]; then
    warn "未找到 AppDir，跳过 AppImage 后处理: $APPIMAGE_DIR"
    exit 0
fi

plugin="$(find_appimagetool_plugin || true)"
if [[ -z "$plugin" ]]; then
    error "未找到 linuxdeploy-plugin-appimage，无法重新组装 AppImage"
    exit 1
fi

found="false"
while IFS= read -r -d '' appdir; do
    found="true"
    app_binary="$(resolve_app_binary "$appdir")" || {
        warn "无法解析 AppDir 主程序，跳过: $appdir"
        continue
    }

    info "修正 AppImage AppRun: $appdir"
    prepare_webkit_fallback_paths "$appdir"
    write_portable_apprun "$appdir" "$app_binary"

    appdir_stem="$(basename "${appdir%.AppDir}")"
    appimage="$(find "$APPIMAGE_DIR" -maxdepth 1 -type f -name "${appdir_stem}*.AppImage" | sort | head -n 1 || true)"
    if [[ -z "$appimage" ]]; then
        appimage="$(find "$APPIMAGE_DIR" -maxdepth 1 -type f -name '*.AppImage' | sort | head -n 1 || true)"
    fi
    if [[ -z "$appimage" ]]; then
        version="$(node -e "console.log(require(process.argv[1]).version)" "$PROJECT_ROOT/src-tauri/tauri.conf.json" 2>/dev/null || printf '0.1.0')"
        appimage="$APPIMAGE_DIR/${app_binary}_${version}_$(uname -m).AppImage"
    fi

    info "重新组装 AppImage: $(basename "$appimage")"
    repack_appimage "$appdir" "$appimage" "$plugin"

    if [[ "${SPRITE_ANIMTE_KEEP_APPDIR:-0}" != "1" ]]; then
        rm -rf "$appdir"
    fi
done < <(find "$APPIMAGE_DIR" -maxdepth 1 -type d -name '*.AppDir' -print0)

if [[ "$found" != "true" ]]; then
    warn "未找到 AppDir，跳过 AppImage 后处理: $APPIMAGE_DIR"
fi
