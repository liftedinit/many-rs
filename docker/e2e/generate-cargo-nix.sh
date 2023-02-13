set -bepu

echo "filter-syscalls = false" >> /etc/nix/nix.conf
echo "experimental-features = nix-command flakes" >> /etc/nix/nix.conf
# Uncomment to update cargo2nix
#cd docker/e2e
#nix flake lock --update-input cargo2nix
#cd ../..
nix run github:cargo2nix/cargo2nix/unstable --max-jobs $CPUCORES -- -f Cargo.nix.new
mv Cargo.nix.new Cargo.nix
chown $UINFO Cargo.nix
