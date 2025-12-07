{ pkgs ? import <nixpkgs> {} }:

pkgs.stdenv.mkDerivation {
  pname = "yandex-browser-stable";
  version = "25.10.1.1173-1";

  src = pkgs.fetchurl {
    url = "https://repo.yandex.ru/yandex-browser/deb/pool/main/y/yandex-browser-stable/yandex-browser-stable_25.10.1.1173-1_amd64.deb";
    sha256 = "0r5a6ylwmd69qkhaxvgd4scl55miv7m244w4sk2cj3msa1gibv1i";
  };

  dontWrapQtApps = true;

  nativeBuildInputs = [
    pkgs.autoPatchelfHook
    pkgs.dpkg
    pkgs.makeWrapper
  ];

  buildInputs = [
    pkgs.alsa-lib
    pkgs.at-spi2-atk
    pkgs.cairo
    pkgs.cups
    pkgs.dbus
    pkgs.expat
    pkgs.glib
    pkgs.glibc
    pkgs.libdrm
    pkgs.libgcc
    pkgs.libglvnd
    pkgs.libxkbcommon
    pkgs.mesa
    pkgs.nspr
    pkgs.nss
    pkgs.pango
    pkgs.qt5.qtbase
    pkgs.qt6.qtbase
    pkgs.systemd
    pkgs.vulkan-loader
    pkgs.xorg.libX11
    pkgs.xorg.libXcomposite
    pkgs.xorg.libXdamage
    pkgs.xorg.libXext
    pkgs.xorg.libXfixes
    pkgs.xorg.libXrandr
    pkgs.xorg.libxcb
  ];
  
  

  unpackPhase = "dpkg-deb --fsys-tarfile $src | tar -x --no-same-permissions --no-same-owner";

  installPhase = ''
    mkdir -p $out
    cp -r usr/* $out/ 2>/dev/null || true
    cp -r opt $out/ 2>/dev/null || true
    cp -r bin $out/ 2>/dev/null || true

    MAIN_BIN=$(find $out/opt -type f -executable -size +10M | head -n1)

    if [ -n "$MAIN_BIN" ]; then
      mkdir -p $out/bin
      BIN_NAME=$(basename "$MAIN_BIN")
      ln -s "$MAIN_BIN" "$out/bin/$BIN_NAME"

      wrapProgram "$out/bin/$BIN_NAME" \
        --prefix LD_LIBRARY_PATH : "${pkgs.lib.makeLibraryPath [ pkgs.libglvnd pkgs.mesa pkgs.libdrm pkgs.vulkan-loader pkgs.libxkbcommon ]}" \
        --add-flags "--no-sandbox"
    fi
  '';

  meta = {
    description = "Automatically packaged yandex-browser-stable";
    platforms = [ "x86_64-linux" ];
  };
}
