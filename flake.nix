{
  description = "Almanac: Mail, Calendar and Contacts (Rust + Qt Quick)";

  # nixos-unstable, pinned in flake.lock (the same revision as the
  # nixpkgs-unstable input in etc-nixos). `nix flake update` moves the pin.
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAll = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});

      # The Qt modules Almanac uses, merged into one tree with qmake (what
      # cxx-qt's build reads to find Qt).
      qtFor = pkgs: with pkgs.qt6; [ qtbase qtdeclarative qtsvg qtwebengine qtwebchannel qtpositioning qtwayland ];
      qtEnvFor = pkgs: pkgs.qt6.env "qt-almanac-${pkgs.qt6.qtbase.version}" ((builtins.tail (qtFor pkgs)) ++ [ pkgs.libGL ]);

      # The sources without build output, pictures or the local Qt link.
      srcFor = pkgs: pkgs.lib.cleanSourceWith {
        src = ./.;
        filter = path: type:
          let name = baseNameOf path; in
          !(builtins.elem name [ "target" "shots" ".qt-env" "result" ]);
      };
    in
    {
      packages = forAll (pkgs:
        let qtEnv = qtEnvFor pkgs; in
        {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "almanac";
            version = "0.1.0";
            src = srcFor pkgs;
            cargoLock.lockFile = ./Cargo.lock;
            nativeBuildInputs = [ pkgs.pkg-config pkgs.qt6.wrapQtAppsHook qtEnv ];
            buildInputs = (qtFor pkgs) ++ [ pkgs.libGL pkgs.dbus ];
            # Qt's build hooks point QMAKE at qtbase alone; the build needs the
            # merged tree (it finds each module's files through it).
            preBuild = ''
              export QMAKE=${qtEnv}/bin/qmake
            '';
            # Tests that need a desktop session (D-Bus, KWin) don't run in the build sandbox.
            doCheck = false;
            postInstall = ''
              install -Dm644 icons/almanac.svg $out/share/icons/hicolor/scalable/apps/almanac.svg
              install -Dm644 /dev/stdin $out/share/applications/almanac.desktop <<EOF
              [Desktop Entry]
              Type=Application
              Name=Almanac
              GenericName=Mail, Calendar and Contacts
              Comment=Mail, calendars and contacts in one app
              Exec=almanac
              Icon=almanac
              Categories=Office;Network;Email;Calendar;ContactManagement;
              StartupWMClass=almanac
              Terminal=false
              EOF
            '';
            meta = {
              description = "Mail, Calendar and Contacts in one desktop app";
              mainProgram = "almanac";
              platforms = systems;
            };
          };
        });

      apps = forAll (pkgs: {
        default = { type = "app"; program = "${self.packages.${pkgs.stdenv.hostPlatform.system}.default}/bin/almanac"; };
      });

      # `nix develop`: Rust and the same Qt, with qmake where the build looks.
      devShells = forAll (pkgs:
        let qtEnv = qtEnvFor pkgs; in
        {
          default = pkgs.mkShell {
            packages = [ pkgs.cargo pkgs.rustc pkgs.clippy pkgs.rustfmt pkgs.pkg-config qtEnv pkgs.libGL pkgs.dbus pkgs.watchexec ];
            QMAKE = "${qtEnv}/bin/qmake";
            shellHook = ''
              # The build also finds Qt through this link when QMAKE isn't set.
              ln -sfn ${qtEnv} .qt-env
            '';
          };
        });
    };
}
