{
  inputs = {
    cargo2nix.url = "github:cargo2nix/cargo2nix/unstable";
    nixpkgs.follows = "cargo2nix/nixpkgs";
    flake-utils.follows = "cargo2nix/flake-utils";
  };

  outputs = { self, cargo2nix, nixpkgs, flake-utils, ... }:
  flake-utils.lib.eachDefaultSystem (system:
  let
    rustToolchain = builtins.fromTOML (builtins.readFile ../../rust-toolchain.toml);
    pkgs = import nixpkgs {
      inherit system;
      overlays = [
        cargo2nix.overlays.default
        (final: prev: {
          baseImage = final.dockerTools.buildLayeredImage {
            name = "many-base";
            contents = [
              final.bash
              final.curl
              final.iputils
            ];
          };

          dockerImageFromPkg = pkg: name: final.dockerTools.buildLayeredImage {
            fromImage = final.baseImage;
            name = "lifted/${name}";
            tag = "latest";
            contents = [
              (pkg {}).bin
            ];
            created = "now";
            config.Cmd = [ "${(pkg {}).bin}/bin/${name}" ];
          };

          rustPkgs = final.rustBuilder.makePackageSet {
            rustVersion = "2023-01-03";
            rustChannel = rustToolchain.toolchain.channel;
            extraRustComponents = rustToolchain.toolchain.components ++ [
              "rust-src"
            ];
            packageFun = import ./Cargo.nix;
            workspaceSrc = ../../.;
            packageOverrides = pkgs: pkgs.rustBuilder.overrides.all ++ [

              (pkgs.rustBuilder.rustLib.makeOverride {
                name = "cryptoki-sys";
                overrideAttrs = drv: {
                  LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
                  nativeBuildInputs = [
                    pkgs.llvmPackages.libcxxClang
                  ];
                };
              })
              (pkgs.rustBuilder.rustLib.makeOverride {
                name = "many-abci";
                overrideAttrs = drv: {
                  prePatch = ''
                    substituteInPlace build.rs --replace 'use vergen::{vergen, Config};' "use vergen::Config;"
                    substituteInPlace build.rs --replace 'vergen(config).expect("Vergen could not run.")' ""
                  '';
                  VERGEN_GIT_SHA = if (self ? rev ) then self.rev else "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"; # random sha1
                };
              })
              (pkgs.rustBuilder.rustLib.makeOverride {
                name = "many-ledger";
                overrideAttrs = drv: {
                  prePatch = ''
                    substituteInPlace build.rs --replace 'use vergen::{vergen, Config};' "use vergen::Config;"
                    substituteInPlace build.rs --replace 'vergen(config).expect("Vergen could not run.")' ""
                  '';
                  VERGEN_GIT_SHA = if (self ? rev ) then self.rev else "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"; # random sha1
                };
              })
              (pkgs.rustBuilder.rustLib.makeOverride {
                name = "many-kvstore";
                overrideAttrs = drv: {
                  prePatch = ''
                    substituteInPlace build.rs --replace 'use vergen::{vergen, Config};' "use vergen::Config;"
                    substituteInPlace build.rs --replace 'vergen(config).expect("Vergen could not run.")' ""
                  '';
                  VERGEN_GIT_SHA = if (self ? rev ) then self.rev else "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"; # random sha1
                };
              })
              (pkgs.rustBuilder.rustLib.makeOverride {
                name = "librocksdb-sys";
                overrideAttrs = drv: {
                  LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
                  nativeBuildInputs = [
                    pkgs.llvmPackages.libcxxClang
                  ];
                };
              })
              (pkgs.rustBuilder.rustLib.makeOverride {
                name = "hidapi";
                overrideAttrs = drv: {
                    nativeBuildInputs = [
                        pkgs.libusb1
                    ];
                };
              })
            ];
          };

          manyPackages = final.lib.attrsets.mapAttrs (name: value: (value {}).bin) final.rustPkgs.workspace;
          manyImages = final.lib.attrsets.mapAttrs' (name: value: {
            name = "docker-${name}";
            value = final.dockerImageFromPkg value name;
          }) final.rustPkgs.workspace;
        })
      ];
    };
  in {
    packages = pkgs.manyPackages // (if pkgs.stdenv.isLinux then pkgs.manyImages else {});

    inherit self pkgs;
  });
}
