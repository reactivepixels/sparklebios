# Template only. This formula does not belong in this repository, it belongs
# in the reactivepixels/homebrew-tap repository, and this file is here so a
# human has a starting point to copy over once that tap exists.
#
# Every sha256 below is a placeholder, not a real digest. The real values come
# from the SHA256SUMS file attached to the GitHub release for the version
# being packaged: match each tarball name to its line in SHA256SUMS and paste
# the digest in. The version string is a placeholder too.
class Sparklebios < Formula
  desc "A 1995 POST screen for your terminal that is secretly a health check"
  homepage "https://github.com/reactivepixels/sparklebios"
  version "0.0.0" # PLACEHOLDER-VERSION, set to the real released version, no leading v.
  license any_of: ["MIT", "Apache-2.0"]

  on_macos do
    on_arm do
      url "https://github.com/reactivepixels/sparklebios/releases/download/v#{version}/bios-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000PLACEHOLDER"
    end
    on_intel do
      url "https://github.com/reactivepixels/sparklebios/releases/download/v#{version}/bios-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "1111111111111111111111111111111111111111111111111111PLACEHOLDER"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/reactivepixels/sparklebios/releases/download/v#{version}/bios-v#{version}-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "2222222222222222222222222222222222222222222222222222PLACEHOLDER"
    end
    on_intel do
      url "https://github.com/reactivepixels/sparklebios/releases/download/v#{version}/bios-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "3333333333333333333333333333333333333333333333333333PLACEHOLDER"
    end
  end

  def install
    bin.install "bios"
  end

  test do
    system "#{bin}/bios", "--version"
  end
end
